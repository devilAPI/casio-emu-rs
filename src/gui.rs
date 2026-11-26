//! Desktop frontend: winit window, software framebuffer, touch/mouse/keyboard.
//!
//! The UI is laid out in *window pixels* and reflows on resize.  The LCD is
//! drawn at the largest whole-number dot scale that fits, and all text is drawn
//! at whole-number multiples of the 8x8 font.  Nothing is ever stretched and
//! there are no letterboxed bars: the tinted body fills the window.

use casio_emu_rs::cpu::Cpu;
use casio_emu_rs::keys;
use casio_emu_rs::machine::Machine;
use casio_emu_rs::model::{self, BoardKind, Model};
use font8x8::UnicodeFonts;
use softbuffer::{Context, Surface};
use std::num::NonZeroU32;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::event::{ElementState, MouseButton, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};
use winit::window::{Window, WindowId};

// ---------------------------------------------------------------------------
// Palette: soft dusk tones, tinted warm charcoal, sage LCD, sand accent.
// ---------------------------------------------------------------------------

const BG: u32 = 0x001A_171C;
const BG_TOP: u32 = 0x002A_242C;
const PANEL: u32 = 0x002F_2832;
const BODY: u32 = 0x0033_2C36;
const BORDER: u32 = 0x0046_3C49;
const SHADOW: u32 = 0x0012_0F14;
const LCD_BEZEL: u32 = 0x0028_232A;
const LCD_BG: u32 = 0x00CF_D6C2;
const LCD_FG: u32 = 0x003C_4536;
const BUTTON: u32 = 0x0040_3846;
const BUTTON_HOT: u32 = 0x0055_4A5C;
const TEXT: u32 = 0x00EA_E2D8;
const MUTED: u32 = 0x00A7_9AA0;
const ACCENT: u32 = 0x00C9_A584;
const ACCENT_HI: u32 = 0x00E4_C7A6;

const TITLE: &str = "casio-emu-rs";
const HELP1: &str = "0-9 . + - * /   Enter   Esc AC";
const HELP2: &str = "F8 models   F6/F7 cycle   tap keys";

/// Emulation is time-budgeted so event handling and drawing always get a slot.
/// Drawing a frame takes ~1ms, so most of the 16ms frame goes to the core:
/// a 5ms slice left busy firmware (menu redraws) running below real speed.
const TICK_BUDGET: Duration = Duration::from_millis(12);
const MAX_STEPS: u32 = 240_000;
const FRAME_TIME: Duration = Duration::from_millis(16);
/// A tap or click is held this long so the firmware's matrix scan sees it.
const KEY_HOLD: Duration = Duration::from_millis(60);
const MIN_TOUCH_HOLD: Duration = Duration::from_millis(90);

// ---------------------------------------------------------------------------
// Rectangles and text fitting
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rect {
    x: usize,
    y: usize,
    w: usize,
    h: usize,
}

impl Rect {
    fn right(self) -> usize {
        self.x + self.w
    }

    fn bottom(self) -> usize {
        self.y + self.h
    }

    fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x as f32
            && y >= self.y as f32
            && x < self.right() as f32
            && y < self.bottom() as f32
    }
}

/// Whole-number text scale from the window height.  Independent of DPI
/// because it works in physical pixels: a 2x display has twice the height and
/// therefore twice the glyph scale, giving the same logical point size.
fn text_scale(height: usize) -> usize {
    ((height + 140) / 280).clamp(1, 8)
}

fn truncate(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars.max(1)).collect()
}

