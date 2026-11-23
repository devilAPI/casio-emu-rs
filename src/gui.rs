//! Desktop frontend: minifb window, optional device skin, keyboard/mouse input.

use casio_emu_rs::cpu::Cpu;
use casio_emu_rs::keys;
use casio_emu_rs::machine::Machine;
use casio_emu_rs::model::{self, BoardKind, Model};
use font8x8::UnicodeFonts;
use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};
use std::path::{Path, PathBuf};

/// Horizontal *and* vertical size of one logical keypad cell.  Both axes must
/// use the same value or the keypad is stretched (and the bottom rows fall off
/// the canvas).
const KEY_SCALE: usize = 10;
const PAD_X: usize = 24;
const PAD_TOP: usize = 12;
const PAD_GAP: usize = 22;
const PAD_BOTTOM: usize = 52;

struct Layout {
    w: usize,
    h: usize,
    lcd_w: usize,
    lcd_h: usize,
    key_w: usize,
    key_h: usize,
}

fn key_grid(model: Model) -> (usize, usize) {
    if matches!(model.board, BoardKind::ClassWiz | BoardKind::ClassWizII) {
        (keys::VNX_GRID_W as usize, keys::VNX_GRID_H as usize)
    } else {
        (keys::GRID_W as usize, 28)
    }
}

/// Integer LCD-dot scale that keeps the display slightly narrower than the
/// keypad, like the real calculators.  Using whole numbers means every LCD dot
/// occupies the same number of pixels, which avoids shimmering.
fn lcd_scale(model: Model) -> usize {
    let (gw, _) = key_grid(model);
    let key_w = gw * KEY_SCALE;
    ((key_w as f32 * 0.88 / model.lcd_width as f32).round() as usize).clamp(2, 4)
}

/// Natural canvas size needed by a model, including the full keypad.
fn layout(model: Model) -> Layout {
    let (gw, gh) = key_grid(model);
    let ls = lcd_scale(model);
    let lcd_w = model.lcd_width * ls;
    let lcd_h = model.lcd_height * ls;
    let key_w = gw * KEY_SCALE;
    let key_h = gh * KEY_SCALE;
    Layout {
        w: lcd_w.max(key_w) + PAD_X * 2,
        h: BAR_H + PAD_TOP + lcd_h + PAD_GAP + key_h + PAD_BOTTOM,
        lcd_w,
        lcd_h,
        key_w,
        key_h,
    }
}

/// Canvas size that fits every registered model (kept for reference by the
/// layout tests); the window itself is sized to the active model.
#[cfg(test)]
fn max_canvas() -> (usize, usize) {
    model::ALL
        .iter()
        .copied()
        .map(layout)
        .fold((0, 0), |(w, h), l| (w.max(l.w), h.max(l.h)))
}

// Soft, low-contrast "dusk" palette: tinted warm charcoal, muted sand accent
// and a sage LCD instead of the harsh black/teal/white combination.
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

const BAR_H: usize = 30;
const MODEL_BTN_H: usize = 20;

pub fn default_skin_candidates() -> Vec<PathBuf> {
    vec![
        PathBuf::from("assets/skin.png"),
        PathBuf::from("assets/skin.bmp"),
        PathBuf::from("skin.png"),
        PathBuf::from("skin.bmp"),
    ]
}

