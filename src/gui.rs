//! Desktop frontend: minifb window, optional device skin, keyboard/mouse input.

use font8x8::UnicodeFonts;
use fx991es_rs::cpu::Cpu;
use fx991es_rs::keys;
use fx991es_rs::machine::{Machine, LCD_HEIGHT, LCD_WIDTH};
use minifb::{Key, MouseButton, MouseMode, Window, WindowOptions};
use std::path::{Path, PathBuf};

const FALLBACK_W: usize = 440;
const FALLBACK_H: usize = 460;
const LCD_SCALE: usize = 4;
const LCD_X: usize = (FALLBACK_W - LCD_WIDTH * LCD_SCALE) / 2;
const LCD_Y: usize = 18;
const KEY_SCALE: usize = 8;
const KEY_X: usize = (FALLBACK_W - keys::GRID_W as usize * KEY_SCALE) / 2;
const KEY_Y: usize = LCD_Y + LCD_HEIGHT * LCD_SCALE + 18;

const BG: u32 = 0x0020_2028;
const LCD_BG: u32 = 0x00d6_e3d6;
const LCD_FG: u32 = 0x0010_1810;
const BUTTON: u32 = 0x0040_4048;
const BUTTON_HOT: u32 = 0x0060_6070;
const TEXT: u32 = 0x00e0_e0e0;
const ACCENT: u32 = 0x00ff_8844;

pub fn default_skin_candidates() -> Vec<PathBuf> {
    vec![
        PathBuf::from("assets/skin.png"),
        PathBuf::from("assets/skin.bmp"),
        PathBuf::from("skin.png"),
        PathBuf::from("skin.bmp"),
    ]
}