/// Integer scale for a key cap.  Width only reduces the scale; at scale 1 the
/// label is kept whole and allowed to spill into the gap between keys rather
/// than being cut to "LEF"/"RIG".
fn label_scale(text: &str, max_w: usize, max_h: usize, cap: usize) -> usize {
    let mut scale = cap.max(1);
    while scale > 1 && (text.chars().count() * 8 * scale > max_w || 8 * scale > max_h) {
        scale -= 1;
    }
    scale
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

fn key_grid(model: Model) -> (usize, usize) {
    if model.board == BoardKind::ClassWizII {
        (keys::CW2_GRID_W as usize, keys::CW2_GRID_H as usize)
    } else if model.board == BoardKind::ClassWiz {
        (keys::VNX_GRID_W as usize, keys::VNX_GRID_H as usize)
    } else {
        (keys::GRID_W as usize, 28)
    }
}

/// Integer LCD-dot scale used only to choose a pleasant initial window size.
fn natural_lcd_scale(model: Model) -> usize {
    let (gw, _) = key_grid(model);
    ((gw * 10) as f32 * 0.88 / model.lcd_width as f32)
        .round()
        .clamp(2.0, 4.0) as usize
}

fn preferred_size(model: Model) -> (f64, f64) {
    let (gw, gh) = key_grid(model);
    let ls = natural_lcd_scale(model);
    let w = ((gw * 10).max(model.lcd_width * ls) + 56).max(540);
    let h = 40 + 16 + model.lcd_height * ls + 28 + gh * 10 + 70;
    (w as f64, h as f64)
}

struct Layout {
    w: usize,
    h: usize,
    ts: usize,
    bar: Rect,
    body: Rect,
    bezel: Rect,
    lcd: Rect,
    lcd_scale: usize,
    keys: Rect,
    cell_x: f32,
    cell_y: f32,
    help_y: usize,
    help_scale: usize,
    btn: Rect,
    btn_scale: usize,
    btn_label: String,
    title_scale: usize,
    title_visible: bool,
}

fn compute_layout(model: Model, ww: usize, wh: usize) -> Layout {
    let w = ww.max(1);
    let h = wh.max(1);
    let ts = text_scale(h);
    let name = model.name.trim_start_matches("Casio ").to_string();
    let short = name.trim_start_matches("fx-").to_string();

    // Bar text uses the largest size at which the title and the model button
    // can coexist.
    let mut bts = ts;
    while bts > 1 && (TITLE.len() + short.len() + 8) * 8 * bts + 48 > w {
        bts -= 1;
    }

    let pad = (4 * ts + 4).min(w / 10).max(3);
    let bar_h = (8 * bts + 16).min(h / 4).max(20);

    let mut help_scale = ts;
    let help_max = w.saturating_sub(2 * pad);
    while help_scale > 1 && HELP1.len().max(HELP2.len()) * 8 * help_scale > help_max {
        help_scale -= 1;
    }
    let help_h = 2 * (8 * help_scale + 4) + 2;
    let help_y = h.saturating_sub(help_h);

    let body_y = (bar_h + pad / 2).min(h.saturating_sub(2));
    let body_bottom = h
        .saturating_sub(help_h + pad / 2)
        .max(body_y.saturating_add(2));
    let body = Rect {
        x: pad,
        y: body_y,
        w: w.saturating_sub(2 * pad).max(1),
        h: body_bottom.saturating_sub(body_y).max(1),
    };

    let inner = (5 * ts + 5).min(body.w / 6).max(3);
    let cx = body.x + inner;
    let cy = body.y + inner;
    let cw = body.w.saturating_sub(2 * inner).max(1);
    let ch = body.h.saturating_sub(2 * inner).max(1);

    let (gw, gh) = key_grid(model);
    let bezel_pad = (3 * ts + 3).max(3);
    let gap = 2 * ts + 4;

    // The LCD gets the largest whole-number scale that fits the content width,
    // capped at half the content height so the keypad always stays usable.
    // The keypad then takes the remaining space.
    let max_scale_x = cw.saturating_sub(2 * bezel_pad) / model.lcd_width;
    let lcd_budget_h = ch / 2;
    let max_scale_y = lcd_budget_h.saturating_sub(2 * bezel_pad) / model.lcd_height.max(1);
    let lcd_scale = max_scale_x.min(max_scale_y).max(1);
    let dot_w = model.lcd_width * lcd_scale;
    let dot_h = model.lcd_height * lcd_scale;
    let bezel_w = dot_w + 2 * bezel_pad;
    let bezel_h = dot_h + 2 * bezel_pad;

    let key_avail_h = ch.saturating_sub(bezel_h + gap);
    // The keypad stretches independently on both axes.  Horizontally it fills
    // the width of the screen bezel minus a small margin on each side, so it is
    // never exactly as wide as the screen.
    let key_margin = (2 * ts + 2).max(3);
    let max_key_w = bezel_w.saturating_sub(2 * key_margin).min(cw).max(1);
    let cell_x = (max_key_w as f32 / gw as f32).max(0.25);
    let cell_y = (key_avail_h as f32 / gh as f32).max(0.25);
    let key_w = (cell_x * gw as f32).round() as usize;
    let key_h = (cell_y * gh as f32).round() as usize;

    let block_h = bezel_h + gap + key_h;
    let top = cy + ch.saturating_sub(block_h) / 2;
    let bezel_x = cx + cw.saturating_sub(bezel_w) / 2;
    let key_x = cx + cw.saturating_sub(key_w) / 2;

    let bezel = Rect {
        x: bezel_x,
        y: top,
        w: bezel_w,
        h: bezel_h,
    };
    let lcd = Rect {
        x: bezel_x + bezel_pad,
        y: top + bezel_pad,
        w: dot_w,
        h: dot_h,
    };
    let keys = Rect {
        x: key_x,
        y: top + bezel_h + gap,
        w: key_w,
        h: key_h,
    };

    let btn_margin = (bts / 2 + 2).max(2);
    let btn_h = bar_h.saturating_sub(2 * btn_margin).max(14);
    let max_btn_w = ((w as f32 * 0.5) as usize)
        .max(140)
        .min(w.saturating_sub(2 * pad));
    let btn_pad = 3 * bts;
    let arrow_w = 8 * bts;
    let mut btn_scale = bts;
    while btn_scale > 1 && short.len() * 8 * btn_scale + arrow_w + 2 * btn_pad > max_btn_w {
        btn_scale -= 1;
    }
    let btn_w = (short.len() * 8 * btn_scale + arrow_w + 2 * btn_pad)
        .min(max_btn_w)
        .max(60);
    let btn = Rect {
        x: w.saturating_sub(pad + btn_w),
        y: (bar_h.saturating_sub(btn_h)) / 2,
        w: btn_w,
        h: btn_h,
    };

    let accent = (6 * bts + 4).max(8);
    let title_x = pad + accent + 3 * bts;
    let title_avail = btn.x.saturating_sub(title_x + pad);
    let mut title_scale = bts;
    while title_scale > 1 && TITLE.len() * 8 * title_scale > title_avail {
        title_scale -= 1;
    }
    let title_visible = TITLE.len() * 8 * title_scale <= title_avail;

    Layout {
        w,
        h,
        ts,
        bar: Rect {
            x: 0,
            y: 0,
            w,
            h: bar_h,
        },
        body,
        bezel,
        lcd,
        lcd_scale,
        keys,
        cell_x,
        cell_y,
        help_y,
        help_scale,
        btn,
        btn_scale,
        btn_label: short,
        title_scale,
        title_visible,
    }
}

/// Compact drop-down anchored below the model button.
struct Dropdown {
    panel: Rect,
    rows_y: usize,
    row_h: usize,
    scale: usize,
    first: usize,
    visible: usize,
}

fn dropdown_layout(l: &Layout, cursor: usize) -> Dropdown {
    let scale = if l.w >= 720 && l.ts >= 2 { 2 } else { 1 };
    let margin = 4;
    let rows = model::ALL.len();
    let longest = model::ALL
        .iter()
        .map(|m| m.name.trim_start_matches("Casio ").len())
        .max()
        .unwrap_or(20);
    let panel_w = (longest * 8 * scale + 80 + 2 * margin)
        .max(l.btn.w)
        .min(l.w.saturating_sub(2 * l.body.x))
        .max(220);
    // Squeeze the rows when necessary so the common case (every registered
    // model) fits without scrolling.  A viewport handles very long lists or
    // extremely short windows, and always follows the keyboard cursor.
    let available_h = l.h.saturating_sub(l.bar.h + 8);
    let base_row = 8 * scale + 10;
    let min_row = (8 * scale + 4).max(12);
    let mut row_h = base_row;
    if rows * row_h + 2 * margin > available_h {
        row_h = ((available_h.saturating_sub(2 * margin)) / rows).clamp(min_row, base_row);
    }
    let max_visible = ((available_h.saturating_sub(2 * margin)) / row_h)
        .max(1)
        .min(rows);
    let visible = max_visible.min(rows);
    let first = cursor
        .saturating_sub(visible / 2)
        .min(rows.saturating_sub(visible));
    let panel_h = visible * row_h + 2 * margin;
    let x = l.btn.x.min(l.w.saturating_sub(l.body.x + panel_w));
    let y = (l.btn.bottom() + 3)
        .min(l.h.saturating_sub(panel_h + 2))
        .max(2);
    Dropdown {
        panel: Rect {
            x,
            y,
            w: panel_w,
            h: panel_h,
        },
        rows_y: y + margin,
        row_h,
        scale,
        first,
        visible,
    }
}

fn dropdown_row(dd: &Dropdown, x: f32, y: f32, count: usize) -> Option<usize> {
    if !dd.panel.contains(x, y) || (y as usize) < dd.rows_y {
        return None;
    }
    let row = (y as usize - dd.rows_y) / dd.row_h;
    if row < dd.visible && dd.first + row < count {
        Some(dd.first + row)
    } else {
        None
    }
}

/// Key rectangle in grid units, grown into the empty space around it so the
/// caps fill more of their cell.  Each axis grows by a fraction of the
/// smallest gap to a neighbouring key on that axis (keys that touch do not
/// grow), so caps never overlap and the d-pad stays intact.
fn key_rect_units(keys: &[keys::CalcKey], k: &keys::CalcKey) -> (f32, f32, f32, f32) {
    const GROW: f32 = 0.35;
    let (x0, y0, x1, y1) = (k.x0 as f32, k.y0 as f32, k.x1 as f32, k.y1 as f32);
    let mut gx = f32::MAX;
    let mut gy = f32::MAX;
    for o in keys {
        if std::ptr::eq(o, k) {
            continue;
        }
        let (ox0, oy0, ox1, oy1) = (o.x0 as f32, o.y0 as f32, o.x1 as f32, o.y1 as f32);
        if oy0 < y1 && oy1 > y0 {
            if ox0 >= x1 {
                gx = gx.min(ox0 - x1);
            } else if ox1 <= x0 {
                gx = gx.min(x0 - ox1);
            }
        }
        if ox0 < x1 && ox1 > x0 {
            if oy0 >= y1 {
                gy = gy.min(oy0 - y1);
            } else if oy1 <= y0 {
                gy = gy.min(y0 - oy1);
            }
        }
    }
    // Also bounded by the key's own size so sparse rows (large gaps) do not
    // inflate a key far beyond its siblings.
    let dx = if gx == f32::MAX {
        0.0
    } else {
        (gx * GROW).min((x1 - x0) * 0.18)
    };
    let dy = if gy == f32::MAX {
        0.0
    } else {
        (gy * GROW).min((y1 - y0) * 0.18)
    };
    (x0 - dx, y0 - dy, x1 + dx, y1 + dy)
}

fn hit_key(l: &Layout, model: Model, x: f32, y: f32) -> Option<&'static keys::CalcKey> {
    if !l.keys.contains(x, y) {
        return None;
    }
    let gx = (x - l.keys.x as f32) / l.cell_x;
    let gy = (y - l.keys.y as f32) / l.cell_y;
    model.keys.iter().find(|k| {
        let (x0, y0, x1, y1) = key_rect_units(model.keys, k);
        gx >= x0 && gx <= x1 && gy >= y0 && gy <= y1
    })
}