/// `ram` is an optional raw RAM snapshot applied only to the initial model;
/// switching models with F6/F7 starts that model with fresh zeroed RAM.
/// `zoom` opens the window at a whole-number multiple of the design size so
/// the bitmap UI stays crisp on high-density displays.
pub fn run_gui(
    rom: Vec<u8>,
    model: Model,
    skin_path: Option<PathBuf>,
    ram: Option<&[u8]>,
    zoom: usize,
) {
    let mut model = model;
    let mut model_index: isize = model::ALL
        .iter()
        .position(|m| m.id == model.id)
        .unwrap_or(0) as isize;
    let mut list_cursor = model_index as usize;
    let mut show_list = false;
    let mut prev_esc = false;

    let mut machine = Machine::with_model(rom.clone(), model);
    if let Some(ram) = ram {
        machine.load_ram(ram);
    }
    let mut cpu = Cpu::new();
    cpu.reset(&mut machine);

    let skin = load_skin(skin_path.as_deref());
    // Without a skin, open the window at exactly the current model's natural
    // canvas size.  `zoom` multiplies that by a whole number so the bitmap UI
    // is never resampled on high-density or large displays.
    let zoom = zoom.clamp(1, 8);
    let (win_w, win_h) = match &skin {
        Some(skin) => (skin.width * zoom, skin.height * zoom),
        None => {
            let l = layout(model);
            (l.w * zoom, l.h * zoom)
        }
    };

    let mut window = Window::new(
        &format!("{} - casio-emu-rs", model.name),
        win_w,
        win_h,
        WindowOptions {
            resize: true,
            ..WindowOptions::default()
        },
    )
    .expect("cannot create window");
    window.set_target_fps(60);

    // Without a device skin the canvas tracks the current model, so each
    // calculator is laid out at its natural proportions and then scaled to the
    // window as a whole.
    let mut canvas = match &skin {
        Some(skin) => Canvas::new(skin.width, skin.height),
        None => {
            let l = layout(model);
            Canvas::new(l.w, l.h)
        }
    };
    let mut view: Vec<u32> = Vec::new();
    let mut last_keys: Vec<Key> = Vec::new();
    let mut mouse_down = false;

    while window.is_open() {
        let l = layout(model);
        let lcd_x = canvas.w.saturating_sub(l.lcd_w) / 2;
        let lcd_y = BAR_H + PAD_TOP;
        let key_x = canvas.w.saturating_sub(l.key_w) / 2;
        let key_y = lcd_y + l.lcd_h + PAD_GAP;
        let (btn_x, btn_w) = model_button(canvas.w);

        // --- render ---------------------------------------------------------
        if let Some(skin) = &skin {
            skin.draw(&mut canvas);
            draw_lcd_rect(&mut canvas, &machine.render(), model, skin.lcd_rect);
        } else {
            canvas.fill_gradient(BG_TOP, BG);
            draw_device_body(&mut canvas, lcd_x, lcd_y, key_x, key_y, &l);
            draw_lcd_fallback(
                &mut canvas,
                &machine.render(),
                model,
                lcd_x,
                lcd_y,
                l.lcd_w,
                l.lcd_h,
            );
            draw_keypad(&mut canvas, &machine, model, key_x, key_y);
        }
        draw_top_bar(&mut canvas, btn_x, btn_w, model, show_list);
        if show_list {
            let panel = list_panel(canvas.w, canvas.h);
            draw_model_list(&mut canvas, panel, list_cursor, model_index as usize);
        }

        // --- input ----------------------------------------------------------
        let esc_now = window.is_key_down(Key::Escape);
        if esc_now && !prev_esc && !show_list {
            break;
        }
        if show_list && esc_now && !prev_esc {
            show_list = false;
        }
        prev_esc = esc_now;

        let pressed = collect_pressed_keys(&window, &last_keys);
        let mut switch = 0isize;
        let mut select = false;

        if show_list {
            for key in &pressed {
                match key {
                    Key::F8 => show_list = false,
                    Key::Up => {
                        list_cursor = (list_cursor + model::ALL.len() - 1) % model::ALL.len()
                    }
                    Key::Down => list_cursor = (list_cursor + 1) % model::ALL.len(),
                    Key::Enter => select = true,
                    _ => {}
                }
            }
            if select {
                switch = list_cursor as isize - model_index;
                show_list = false;
            }
        } else {
            for key in &pressed {
                match key {
                    Key::F8 => show_list = true,
                    Key::F6 => switch += 1,
                    Key::F7 => switch -= 1,
                    _ => {}
                }
            }

            // Calculator keys: only when the picker is closed.
            if switch == 0 && !show_list {
                let shift =
                    window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
                for key in &pressed {
                    let label = if *key == Key::Equal && shift {
                        Some("+")
                    } else {
                        key_label(*key)
                    };
                    if let Some(label) = label {
                        if label == "ON" {
                            cpu.reset(&mut machine);
                        } else if let Some((ki, ko)) = keys::pc_key(label) {
                            machine.press(ki, ko);
                        }
                    }
                }
            }
        }
        let currently_down = collect_down_keys(&window);

        // Mouse.  Map window coordinates back through the view transform so
        // hitboxes stay aligned with what is drawn at any window size.
        let (ww, wh) = window.get_size();
        let transform = view_transform(canvas.w, canvas.h, ww, wh);
        let mouse = window
            .get_mouse_pos(MouseMode::Discard)
            .and_then(|(mx, my)| window_to_canvas(&transform, mx, my));
        if let Some((mx, my)) = mouse {
            let down = window.get_mouse_down(MouseButton::Left);
            if down && !mouse_down {
                if show_list {
                    let panel = list_panel(canvas.w, canvas.h);
                    if point_in(mx, my, panel) {
                        if let Some(row) = list_row_at(panel, mx, my, model::ALL.len()) {
                            list_cursor = row;
                            switch = row as isize - model_index;
                            show_list = false;
                        }
                    } else {
                        show_list = false;
                    }
                } else if (4..4 + MODEL_BTN_H).contains(&(my as usize))
                    && (btn_x..btn_x + btn_w).contains(&(mx as usize))
                {
                    show_list = true;
                } else {
                    let key = if let Some(skin) = &skin {
                        skin.hit_key(mx, my, model)
                    } else {
                        hit_key_fallback(mx, my, model, key_x, key_y)
                    };
                    if let Some(key) = key {
                        if key.label == "ON" {
                            cpu.reset(&mut machine);
                        } else if key.ki != 0 || key.ko != 0 {
                            machine.press(key.ki, key.ko);
                        }
                    }
                }
                mouse_down = true;
            } else if !down && mouse_down {
                machine.release();
                mouse_down = false;
            }
        }

        // Apply a model change.  Load the matching ROM from roms/ if present.
        if switch != 0 {
            let count = model::ALL.len() as isize;
            model_index = (model_index + switch).rem_euclid(count);
            list_cursor = model_index as usize;
            model = model::ALL[model_index as usize];
            let candidate = PathBuf::from("roms").join(model.rom_filename());
            let image = std::fs::read(&candidate).unwrap_or_else(|_| rom.clone());
            if candidate.exists() {
                eprintln!("loaded {}", candidate.display());
            }
            machine = Machine::with_model(image, model);
            cpu = Cpu::new();
            cpu.reset(&mut machine);
            if skin.is_none() {
                // minifb 0.28 cannot resize an existing window, so recreate it
                // at the new model's natural size and keep its old position.
                let l = layout(model);
                canvas = Canvas::new(l.w, l.h);
                let pos = window.get_position();
                // Drop the old window first: minifb's X11/Wayland backends are
                // not designed to host two windows at once.
                drop(window);
                window = Window::new(
                    &format!("{} - casio-emu-rs", model.name),
                    l.w * zoom,
                    l.h * zoom,
                    WindowOptions {
                        resize: true,
                        ..WindowOptions::default()
                    },
                )
                .expect("cannot create window");
                window.set_position(pos.0, pos.1);
                window.set_target_fps(60);
                view.clear();
            } else {
                window.set_title(&format!("{} - casio-emu-rs", model.name));
            }
        }

        if currently_down.is_empty() && !last_keys.is_empty() {
            machine.release();
        }
        last_keys = currently_down;

        for _ in 0..120_000 {
            machine.tick(&mut cpu);
        }

        let (ww, wh) = window.get_size();
        if ww == 0 || wh == 0 {
            continue;
        }
        if view.len() != ww * wh {
            view.resize(ww * wh, BG);
        }
        blit_view(
            &canvas,
            &mut view,
            &view_transform(canvas.w, canvas.h, ww, wh),
        );
        window
            .update_with_buffer(&view, ww, wh)
            .expect("cannot update window");
    }
}