pub fn run_gui(rom: Vec<u8>, skin_path: Option<PathBuf>) {
    let mut machine = Machine::new(rom);
    let mut cpu = Cpu::new();
    cpu.reset(&mut machine);

    let skin = load_skin(skin_path.as_deref());

    let (win_w, win_h) = if let Some(skin) = &skin {
        (skin.width, skin.height)
    } else {
        (FALLBACK_W, FALLBACK_H)
    };

    let mut window = Window::new(
        "fx-991ES PLUS C — Rust",
        win_w,
        win_h,
        WindowOptions {
            resize: true,
            ..WindowOptions::default()
        },
    )
    .expect("cannot create window");
    window.set_target_fps(60);

    let mut canvas = Canvas::new(win_w, win_h);
    let mut view: Vec<u32> = Vec::new();
    let mut last_keys: Vec<Key> = Vec::new();
    let mut mouse_down = false;

    while window.is_open() && !window.is_key_down(Key::Escape) {
        if let Some(skin) = &skin {
            skin.draw(&mut canvas);
            draw_lcd_rect(&mut canvas, &machine.render(), skin.lcd_rect);
        } else {
            canvas.fill(BG);
            draw_background(&mut canvas);
            draw_lcd_fallback(&mut canvas, &machine.render());
            draw_keypad(&mut canvas, &machine);
        }

        // Keyboard input.  The firmware scans the matrix itself, so a press
        // only needs to be presented for one mailbox cycle.
        let shift = window.is_key_down(Key::LeftShift) || window.is_key_down(Key::RightShift);
        let pressed = collect_pressed_keys(&window, &last_keys);
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
        let currently_down = collect_down_keys(&window);
        if currently_down.is_empty() && !last_keys.is_empty() {
            machine.release();
        }
        last_keys = currently_down;

        // Mouse input on the device image.  The window may be resized, so
        // map from window coordinates back to the design canvas.
        let (ww, wh) = window.get_size();
        let mouse = window.get_mouse_pos(MouseMode::Discard);
        if let Some((mx, my)) = mouse {
            let mx = mx * canvas.w as f32 / ww.max(1) as f32;
            let my = my * canvas.h as f32 / wh.max(1) as f32;
            let down = window.get_mouse_down(MouseButton::Left);
            if down && !mouse_down {
                let key = if let Some(skin) = &skin {
                    skin.hit_key(mx, my)
                } else {
                    hit_key_fallback(mx, my)
                };
                if let Some(key) = key {
                    if key.label == "ON" {
                        cpu.reset(&mut machine);
                    } else if key.ki != 0 || key.ko != 0 {
                        machine.press(key.ki, key.ko);
                    }
                    mouse_down = true;
                }
            } else if !down && mouse_down {
                machine.release();
                mouse_down = false;
            }
        }

        // About 7.2M instructions/second at 60 fps.
        for _ in 0..120_000 {
            machine.tick(&mut cpu);
        }

        // Nearest-neighbour scale the design canvas to the current window
        // size.  This keeps the skin and LCD crisp at any zoom.
        let (ww, wh) = window.get_size();
        if ww == 0 || wh == 0 {
            continue;
        }
        if view.len() != ww * wh {
            view.resize(ww * wh, BG);
        }
        for y in 0..wh {
            let sy = y * canvas.h / wh;
            let src_row = sy * canvas.w;
            let dst_row = y * ww;
            for x in 0..ww {
                let sx = x * canvas.w / ww;
                view[dst_row + x] = canvas.buf[src_row + sx];
            }
        }
        window
            .update_with_buffer(&view, ww, wh)
            .expect("cannot update window");
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

    fn hit_key(&self, mx: f32, my: f32) -> Option<&'static keys::CalcKey> {
        let (x0, y0, x1, y1) = self.keypad_rect;
        if mx < x0 as f32 || mx >= x1 as f32 || my < y0 as f32 || my >= y1 as f32 {
            return None;
        }
        let gx = (mx - x0 as f32) * keys::GRID_W as f32 / (x1 - x0) as f32;
        let gy = (my - y0 as f32) * 28.0 / (y1 - y0) as f32;
        keys::KEYS.iter().find(|k| {
            gx >= k.x0 as f32 && gx <= k.x1 as f32 && gy >= k.y0 as f32 && gy <= k.y1 as f32
        })
    }
}

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

    fn fill(&mut self, color: u32) {
        self.buf.fill(color);
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

fn draw_background(canvas: &mut Canvas) {
    let x0 = LCD_X.saturating_sub(6);
    let y0 = LCD_Y.saturating_sub(6);
    let x1 = LCD_X + LCD_WIDTH * LCD_SCALE + 6;
    let y1 = LCD_Y + LCD_HEIGHT * LCD_SCALE + 6;
    canvas.fill_rect(x0, y0, x1, y1, 0x0030_3038);
}

fn draw_lcd_fallback(canvas: &mut Canvas, lcd: &[u8; LCD_WIDTH * LCD_HEIGHT]) {
    draw_lcd_at(
        canvas,
        lcd,
        LCD_X,
        LCD_Y,
        LCD_WIDTH * LCD_SCALE,
        LCD_HEIGHT * LCD_SCALE,
    );
}

fn draw_lcd_rect(
    canvas: &mut Canvas,
    lcd: &[u8; LCD_WIDTH * LCD_HEIGHT],
    rect: (usize, usize, usize, usize),
) {
    let (x, y, x1, y1) = rect;
    draw_lcd_at(
        canvas,
        lcd,
        x,
        y,
        x1.saturating_sub(x),
        y1.saturating_sub(y),
    );
}

fn draw_lcd_at(
    canvas: &mut Canvas,
    lcd: &[u8; LCD_WIDTH * LCD_HEIGHT],
    x: usize,
    y: usize,
    w: usize,
    h: usize,
) {
    canvas.fill_rect(x, y, x + w, y + h, LCD_BG);
    for py in 0..LCD_HEIGHT {
        let dy0 = y + py * h / LCD_HEIGHT;
        let dy1 = y + (py + 1) * h / LCD_HEIGHT;
        for px in 0..LCD_WIDTH {
            if lcd[py * LCD_WIDTH + px] != 0 {
                let dx0 = x + px * w / LCD_WIDTH;
                let dx1 = x + (px + 1) * w / LCD_WIDTH;
                canvas.fill_rect(dx0, dy0, dx1, dy1, LCD_FG);
            }
        }
    }
}

fn draw_keypad(canvas: &mut Canvas, machine: &Machine) {
    for key in keys::KEYS {
        let x0 = KEY_X + key.x0 as usize * KEY_SCALE;
        let y0 = KEY_Y + key.y0 as usize * KEY_SCALE;
        let x1 = KEY_X + key.x1 as usize * KEY_SCALE;
        let y1 = KEY_Y + key.y1 as usize * KEY_SCALE;
        let hot = machine.pending_key == Some((key.ki, key.ko)) && (key.ki != 0 || key.ko != 0);
        let color = if hot { BUTTON_HOT } else { BUTTON };
        canvas.fill_rect(x0 + 1, y0 + 1, x1 - 1, y1 - 1, color);
        if key.label == "ON" {
            canvas.fill_rect(x0 + 1, y0 + 1, x1 - 1, y0 + 4, ACCENT);
        }
        canvas.draw_text_centered(x0, y0, x1, y1, key.label, TEXT);
    }

    let help = "0-9 . + - * /  Enter =  Esc AC  Bksp DEL";
    canvas.draw_text(8, FALLBACK_H - 18, help, 0x00a0_a0a0, 1);
}

fn hit_key_fallback(mx: f32, my: f32) -> Option<&'static keys::CalcKey> {
    let (mx, my) = (mx as i32, my as i32);
    for key in keys::KEYS {
        let x0 = KEY_X as i32 + key.x0 as i32 * KEY_SCALE as i32;
        let y0 = KEY_Y as i32 + key.y0 as i32 * KEY_SCALE as i32;
        let x1 = KEY_X as i32 + key.x1 as i32 * KEY_SCALE as i32;
        let y1 = KEY_Y as i32 + key.y1 as i32 * KEY_SCALE as i32;
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
];