// ---------------------------------------------------------------------------
// Canvas
// ---------------------------------------------------------------------------

struct Canvas {
    w: usize,
    h: usize,
    buf: Vec<u32>,
}

impl Canvas {
    fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            buf: vec![BG; w * h],
        }
    }

    fn resize(&mut self, w: usize, h: usize) {
        self.w = w;
        self.h = h;
        self.buf.resize(w * h, BG);
    }

    fn fill_gradient(&mut self, top: u32, bottom: u32) {
        if self.h == 0 {
            return;
        }
        let denom = self.h.saturating_sub(1).max(1) as f32;
        for y in 0..self.h {
            let color = lerp_color(top, bottom, y as f32 / denom);
            let row = y * self.w;
            self.buf[row..row + self.w].fill(color);
        }
    }

    fn fill_rect(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, color: u32) {
        let x0 = x0.min(self.w);
        let x1 = x1.min(self.w);
        let y0 = y0.min(self.h);
        let y1 = y1.min(self.h);
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        for y in y0..y1 {
            let row = y * self.w;
            self.buf[row + x0..row + x1].fill(color);
        }
    }

    fn stroke_rect(&mut self, x0: usize, y0: usize, x1: usize, y1: usize, color: u32) {
        if x1 <= x0 || y1 <= y0 {
            return;
        }
        self.fill_rect(x0, y0, x1, y0 + 1, color);
        self.fill_rect(x0, y1 - 1, x1, y1, color);
        self.fill_rect(x0, y0, x0 + 1, y1, color);
        self.fill_rect(x1 - 1, y0, x1, y1, color);
    }

    fn draw_text(&mut self, x: usize, y: usize, text: &str, color: u32, scale: usize) {
        if scale == 0 {
            return;
        }
        let mut cx = x;
        for ch in text.chars() {
            if let Some(glyph) = font8x8::BASIC_FONTS.get(ch) {
                for (gy, bits) in glyph.iter().enumerate() {
                    for gx in 0..8 {
                        if (bits >> gx) & 1 == 0 {
                            continue;
                        }
                        for dy in 0..scale {
                            let yy = y + gy * scale + dy;
                            if yy >= self.h {
                                continue;
                            }
                            let row = yy * self.w;
                            for dx in 0..scale {
                                let xx = cx + gx * scale + dx;
                                if xx < self.w {
                                    self.buf[row + xx] = color;
                                }
                            }
                        }
                    }
                }
            }
            cx += 8 * scale;
            if cx >= self.w {
                break;
            }
        }
    }

    fn draw_text_centered(&mut self, r: Rect, text: &str, color: u32, scale: usize) {
        let tw = text.chars().count() * 8 * scale;
        let th = 8 * scale;
        let x = r.x + r.w.saturating_sub(tw) / 2;
        let y = r.y + r.h.saturating_sub(th) / 2;
        self.draw_text(x, y, text, color, scale);
    }

    fn draw_chevron(&mut self, x: usize, y: usize, size: usize, color: u32) {
        let h = (size / 2).max(2);
        for i in 0..h {
            self.fill_rect(x + i, y + i, x + size.saturating_sub(i), y + i + 1, color);
        }
    }
}