fn model_button(width: usize) -> (usize, usize) {
    let w = 300usize.min(width.saturating_sub(16));
    (width.saturating_sub(w + 8), w)
}

fn draw_top_bar(canvas: &mut Canvas, btn_x: usize, btn_w: usize, model: Model, open: bool) {
    canvas.fill_rect(0, 0, canvas.w, BAR_H, PANEL);
    canvas.fill_rect(0, BAR_H - 1, canvas.w, BAR_H, BORDER);
    // A small accent square rather than a long subtitle, so the title and the
    // model button never collide on narrow canvases.
    canvas.fill_rect(10, 11, 18, 19, ACCENT);
    canvas.draw_text(26, 11, "casio-emu-rs", TEXT, 1);

    let c = if open { BUTTON_HOT } else { BUTTON };
    canvas.fill_rect(btn_x, 4, btn_x + btn_w, 4 + MODEL_BTN_H, c);
    canvas.fill_rect(btn_x, 4, btn_x + btn_w, 5, BORDER);
    canvas.fill_rect(
        btn_x,
        3 + MODEL_BTN_H,
        btn_x + btn_w,
        4 + MODEL_BTN_H,
        BORDER,
    );
    let name = model.name.trim_start_matches("Casio ");
    let label = format!("MODEL: {} [Change]", name);
    let max_chars = (btn_w.saturating_sub(10)) / 8;
    let text: String = label.chars().take(max_chars).collect();
    canvas.draw_text(btn_x + 5, 8, &text, if open { ACCENT_HI } else { TEXT }, 1);
}

fn list_panel(w: usize, h: usize) -> (usize, usize, usize, usize) {
    let pw = 520usize.min(w.saturating_sub(16));
    let ph = 412usize.min(h.saturating_sub(16));
    ((w - pw) / 2, (h - ph) / 2, pw, ph)
}