fn lerp_color(a: u32, b: u32, t: f32) -> u32 {
    let t = t.clamp(0.0, 1.0);
    let channel = |shift: u32| {
        let av = ((a >> shift) & 0xff) as f32;
        let bv = ((b >> shift) & 0xff) as f32;
        ((av + (bv - av) * t).round() as u32) << shift
    };
    channel(16) | channel(8) | channel(0)
}

// ---------------------------------------------------------------------------
// Drawing
// ---------------------------------------------------------------------------

fn draw_top_bar(c: &mut Canvas, l: &Layout, open: bool) {
    c.fill_rect(0, 0, l.bar.w, l.bar.h, PANEL);
    c.fill_rect(0, l.bar.h.saturating_sub(1), l.bar.w, l.bar.h, BORDER);

    let accent = (6 * l.btn_scale + 4).max(8);
    let ax = l.body.x;
    let ay = l.bar.h.saturating_sub(accent) / 2;
    c.fill_rect(ax, ay, ax + accent, ay + accent, ACCENT);

    if l.title_visible {
        let tx = ax + accent + 3 * l.btn_scale;
        let ty = l.bar.h.saturating_sub(8 * l.title_scale) / 2;
        c.draw_text(tx, ty, TITLE, TEXT, l.title_scale);
    }

    let bg = if open { BUTTON_HOT } else { BUTTON };
    c.fill_rect(l.btn.x, l.btn.y, l.btn.right(), l.btn.bottom(), bg);
    c.stroke_rect(l.btn.x, l.btn.y, l.btn.right(), l.btn.bottom(), BORDER);

    let arrow = 8 * l.btn_scale;
    let pad = 3 * l.btn_scale;
    let text_w = l.btn.w.saturating_sub(arrow + 2 * pad);
    let label = truncate(&l.btn_label, text_w / (8 * l.btn_scale));
    let ty = l.btn.y + l.btn.h.saturating_sub(8 * l.btn_scale) / 2;
    c.draw_text(
        l.btn.x + pad,
        ty,
        &label,
        if open { ACCENT_HI } else { TEXT },
        l.btn_scale,
    );
    let ax = l.btn.right().saturating_sub(pad + arrow);
    let ay = l.btn.y + l.btn.h.saturating_sub(arrow / 2) / 2;
    c.draw_chevron(ax, ay, arrow, MUTED);
}

fn draw_body(c: &mut Canvas, l: &Layout) {
    let b = l.body;
    c.fill_rect(b.x + 3, b.y + 4, b.right() + 3, b.bottom() + 4, SHADOW);
    c.fill_rect(b.x, b.y, b.right(), b.bottom(), BODY);
    c.stroke_rect(b.x, b.y, b.right(), b.bottom(), BORDER);

    c.fill_rect(
        l.bezel.x,
        l.bezel.y,
        l.bezel.right(),
        l.bezel.bottom(),
        LCD_BEZEL,
    );
    c.stroke_rect(
        l.bezel.x,
        l.bezel.y,
        l.bezel.right(),
        l.bezel.bottom(),
        BORDER,
    );
}

fn draw_lcd(c: &mut Canvas, lcd: &[u8], model: Model, l: &Layout) {
    c.fill_rect(l.lcd.x, l.lcd.y, l.lcd.right(), l.lcd.bottom(), LCD_BG);
    let ls = l.lcd_scale.max(1);
    let lw = model.lcd_width;
    let lh = model.lcd_height;
    for py in 0..lh {
        for px in 0..lw {
            if lcd.get(py * lw + px).copied().unwrap_or(0) == 0 {
                continue;
            }
            let x0 = l.lcd.x + px * ls;
            let y0 = l.lcd.y + py * ls;
            c.fill_rect(x0, y0, x0 + ls, y0 + ls, LCD_FG);
        }
    }
}

/// Text drawn on a cap.  The d-pad keys show arrows, which are short enough to
/// use the same text size as the rest of the keypad.
fn display_label(label: &str) -> &str {
    match label {
        "UP" => "^",
        "DOWN" => "v",
        "LEFT" => "<",
        "RIGHT" => ">",
        "PGUP" => "^^",
        "PGDN" => "vv",
        other => other,
    }
}

fn draw_keypad(c: &mut Canvas, machine: &Machine, model: Model, l: &Layout) {
    // One integer scale for most labels, reduced per label when it is too wide
    // or the keys are small.
    // One text size for the whole keypad, derived from the median cap height,
    // so a tall key (UP) does not get giant text next to its neighbours.
    // Individual labels still shrink when they are too wide for their cap.
    let mut heights: Vec<usize> = model
        .keys
        .iter()
        .map(|k| {
            let (_, uy0, _, uy1) = key_rect_units(model.keys, k);
            ((uy1 - uy0) * l.cell_y).round().max(0.0) as usize
        })
        .collect();
    heights.sort_unstable();
    let median_h = heights.get(heights.len() / 2).copied().unwrap_or(0);
    let label_cap = (median_h.saturating_sub(4) / 8).clamp(1, 4);
    for key in model.keys {
        let (ux0, uy0, ux1, uy1) = key_rect_units(model.keys, key);
        let x0 = l.keys.x + (ux0 * l.cell_x).round().max(0.0) as usize;
        let y0 = l.keys.y + (uy0 * l.cell_y).round().max(0.0) as usize;
        let x1 = l.keys.x + (ux1 * l.cell_x).round() as usize;
        let y1 = l.keys.y + (uy1 * l.cell_y).round() as usize;
        if x1 <= x0 || y1 <= y0 {
            continue;
        }
        let hot = (machine.pending_key == Some((key.ki, key.ko))
            || machine.held_key == Some((key.ki, key.ko)))
            && (key.ki != 0 || key.ko != 0);

        c.fill_rect(x0, y0, x1, y1, BORDER);
        let color = if hot { BUTTON_HOT } else { BUTTON };
        c.fill_rect(x0 + 1, y0 + 1, x1 - 1, y1 - 1, color);
        if x1 > x0 + 2 && y1 > y0 + 2 {
            c.fill_rect(
                x0 + 1,
                y0 + 1,
                x1 - 1,
                y0 + 2,
                lerp_color(color, TEXT, 0.12),
            );
        }
        if key.label == "ON" {
            c.fill_rect(x0 + 1, y0 + 1, x1 - 1, (y0 + 4).min(y1 - 1), ACCENT);
        }

        // Minimal padding so the glyphs get as much of the key as possible:
        // 1 px per side, nothing extra vertically.
        let inner = Rect {
            x: x0 + 1,
            y: y0 + 1,
            w: (x1 - x0).saturating_sub(2),
            h: (y1 - y0).saturating_sub(2),
        };
        // Largest whole-number scale whose glyph height fits the key (with a
        // pixel of air above and below), further reduced when the label is
        // too wide.
        let text = display_label(key.label);
        let scale = label_scale(text, inner.w, inner.h, label_cap);
        c.draw_text_centered(inner, text, TEXT, scale);
    }
}