fn point_in(mx: f32, my: f32, p: (usize, usize, usize, usize)) -> bool {
    let (x, y, w, h) = p;
    (x as f32..(x + w) as f32).contains(&mx) && (y as f32..(y + h) as f32).contains(&my)
}

fn list_row_at(p: (usize, usize, usize, usize), mx: f32, my: f32, count: usize) -> Option<usize> {
    let (x, y, w, h) = p;
    let rows_y = y + 42;
    let row_h = (h - 46) / count;
    if mx < (x + 4) as f32 || mx >= (x + w - 4) as f32 {
        return None;
    }
    let row = ((my - rows_y as f32) / row_h as f32) as isize;
    if row >= 0 && (row as usize) < count {
        Some(row as usize)
    } else {
        None
    }
}

fn draw_model_list(
    canvas: &mut Canvas,
    p: (usize, usize, usize, usize),
    cursor: usize,
    current: usize,
) {
    let (x, y, w, h) = p;
    canvas.fill_rect(
        x.saturating_sub(2),
        y.saturating_sub(2),
        x + w + 2,
        y + h + 2,
        BORDER,
    );
    canvas.fill_rect(x, y, x + w, y + h, PANEL);
    canvas.fill_rect(x, y, x + w, y + 24, ACCENT);
    canvas.draw_text(x + 8, y + 8, "CASIO MODELS", BG, 1);
    canvas.draw_text(x + w - 170, y + 8, "Up/Down + Enter, Esc", BG, 1);
    canvas.draw_text(x + 8, y + 29, "Click a row to load that model.", MUTED, 1);

    let rows_y = y + 42;
    let row_h = (h - 46) / model::ALL.len();
    for (i, m) in model::ALL.iter().enumerate() {
        let ry = rows_y + i * row_h;
        let ty = ry + (row_h.saturating_sub(8)) / 2;
        if i == cursor {
            canvas.fill_rect(x + 4, ry, x + w - 4, ry + row_h - 1, BUTTON_HOT);
            canvas.fill_rect(x + 4, ry, x + 6, ry + row_h - 1, ACCENT_HI);
        } else if i == current {
            canvas.fill_rect(x + 4, ry, x + w - 4, ry + row_h - 1, BUTTON);
        }
        let col = if i == current { ACCENT_HI } else { TEXT };
        canvas.draw_text(x + 10, ty, &format!("{:>2}", i + 1), MUTED, 1);
        let name = m.name.trim_start_matches("Casio ");
        let max = (w.saturating_sub(96)) / 8;
        let label: String = name.chars().take(max).collect();
        canvas.draw_text(x + 34, ty, &label, col, 1);
        let tag = match m.board {
            BoardKind::EsPlus => "ES+",
            BoardKind::ClassWiz => "CW",
            BoardKind::ClassWizII => "CW2",
        };
        canvas.draw_text(x + w - 34, ty, tag, MUTED, 1);
    }
}

fn load_skin(path: Option<&Path>) -> Option<Skin> {
    let candidates: Vec<PathBuf> = match path {
        Some(p) => vec![p.to_path_buf()],
        None => default_skin_candidates(),
    };
    for p in candidates {
        if p.exists() {
            if let Some(skin) = Skin::load(&p) {
                eprintln!("loaded device skin {}", p.display());
                return Some(skin);
            }
            eprintln!("could not decode device skin {}", p.display());
        }
    }
    None
}

struct Skin {
    width: usize,
    height: usize,
    pixels: Vec<u32>,
    lcd_rect: (usize, usize, usize, usize),
    keypad_rect: (usize, usize, usize, usize),
}

impl Skin {
    fn load(path: &Path) -> Option<Self> {
        let img = image::open(path).ok()?.to_rgba8();
        let (w, h) = img.dimensions();
        let mut pixels = Vec::with_capacity((w * h) as usize);
        for px in img.pixels() {
            pixels.push(((px[0] as u32) << 16) | ((px[1] as u32) << 8) | px[2] as u32);
        }
        // The official skin is 289x582.  The LCD and keypad rectangles scale
        // linearly for the 405x816 high-DPI variant.
        let sx = |v: u32| (v as usize * w as usize) / 289;
        let sy = |v: u32| (v as usize * h as usize) / 582;
        Some(Self {
            width: w as usize,
            height: h as usize,
            pixels,
            lcd_rect: (sx(39), sy(94), sx(254), sy(179)),
            keypad_rect: (sx(10), sy(190), sx(280), sy(575)),
        })
    }