fn draw_help(c: &mut Canvas, l: &Layout) {
    let scale = l.help_scale;
    let line_h = 8 * scale;
    for (i, text) in [HELP1, HELP2].iter().enumerate() {
        let max_chars = l.w.saturating_sub(2 * l.body.x) / (8 * scale).max(1);
        let line = truncate(text, max_chars);
        let x = l.w.saturating_sub(line.chars().count() * 8 * scale) / 2;
        let y = l.help_y + i * (line_h + 4);
        c.draw_text(x, y, &line, MUTED, scale);
    }
}

fn draw_dropdown(c: &mut Canvas, dd: &Dropdown, cursor: usize, current: usize) {
    let p = dd.panel;
    c.fill_rect(p.x + 3, p.y + 4, p.right() + 3, p.bottom() + 4, SHADOW);
    c.fill_rect(p.x, p.y, p.right(), p.bottom(), PANEL);
    c.stroke_rect(p.x, p.y, p.right(), p.bottom(), BORDER);

    let s = dd.scale;
    let row_h = dd.row_h;
    for offset in 0..dd.visible {
        let i = dd.first + offset;
        let Some(m) = model::ALL.get(i) else {
            break;
        };
        let ry = dd.rows_y + offset * row_h;
        if i == cursor {
            c.fill_rect(p.x + 1, ry, p.right() - 1, ry + row_h, BUTTON_HOT);
            c.fill_rect(p.x + 1, ry, p.x + 2, ry + row_h, ACCENT_HI);
        } else if i == current {
            c.fill_rect(p.x + 1, ry, p.right() - 1, ry + row_h, BUTTON);
        }

        let col = if i == current { ACCENT_HI } else { TEXT };
        let ty = ry + row_h.saturating_sub(8 * s) / 2;
        c.draw_text(p.x + 6, ty, &format!("{:>2}", i + 1), MUTED, s);

        let tag_w = 4 * 8 * s;
        let num_w = 5 * 8 * s;
        let name_x = p.x + 6 + num_w + 4 * s;
        let name_w = p
            .right()
            .saturating_sub(name_x)
            .saturating_sub(tag_w + 8 * s);
        let name = truncate(m.name.trim_start_matches("Casio "), name_w / (8 * s).max(1));
        c.draw_text(name_x, ty, &name, col, s);

        let tag = match m.board {
            BoardKind::EsPlus => "ES+",
            BoardKind::ClassWiz => "CW",
            BoardKind::ClassWizII => "CW2",
        };
        c.draw_text(p.right().saturating_sub(tag_w + 6), ty, tag, MUTED, s);
    }

    // Show that the viewport has more entries above/below.
    if dd.first > 0 {
        c.fill_rect(p.x + 1, p.y + 1, p.right() - 1, p.y + 2, ACCENT);
    }
    if dd.first + dd.visible < model::ALL.len() {
        c.fill_rect(
            p.x + 1,
            p.bottom() - 2,
            p.right() - 1,
            p.bottom() - 1,
            ACCENT,
        );
    }
}

// ---------------------------------------------------------------------------
// Application
// ---------------------------------------------------------------------------

struct App {
    rom: Vec<u8>,
    model: Model,
    machine: Machine,
    cpu: Cpu,
    window: Option<Arc<Window>>,
    context: Option<Context<Arc<Window>>>,
    surface: Option<Surface<Arc<Window>, Arc<Window>>>,
    canvas: Canvas,
    lcd: Vec<u8>,
    last_lcd: Vec<u8>,
    dirty: bool,
    show_list: bool,
    list_cursor: usize,
    cursor: Option<(f32, f32)>,
    mouse_down: bool,
    active_touch: Option<u64>,
    suppress_mouse_until: Instant,
    pressed: Option<(u8, u8)>,
    release_at: Option<Instant>,
    force_steps: u32,
    modifiers: ModifiersState,
    occluded: bool,
    next_tick: Instant,
}

impl App {
    fn new(rom: Vec<u8>, model: Model, ram: Option<&[u8]>) -> Self {
        let mut machine = Machine::with_model(rom.clone(), model);
        if let Some(ram) = ram {
            machine.load_ram(ram);
        }
        let mut cpu = Cpu::new();
        cpu.reset(&mut machine);
        let lcd = vec![0u8; model.lcd_width * model.lcd_height];
        let app = Self {
            rom,
            model,
            machine,
            cpu,
            window: None,
            context: None,
            surface: None,
            canvas: Canvas::new(1, 1),
            lcd: lcd.clone(),
            last_lcd: lcd,
            dirty: true,
            show_list: false,
            list_cursor: model_index(model),
            cursor: None,
            mouse_down: false,
            active_touch: None,
            suppress_mouse_until: Instant::now(),
            pressed: None,
            release_at: None,
            force_steps: 0,
            modifiers: ModifiersState::empty(),
            occluded: false,
            next_tick: Instant::now(),
        };
        app.refresh_title();
        app
    }

    fn window_size(&self) -> (usize, usize) {
        self.window
            .as_ref()
            .map(|w| {
                let s = w.inner_size();
                (s.width as usize, s.height as usize)
            })
            .unwrap_or((0, 0))
    }

    fn layout(&self) -> Layout {
        let (w, h) = self.window_size();
        compute_layout(self.model, w, h)
    }

    fn refresh_title(&self) {
        if let Some(window) = &self.window {
            window.set_title(&format!("{} - casio-emu-rs", self.model.name));
        }
    }