    fn draw(&self, canvas: &mut Canvas) {
        if self.width == canvas.w && self.height == canvas.h {
            canvas.buf.copy_from_slice(&self.pixels);
            return;
        }
        for y in 0..canvas.h {
            let sy = y * self.height / canvas.h;
            for x in 0..canvas.w {
                let sx = x * self.width / canvas.w;
                canvas.buf[y * canvas.w + x] = self.pixels[sy * self.width + sx];
            }
        }
    }

    fn hit_key(&self, mx: f32, my: f32, model: Model) -> Option<&'static keys::CalcKey> {
        let (x0, y0, x1, y1) = self.keypad_rect;
        if mx < x0 as f32 || mx >= x1 as f32 || my < y0 as f32 || my >= y1 as f32 {
            return None;
        }
        let (gw, gh) = key_grid(model);
        let gx = (mx - x0 as f32) * gw as f32 / (x1 - x0) as f32;
        let gy = (my - y0 as f32) * gh as f32 / (y1 - y0) as f32;
        model.keys.iter().find(|k| {
            gx >= k.x0 as f32 && gx <= k.x1 as f32 && gy >= k.y0 as f32 && gy <= k.y1 as f32
        })
    }
}

struct Canvas {
    w: usize,
    h: usize,
    buf: Vec<u32>,
}

/// Maps a fixed-size design canvas into a resizable window while preserving
/// its aspect ratio (letterboxed/pillarboxed).  Mouse coordinates are mapped
/// back through the same transform so hitboxes stay correct at any size.
struct ViewTransform {
    scale: f32,
    dx: usize,
    dy: usize,
    dw: usize,
    dh: usize,
    ww: usize,
    src_w: usize,
    src_h: usize,
}

fn view_transform(src_w: usize, src_h: usize, ww: usize, wh: usize) -> ViewTransform {
    let scale = (ww as f32 / src_w as f32).min(wh as f32 / src_h as f32);
    let dw = ((src_w as f32 * scale).round() as usize).min(ww);
    let dh = ((src_h as f32 * scale).round() as usize).min(wh);
    ViewTransform {
        scale,
        dx: (ww - dw) / 2,
        dy: (wh - dh) / 2,
        dw,
        dh,
        ww,
        src_w,
        src_h,
    }
}

fn window_to_canvas(t: &ViewTransform, mx: f32, my: f32) -> Option<(f32, f32)> {
    if t.dw == 0 || t.dh == 0 || t.scale <= 0.0 {
        return None;
    }
    let cx = (mx - t.dx as f32) * t.src_w as f32 / t.dw as f32;
    let cy = (my - t.dy as f32) * t.src_h as f32 / t.dh as f32;
    if cx >= 0.0 && cy >= 0.0 && cx < t.src_w as f32 && cy < t.src_h as f32 {
        Some((cx, cy))
    } else {
        None
    }
}

/// Blit the design canvas into the window buffer.  Upscaling uses nearest
/// neighbour (crisp pixels); downscaling averages the covered source pixels
/// so resizing smaller does not produce moire/aliasing.
fn blit_view(src: &Canvas, view: &mut [u32], t: &ViewTransform) {
    view.fill(BG);
    if t.dw == 0 || t.dh == 0 || t.scale <= 0.0 || src.w == 0 || src.h == 0 {
        return;
    }
    // Use the exact destination/source ratio so the rounded destination size
    // fills without distorting or leaving a seam.
    let xs = t.dw as f32 / src.w as f32;
    let ys = t.dh as f32 / src.h as f32;
    if t.scale >= 1.0 {
        // Nearest neighbour with pixel-centre sampling: each design pixel gets
        // a balanced, stable footprint instead of the uneven smearing caused by
        // sampling from the top-left corner.
        let cols: Vec<usize> = (0..t.dw)
            .map(|x| (((x as f32 + 0.5) / xs) as usize).min(src.w - 1))
            .collect();
        for y in 0..t.dh {
            let sy = (((y as f32 + 0.5) / ys) as usize).min(src.h - 1);
            let src_row = sy * src.w;
            let dst_row = (t.dy + y) * t.ww + t.dx;
            let dst = &mut view[dst_row..dst_row + t.dw];
            for (x, &sx) in cols.iter().enumerate() {
                dst[x] = src.buf[src_row + sx];
            }
        }
    } else {
        for y in 0..t.dh {
            let y0 = ((y as f32 / ys).floor() as usize).min(src.h - 1);
            let y1 = (((y + 1) as f32 / ys).ceil() as usize)
                .min(src.h)
                .max(y0 + 1);
            for x in 0..t.dw {
                let x0 = ((x as f32 / xs).floor() as usize).min(src.w - 1);
                let x1 = (((x + 1) as f32 / xs).ceil() as usize)
                    .min(src.w)
                    .max(x0 + 1);
                let (mut r, mut g, mut b) = (0u32, 0u32, 0u32);
                for sy in y0..y1 {
                    let row = sy * src.w;
                    for sx in x0..x1 {
                        let c = src.buf[row + sx];
                        r += (c >> 16) & 0xff;
                        g += (c >> 8) & 0xff;
                        b += c & 0xff;
                    }
                }
                let n = ((y1 - y0) * (x1 - x0)) as u32;
                view[(t.dy + y) * t.ww + t.dx + x] = ((r / n) << 16) | ((g / n) << 8) | (b / n);
            }
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

impl Canvas {
    fn new(w: usize, h: usize) -> Self {
        Self {
            w,
            h,
            buf: vec![BG; w * h],
        }
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
        for y in y0..y1 {
            let row = y * self.w;
            for x in x0..x1 {
                self.buf[row + x] = color;
            }
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
        let mut cx = x;
        for ch in text.chars() {
            if let Some(glyph) = font8x8::BASIC_FONTS.get(ch) {
                for (gy, row) in glyph.iter().enumerate() {
                    for gx in 0..8 {
                        if (row >> gx) & 1 != 0 {
                            self.fill_rect(
                                cx + gx * scale,
                                y + gy * scale,
                                cx + (gx + 1) * scale,
                                y + (gy + 1) * scale,
                                color,
                            );
                        }
                    }
                }
            }
            cx += 8 * scale;
        }
    }

    fn draw_text_centered(
        &mut self,
        x0: usize,
        y0: usize,
        x1: usize,
        y1: usize,
        text: &str,
        color: u32,
    ) {
        let w = text.chars().count() * 8;
        let x = x0 + (x1.saturating_sub(x0).saturating_sub(w)) / 2;
        let y = y0 + (y1.saturating_sub(y0).saturating_sub(8)) / 2;
        self.draw_text(x, y, text, color, 1);
    }
}

fn draw_device_body(
    canvas: &mut Canvas,
    lcd_x: usize,
    lcd_y: usize,
    key_x: usize,
    key_y: usize,
    l: &Layout,
) {
    let body_x0 = lcd_x.min(key_x).saturating_sub(14);
    let body_x1 = (lcd_x + l.lcd_w).max(key_x + l.key_w) + 14;
    let body_y0 = lcd_y.saturating_sub(12);
    let body_y1 = key_y + l.key_h + 14;

    // Soft drop shadow, then the tinted body and its hairline border.
    canvas.fill_rect(body_x0 + 3, body_y0 + 5, body_x1 + 3, body_y1 + 5, SHADOW);
    canvas.fill_rect(body_x0, body_y0, body_x1, body_y1, BODY);
    canvas.stroke_rect(body_x0, body_y0, body_x1, body_y1, BORDER);

    // Recessed bezel around the LCD.
    let bx0 = lcd_x.saturating_sub(7);
    let by0 = lcd_y.saturating_sub(7);
    let bx1 = lcd_x + l.lcd_w + 7;
    let by1 = lcd_y + l.lcd_h + 7;
    canvas.fill_rect(bx0, by0, bx1, by1, LCD_BEZEL);
    canvas.stroke_rect(bx0, by0, bx1, by1, BORDER);
}

fn draw_lcd_fallback(
    canvas: &mut Canvas,
    lcd: &[u8],
    model: Model,
    lcd_x: usize,
    lcd_y: usize,
    w: usize,
    h: usize,
) {
    draw_lcd_at(canvas, lcd, model, lcd_x, lcd_y, w, h);
}

fn draw_lcd_rect(
    canvas: &mut Canvas,
    lcd: &[u8],
    model: Model,
    rect: (usize, usize, usize, usize),
) {
    let (x, y, x1, y1) = rect;
    draw_lcd_at(
        canvas,
        lcd,
        model,
        x,
        y,
        x1.saturating_sub(x),
        y1.saturating_sub(y),
    );
}

fn draw_lcd_at(
    canvas: &mut Canvas,
    lcd: &[u8],
    model: Model,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
) {
    canvas.fill_rect(x, y, x + w, y + h, LCD_BG);
    let (lw, lh) = (model.lcd_width, model.lcd_height);
    for py in 0..lh {
        let dy0 = y + py * h / lh;
        let dy1 = y + (py + 1) * h / lh;
        for px in 0..lw {
            if lcd[py * lw + px] != 0 {
                let dx0 = x + px * w / lw;
                let dx1 = x + (px + 1) * w / lw;
                canvas.fill_rect(dx0, dy0, dx1, dy1, LCD_FG);
            }
        }
    }
}

fn draw_keypad(canvas: &mut Canvas, machine: &Machine, model: Model, key_x: usize, key_y: usize) {
    let caption = format!("KEYPAD - {}", model.name.trim_start_matches("Casio "));
    canvas.draw_text(key_x, key_y.saturating_sub(13), &caption, MUTED, 1);
    for key in model.keys {
        let x0 = key_x + key.x0 as usize * KEY_SCALE;
        let y0 = key_y + key.y0 as usize * KEY_SCALE;
        let x1 = key_x + key.x1 as usize * KEY_SCALE;
        let y1 = key_y + key.y1 as usize * KEY_SCALE;
        let hot = (machine.pending_key == Some((key.ki, key.ko))
            || machine.held_key == Some((key.ki, key.ko)))
            && (key.ki != 0 || key.ko != 0);
        canvas.fill_rect(x0, y0, x1, y1, BORDER);
        let color = if hot { BUTTON_HOT } else { BUTTON };
        canvas.fill_rect(x0 + 1, y0 + 1, x1 - 1, y1 - 1, color);
        // Faint top highlight gives the keys a soft, tactile edge without a
        // harsh high-contrast outline.
        canvas.fill_rect(
            x0 + 1,
            y0 + 1,
            x1 - 1,
            y0 + 2,
            lerp_color(color, TEXT, 0.12),
        );
        if key.label == "ON" {
            canvas.fill_rect(x0 + 1, y0 + 1, x1 - 1, y0 + 4, ACCENT);
        }
        canvas.draw_text_centered(x0, y0, x1, y1, key.label, TEXT);
    }

    let line1 = "0-9 . + - * /   Enter =   Esc AC   Bksp DEL";
    let line2 = "F8 models   F6/F7 cycle   resize the window to zoom";
    canvas.draw_text(8, canvas.h.saturating_sub(30), line1, MUTED, 1);
    canvas.draw_text(8, canvas.h.saturating_sub(17), line2, MUTED, 1);
}

fn hit_key_fallback(
    mx: f32,
    my: f32,
    model: Model,
    key_x: usize,
    key_y: usize,
) -> Option<&'static keys::CalcKey> {
    let (mx, my) = (mx as i32, my as i32);
    for key in model.keys {
        let x0 = key_x as i32 + key.x0 as i32 * KEY_SCALE as i32;
        let y0 = key_y as i32 + key.y0 as i32 * KEY_SCALE as i32;
        let x1 = key_x as i32 + key.x1 as i32 * KEY_SCALE as i32;
        let y1 = key_y as i32 + key.y1 as i32 * KEY_SCALE as i32;
        if mx >= x0 && mx < x1 && my >= y0 && my < y1 {
            return Some(key);
        }
    }
    None
}

fn key_label(key: Key) -> Option<&'static str> {
    Some(match key {
        Key::Key0 => "0",
        Key::Key1 => "1",
        Key::Key2 => "2",
        Key::Key3 => "3",
        Key::Key4 => "4",
        Key::Key5 => "5",
        Key::Key6 => "6",
        Key::Key7 => "7",
        Key::Key8 => "8",
        Key::Key9 => "9",
        Key::Period => ".",
        Key::Minus => "-",
        Key::Equal | Key::Enter => "=",
        Key::Slash => "/",
        Key::Backspace => "DEL",
        Key::Delete => "AC",
        Key::Left => "LEFT",
        Key::Right => "RIGHT",
        Key::Up => "UP",
        Key::Down => "DOWN",
        Key::A => "Abs",
        Key::B => "a/b",
        Key::C => "cos",
        Key::D => "dms",
        Key::E => "ENG",
        Key::H => "hyp",
        Key::L => "ln",
        Key::M => "M+",
        Key::N => "xn",
        Key::O => "log",
        Key::P => "x2",
        Key::R => "RCL",
        Key::S => "sin",
        Key::T => "tan",
        Key::X => "X",
        Key::Y => "x10",
        Key::Z => "x-1",
        Key::F1 => "MODE",
        Key::F2 => "SHIFT",
        Key::F3 => "ALPHA",
        Key::F4 => "ON",
        Key::F5 => "sqrt",
        _ => return None,
    })
}

fn collect_down_keys(window: &Window) -> Vec<Key> {
    ALL_KEYS
        .iter()
        .copied()
        .filter(|k| window.is_key_down(*k))
        .collect()
}

fn collect_pressed_keys(window: &Window, last: &[Key]) -> Vec<Key> {
    ALL_KEYS
        .iter()
        .copied()
        .filter(|k| window.is_key_down(*k) && !last.contains(k))
        .collect()
}

const ALL_KEYS: &[Key] = &[
    Key::Key0,
    Key::Key1,
    Key::Key2,
    Key::Key3,
    Key::Key4,
    Key::Key5,
    Key::Key6,
    Key::Key7,
    Key::Key8,
    Key::Key9,
    Key::Period,
    Key::Minus,
    Key::Equal,
    Key::Enter,
    Key::Slash,
    Key::Backspace,
    Key::Delete,
    Key::Left,
    Key::Right,
    Key::Up,
    Key::Down,
    Key::A,
    Key::B,
    Key::C,
    Key::D,
    Key::E,
    Key::H,
    Key::L,
    Key::M,
    Key::N,
    Key::O,
    Key::P,
    Key::R,
    Key::S,
    Key::T,
    Key::X,
    Key::Y,
    Key::Z,
    Key::F1,
    Key::F2,
    Key::F3,
    Key::F4,
    Key::F5,
    Key::F6,
    Key::F7,
    Key::F8,
];

#[cfg(test)]
mod scaling_tests {
    use super::*;

    #[test]
    fn aspect_ratio_is_preserved_when_widening() {
        let t = view_transform(600, 400, 1000, 400);
        assert!((t.scale - 1.0).abs() < 1e-6);
        assert_eq!((t.dw, t.dh), (600, 400));
        assert_eq!((t.dx, t.dy), (200, 0));
    }

    #[test]
    fn aspect_ratio_is_preserved_when_tallering() {
        let t = view_transform(600, 400, 300, 300);
        assert!((t.scale - 0.5).abs() < 1e-6);
        assert_eq!((t.dw, t.dh), (300, 200));
        assert_eq!((t.dx, t.dy), (0, 50));
    }

    #[test]
    fn mouse_mapping_round_trips_through_the_transform() {
        let t = view_transform(600, 400, 1000, 400);
        let screen_x = t.dx as f32 + 300.0 * t.scale;
        let screen_y = t.dy as f32 + 200.0 * t.scale;
        let (cx, cy) = window_to_canvas(&t, screen_x, screen_y).unwrap();
        assert!((cx - 300.0).abs() < 0.01);
        assert!((cy - 200.0).abs() < 0.01);
        // Clicks in the letterbox area are ignored.
        assert!(window_to_canvas(&t, 10.0, 200.0).is_none());
        assert!(window_to_canvas(&t, 300.0, 390.0).is_some());
    }

    #[test]
    fn blit_fills_letterbox_and_downscales_without_aliasing() {
        let mut src = Canvas::new(4, 2);
        src.buf.fill(0x00FF_0000);
        let mut view = vec![0u32; 8 * 8];
        let t = view_transform(4, 2, 8, 8);
        assert_eq!((t.dw, t.dh), (8, 4));
        assert_eq!((t.dx, t.dy), (0, 2));
        blit_view(&src, &mut view, &t);
        // Letterbox rows above and below the content.
        assert_eq!(view[0], BG);
        assert_eq!(view[7 * 8], BG);
        // Content region is the source colour.
        assert_eq!(view[2 * 8 + 4], 0x00FF_0000);
        assert_eq!(view[5 * 8 + 3], 0x00FF_0000);
    }

    #[test]
    fn every_model_reserves_the_full_keypad() {
        for m in model::ALL.iter().copied() {
            let l = layout(m);
            let key_y = BAR_H + PAD_TOP + l.lcd_h + PAD_GAP;
            assert!(
                key_y + l.key_h <= l.h,
                "{}: keypad bottom {} exceeds canvas height {}",
                m.id,
                key_y + l.key_h,
                l.h
            );
            assert!(l.key_w <= l.w, "{}: keypad wider than canvas", m.id);
            assert!(l.lcd_w <= l.w, "{}: LCD wider than canvas", m.id);
        }
    }

    #[test]
    fn max_canvas_covers_every_model() {
        let (mw, mh) = max_canvas();
        for m in model::ALL.iter().copied() {
            let l = layout(m);
            assert!(l.w <= mw, "{} wider than shared canvas", m.id);
            assert!(l.h <= mh, "{} taller than shared canvas", m.id);
        }
    }

    #[test]
    fn lcd_and_keypad_proportions_are_sane() {
        for m in model::ALL.iter().copied() {
            let l = layout(m);
            // The LCD should not dominate the keypad width, and every LCD dot
            // is scaled by the same whole number.
            assert!(
                l.lcd_w <= l.key_w,
                "{}: LCD {} wider than keypad {}",
                m.id,
                l.lcd_w,
                l.key_w
            );
            let ls = lcd_scale(m);
            assert_eq!(l.lcd_w, m.lcd_width * ls);
            assert_eq!(l.lcd_h, m.lcd_height * ls);
        }
    }
}