    fn resize_surface(&mut self, size: PhysicalSize<u32>) {
        if let Some(surface) = self.surface.as_mut() {
            if let (Some(w), Some(h)) = (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
            {
                let _ = surface.resize(w, h);
            }
        }
        if size.width > 0 && size.height > 0 {
            self.canvas
                .resize(size.width as usize, size.height as usize);
        }
        self.dirty = true;
    }

    fn press(&mut self, ki: u8, ko: u8) {
        self.machine.press(ki, ko);
        self.pressed = Some((ki, ko));
        self.release_at = None;
        // Give the firmware a short burst so the matrix scan runs promptly.
        self.force_steps = self.force_steps.max(12_000);
        self.dirty = true;
    }

    fn schedule_release(&mut self, hold: Duration) {
        if self.pressed.is_some() {
            self.release_at = Some(Instant::now() + hold);
        }
    }

    fn pointer_down(&mut self, x: f32, y: f32) {
        let l = self.layout();
        if self.show_list {
            let dd = dropdown_layout(&l, self.list_cursor);
            if let Some(row) = dropdown_row(&dd, x, y, model::ALL.len()) {
                self.select_model(row);
            } else if !dd.panel.contains(x, y) {
                self.show_list = false;
                self.dirty = true;
            }
            return;
        }
        if l.btn.contains(x, y) {
            self.show_list = true;
            self.list_cursor = model_index(self.model);
            self.dirty = true;
            return;
        }
        if let Some(key) = hit_key(&l, self.model, x, y) {
            if key.label == "ON" {
                self.cpu.reset(&mut self.machine);
                self.dirty = true;
            } else if key.ki != 0 || key.ko != 0 {
                self.press(key.ki, key.ko);
            }
        }
    }

    fn pointer_up(&mut self) {
        self.mouse_down = false;
        self.schedule_release(KEY_HOLD);
    }

    fn select_model(&mut self, index: usize) {
        self.show_list = false;
        let index = index.min(model::ALL.len() - 1);
        if model::ALL[index].id == self.model.id {
            self.list_cursor = index;
            self.dirty = true;
            return;
        }
        let new_model = model::ALL[index];
        let candidate = PathBuf::from("roms").join(new_model.rom_filename());
        let image = match std::fs::read(&candidate) {
            Ok(image) => {
                eprintln!("loaded {}", candidate.display());
                image
            }
            Err(_) => self.rom.clone(),
        };
        self.model = new_model;
        self.machine = Machine::with_model(image, new_model);
        self.cpu = Cpu::new();
        self.cpu.reset(&mut self.machine);
        self.pressed = None;
        self.release_at = None;
        let len = new_model.lcd_width * new_model.lcd_height;
        self.lcd.resize(len, 0);
        self.lcd.fill(0);
        self.last_lcd.resize(len, 0);
        self.last_lcd.fill(0);
        self.list_cursor = index;
        self.refresh_title();
        self.dirty = true;
    }

    fn cycle_model(&mut self, delta: isize) {
        let count = model::ALL.len() as isize;
        let current = model_index(self.model) as isize;
        let next = (current + delta).rem_euclid(count) as usize;
        self.select_model(next);
    }

    fn handle_key(&mut self, event: winit::event::KeyEvent, root: &ActiveEventLoop) {
        let pressed = event.state == ElementState::Pressed;
        let PhysicalKey::Code(code) = event.physical_key else {
            return;
        };

        if pressed && code == KeyCode::Escape {
            if self.show_list {
                self.show_list = false;
                self.dirty = true;
            } else {
                root.exit();
            }
            return;
        }
        if pressed && code == KeyCode::F8 {
            if !event.repeat {
                self.show_list = !self.show_list;
                self.list_cursor = model_index(self.model);
                self.dirty = true;
            }
            return;
        }
        if pressed && code == KeyCode::F6 {
            if !event.repeat {
                self.cycle_model(1);
            }
            return;
        }
        if pressed && code == KeyCode::F7 {
            if !event.repeat {
                self.cycle_model(-1);
            }
            return;
        }

        if self.show_list {
            if !pressed {
                return;
            }
            match code {
                KeyCode::ArrowUp => {
                    self.list_cursor = (self.list_cursor + model::ALL.len() - 1) % model::ALL.len();
                    self.dirty = true;
                }
                KeyCode::ArrowDown => {
                    self.list_cursor = (self.list_cursor + 1) % model::ALL.len();
                    self.dirty = true;
                }
                KeyCode::Enter | KeyCode::NumpadEnter => self.select_model(self.list_cursor),
                _ => {}
            }
            return;
        }

        let Some(label) = key_label(code, self.modifiers.shift_key()) else {
            return;
        };
        if !pressed {
            self.schedule_release(KEY_HOLD);
            return;
        }
        if event.repeat {
            return;
        }
        if label == "ON" {
            self.cpu.reset(&mut self.machine);
            self.dirty = true;
            return;
        }
        if let Some((ki, ko)) = keys::lookup(self.model.keys, label) {
            self.press(ki, ko);
        }
    }

    fn handle_touch(&mut self, touch: winit::event::Touch) {
        self.suppress_mouse_until = Instant::now() + Duration::from_millis(400);
        match touch.phase {
            TouchPhase::Started => {
                if self.active_touch.is_none() {
                    self.active_touch = Some(touch.id);
                    self.cursor = Some((touch.location.x as f32, touch.location.y as f32));
                    self.pointer_down(touch.location.x as f32, touch.location.y as f32);
                }
            }
            TouchPhase::Moved => {
                if self.active_touch == Some(touch.id) {
                    self.cursor = Some((touch.location.x as f32, touch.location.y as f32));
                }
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                if self.active_touch == Some(touch.id) {
                    self.active_touch = None;
                    self.mouse_down = false;
                    // Touch taps are often shorter than one matrix scan; hold
                    // the key a little longer than the physical contact.
                    self.schedule_release(MIN_TOUCH_HOLD);
                }
            }
        }
        self.dirty = true;
    }

    fn tick(&mut self) {
        let mut steps = 0u32;
        let deadline = Instant::now() + TICK_BUDGET;
        while steps < MAX_STEPS {
            self.machine.tick(&mut self.cpu);
            steps += 1;
            if steps.is_multiple_of(2048) && Instant::now() >= deadline {
                break;
            }
        }
        if self.force_steps > steps {
            let extra = (self.force_steps - steps).min(20_000);
            for _ in 0..extra {
                self.machine.tick(&mut self.cpu);
            }
        }
        self.force_steps = 0;

        self.machine.render_into(&mut self.lcd);
        if self.lcd != self.last_lcd {
            self.last_lcd.copy_from_slice(&self.lcd);
            self.dirty = true;
        }
    }

    fn render(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        let size = window.inner_size();
        let (w, h) = (size.width as usize, size.height as usize);
        if w == 0 || h == 0 {
            return;
        }
        if self.canvas.w != w || self.canvas.h != h {
            self.canvas.resize(w, h);
        }

        let l = compute_layout(self.model, w, h);
        self.canvas.fill_gradient(BG_TOP, BG);
        draw_body(&mut self.canvas, &l);
        draw_lcd(&mut self.canvas, &self.lcd, self.model, &l);
        draw_keypad(&mut self.canvas, &self.machine, self.model, &l);
        draw_top_bar(&mut self.canvas, &l, self.show_list);
        draw_help(&mut self.canvas, &l);
        if self.show_list {
            let dd = dropdown_layout(&l, self.list_cursor);
            draw_dropdown(
                &mut self.canvas,
                &dd,
                self.list_cursor,
                model_index(self.model),
            );
        }

        if let Some(surface) = self.surface.as_mut() {
            if let Ok(mut buffer) = surface.buffer_mut() {
                let n = buffer.len().min(self.canvas.buf.len());
                buffer[..n].copy_from_slice(&self.canvas.buf[..n]);
                let _ = buffer.present();
            }
        }
        self.dirty = false;
    }
}

fn model_index(model: Model) -> usize {
    model::ALL
        .iter()
        .position(|m| m.id == model.id)
        .unwrap_or(0)
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let (pw, ph) = preferred_size(self.model);
        let (max_w, max_h) = event_loop
            .primary_monitor()
            .map(|m| {
                let s = m.size();
                let factor = m.scale_factor();
                (s.width as f64 / factor, s.height as f64 / factor)
            })
            .unwrap_or((1280.0, 800.0));
        let fit = ((max_w * 0.9) / pw)
            .min((max_h * 0.86) / ph)
            .clamp(0.5, 1.4);
        let w = (pw * fit).max(360.0);
        let h = (ph * fit).max(460.0);

        let attributes = Window::default_attributes()
            .with_title(format!("{} - casio-emu-rs", self.model.name))
            .with_inner_size(LogicalSize::new(w, h))
            .with_min_inner_size(LogicalSize::new(360.0, 460.0));
        let window = Arc::new(
            event_loop
                .create_window(attributes)
                .expect("cannot create window"),
        );
        let context = Context::new(window.clone()).expect("cannot create softbuffer context");
        let surface =
            Surface::new(&context, window.clone()).expect("cannot create softbuffer surface");
        self.window = Some(window);
        self.context = Some(context);
        self.surface = Some(surface);

        let size = self.window.as_ref().unwrap().inner_size();
        self.resize_surface(size);
        self.refresh_title();
        self.dirty = true;
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => self.resize_surface(size),
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(window) = &self.window {
                    self.resize_surface(window.inner_size());
                }
            }
            WindowEvent::RedrawRequested => self.render(),
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers = modifiers.state();
            }
            WindowEvent::KeyboardInput { event, .. } => self.handle_key(event, event_loop),
            WindowEvent::CursorMoved { position, .. } => {
                self.cursor = Some((position.x as f32, position.y as f32));
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                if Instant::now() < self.suppress_mouse_until {
                    return;
                }
                let Some((x, y)) = self.cursor else {
                    return;
                };
                match state {
                    ElementState::Pressed => {
                        self.mouse_down = true;
                        self.pointer_down(x, y);
                    }
                    ElementState::Released => {
                        if self.mouse_down {
                            self.pointer_up();
                        }
                    }
                }
            }
            WindowEvent::Touch(touch) => self.handle_touch(touch),
            WindowEvent::Focused(false) => {
                // Losing focus while a key is held would otherwise leave the
                // matrix stuck until another press.
                self.machine.release();
                self.pressed = None;
                self.release_at = None;
                self.mouse_down = false;
                self.dirty = true;
            }
            WindowEvent::Occluded(occluded) => self.occluded = occluded,
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            event_loop.set_control_flow(ControlFlow::Wait);
            return;
        }
        let now = Instant::now();
        let mut need_tick = self.force_steps > 0;

        if let Some(at) = self.release_at {
            if now >= at {
                self.machine.release();
                self.pressed = None;
                self.release_at = None;
                self.dirty = true;
                need_tick = true;
            }
        }

        if now >= self.next_tick {
            need_tick = true;
        }

        if need_tick {
            // Advance no faster than the frame cadence.  Mouse-motion and
            // redraw events therefore do not trigger extra emulation bursts.
            self.next_tick = now + FRAME_TIME;
            if !self.occluded {
                self.tick();
            }
        }

        if self.dirty {
            if let Some(window) = &self.window {
                window.request_redraw();
            }
        }

        let mut wake = self.next_tick;
        if let Some(at) = self.release_at {
            wake = wake.min(at);
        }
        event_loop.set_control_flow(ControlFlow::WaitUntil(wake));
    }
}

// ---------------------------------------------------------------------------
// Keyboard mapping
// ---------------------------------------------------------------------------

fn key_label(code: KeyCode, shift: bool) -> Option<&'static str> {
    Some(match code {
        KeyCode::Digit0 | KeyCode::Numpad0 => "0",
        KeyCode::Digit1 | KeyCode::Numpad1 => "1",
        KeyCode::Digit2 | KeyCode::Numpad2 => "2",
        KeyCode::Digit3 | KeyCode::Numpad3 => "3",
        KeyCode::Digit4 | KeyCode::Numpad4 => "4",
        KeyCode::Digit5 | KeyCode::Numpad5 => "5",
        KeyCode::Digit6 | KeyCode::Numpad6 => "6",
        KeyCode::Digit7 | KeyCode::Numpad7 => "7",
        KeyCode::Digit8 | KeyCode::Numpad8 => "8",
        KeyCode::Digit9 | KeyCode::Numpad9 => "9",
        KeyCode::Period | KeyCode::NumpadDecimal => ".",
        KeyCode::Minus | KeyCode::NumpadSubtract => "-",
        KeyCode::Equal if shift => "+",
        KeyCode::Equal | KeyCode::NumpadAdd => "=",
        KeyCode::Slash | KeyCode::NumpadDivide => "/",
        KeyCode::Enter | KeyCode::NumpadEnter => "=",
        KeyCode::Backspace => "DEL",
        KeyCode::Delete => "AC",
        KeyCode::ArrowLeft => "LEFT",
        KeyCode::ArrowRight => "RIGHT",
        KeyCode::ArrowUp => "UP",
        KeyCode::ArrowDown => "DOWN",
        KeyCode::KeyA => "Abs",
        KeyCode::KeyB => "a/b",
        KeyCode::KeyC => "cos",
        KeyCode::KeyD => "dms",
        KeyCode::KeyE => "ENG",
        KeyCode::KeyH => "hyp",
        KeyCode::KeyL => "ln",
        KeyCode::KeyM => "M+",
        KeyCode::KeyN => "xn",
        KeyCode::KeyO => "log",
        KeyCode::KeyP => "x2",
        KeyCode::KeyR => "RCL",
        KeyCode::KeyS => "sin",
        KeyCode::KeyT => "tan",
        KeyCode::KeyX => "X",
        KeyCode::KeyY => "x10",
        KeyCode::KeyZ => "x-1",
        KeyCode::F1 => "MODE",
        KeyCode::F2 => "SHIFT",
        KeyCode::F3 => "ALPHA",
        KeyCode::F4 => "ON",
        KeyCode::F5 => "sqrt",
        _ => return None,
    })
}

pub fn run_gui(rom: Vec<u8>, model: Model, ram: Option<&[u8]>) {
    let event_loop = EventLoop::new().expect("cannot create event loop");
    event_loop.set_control_flow(ControlFlow::Wait);
    let mut app = App::new(rom, model, ram);
    event_loop.run_app(&mut app).expect("event loop failed");
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod layout_tests {
    use super::*;

    const SIZES: &[(usize, usize)] = &[
        (360, 460),
        (480, 560),
        (478, 802),
        (720, 920),
        (956, 1040),
        (1280, 900),
        (1920, 1080),
    ];

    #[test]
    fn lcd_dots_are_always_square_integer_blocks() {
        for m in model::ALL.iter().copied() {
            for &(w, h) in SIZES {
                let l = compute_layout(m, w, h);
                assert_eq!(l.lcd.w, m.lcd_width * l.lcd_scale);
                assert_eq!(l.lcd.h, m.lcd_height * l.lcd_scale);
                assert!(l.lcd_scale >= 1);
            }
        }
    }

    #[test]
    fn content_stays_inside_the_window() {
        for m in model::ALL.iter().copied() {
            for &(w, h) in SIZES {
                let l = compute_layout(m, w, h);
                assert!(l.bar.w <= w && l.bar.h <= h);
                assert!(l.body.right() <= w, "{} body right", m.id);
                assert!(l.body.bottom() <= h, "{} body bottom", m.id);
                assert!(l.lcd.right() <= w, "{} lcd right", m.id);
                assert!(l.keys.bottom() <= h, "{} keys bottom", m.id);
                assert!(l.btn.right() <= w, "{} button right", m.id);
            }
        }
    }

    #[test]
    fn keypad_is_narrower_than_the_screen_but_stretches_horizontally() {
        for m in model::ALL.iter().copied() {
            for &(w, h) in SIZES {
                let l = compute_layout(m, w, h);
                assert!(
                    l.keys.w < l.bezel.w || l.bezel.w <= 2 * 3,
                    "{} keypad {} not narrower than screen {} at {w}x{h}",
                    m.id,
                    l.keys.w,
                    l.bezel.w
                );
                assert!(l.keys.x >= l.bezel.x && l.keys.right() <= l.bezel.right());
            }
        }
    }

    #[test]
    fn classwiz2_dpad_uses_its_own_matrix() {
        let m = model::ALL.iter().find(|m| m.id == "fx991cncw").unwrap();
        let get = |l: &str| keys::lookup(m.keys, l).unwrap();
        assert_eq!(get("UP"), (0x80, 0x08));
        assert_eq!(get("DOWN"), (0x20, 0x08));
        assert_eq!(get("LEFT"), (0x40, 0x04));
        assert_eq!(get("RIGHT"), (0x40, 0x10));
    }

    #[test]
    fn keypad_hit_testing_round_trips() {
        for m in model::ALL.iter().copied() {
            let l = compute_layout(m, 640, 760);
            for key in m.keys {
                let cx = l.keys.x as f32 + (key.x0 + key.x1) as f32 * 0.5 * l.cell_x;
                let cy = l.keys.y as f32 + (key.y0 + key.y1) as f32 * 0.5 * l.cell_y;
                if l.keys.contains(cx, cy) {
                    let hit = hit_key(&l, m, cx, cy);
                    assert!(hit.is_some(), "{} key {} missed", m.id, key.label);
                }
            }
        }
    }

    #[test]
    fn dropdown_viewport_follows_the_cursor() {
        let l = compute_layout(model::ALL[0], 360, 460);
        let last = model::ALL.len() - 1;
        let dd = dropdown_layout(&l, last);
        assert!(dd.first + dd.visible > last);
        let row = dropdown_row(
            &dd,
            dd.panel.x as f32 + 2.0,
            (dd.rows_y + (dd.visible - 1) * dd.row_h) as f32 + 1.0,
            model::ALL.len(),
        );
        assert_eq!(row, Some(last));
    }

    #[test]
    fn text_scales_are_whole_numbers() {
        for h in [300, 460, 520, 760, 1040, 2080] {
            let s = text_scale(h);
            assert!(s >= 1);
        }
        assert!(text_scale(520) <= text_scale(1040));
    }

    #[test]
    fn dropdown_rows_are_in_bounds() {
        for &(w, h) in SIZES {
            let l = compute_layout(model::ALL[0], w, h);
            let dd = dropdown_layout(&l, 0);
            assert!(dd.panel.right() <= w);
            assert!(dd.panel.bottom() <= h);
            let row = dropdown_row(
                &dd,
                dd.panel.x as f32 + 2.0,
                dd.rows_y as f32 + 1.0,
                model::ALL.len(),
            );
            assert_eq!(row, Some(0));
        }
    }
}
