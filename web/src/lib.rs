//! WebAssembly bindings for the Casio nX-U8/100 calculator emulator.

use casio_emu_rs::cpu::Cpu;
use casio_emu_rs::keys;
use casio_emu_rs::machine::Machine;
use casio_emu_rs::model::{self, Model};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Emulator {
    machine: Machine,
    cpu: Cpu,
    model: Model,
}

#[wasm_bindgen]
impl Emulator {
    #[wasm_bindgen(constructor)]
    pub fn new(rom: &[u8]) -> Emulator {
        console_error_panic_hook::set_once();
        let model = model::detect(rom).unwrap_or_else(model::default);
        Self::build(rom, model)
    }

    /// Construct an emulator with an explicit model id (see `model_ids`).
    pub fn new_with_model(rom: &[u8], id: &str) -> Emulator {
        console_error_panic_hook::set_once();
        let model = model::by_id(id).unwrap_or_else(model::default);
        Self::build(rom, model)
    }

    fn build(rom: &[u8], model: Model) -> Emulator {
        let mut machine = Machine::with_model(rom.to_vec(), model);
        let mut cpu = Cpu::new();
        cpu.reset(&mut machine);
        Emulator {
            machine,
            cpu,
            model,
        }
    }

    /// Identifier of the model currently being emulated.
    pub fn model_id(&self) -> String {
        self.model.id.to_string()
    }

    /// Human-readable name of the model currently being emulated.
    pub fn model_name(&self) -> String {
        self.model.name.to_string()
    }

    /// Width of the active model's LCD in dots.
    pub fn lcd_width(&self) -> usize {
        self.model.lcd_width
    }

    /// Height of the active model's LCD in dots.
    pub fn lcd_height(&self) -> usize {
        self.model.lcd_height
    }

    pub fn step(&mut self, count: u32) {
        for _ in 0..count {
            self.machine.tick(&mut self.cpu);
        }
    }

    pub fn press(&mut self, ki: u8, ko: u8) {
        self.machine.press(ki, ko);
    }

    pub fn release(&mut self) {
        self.machine.release();
    }

    pub fn reset(&mut self) {
        self.cpu.reset(&mut self.machine);
    }

    /// 96*32 bytes, one byte per pixel.
    pub fn render(&self) -> Vec<u8> {
        self.machine.render().to_vec()
    }

    pub fn pc(&self) -> u16 {
        self.cpu.regs.pc
    }

    pub fn psw(&self) -> u8 {
        self.cpu.regs.psw
    }

    pub fn cycles(&self) -> f64 {
        self.cpu.cycles as f64
    }
}

/// JSON array of the model ids accepted by `new_with_model`.
#[wasm_bindgen]
pub fn model_ids() -> String {
    let mut s = String::from("[");
    for (i, m) in model::ALL.iter().enumerate() {
        if i != 0 {
            s.push(',');
        }
        s.push_str(&format!("{{\"id\":\"{}\",\"name\":\"{}\"}}", m.id, m.name));
    }
    s.push(']');
    s
}

/// JSON description of the key matrix, used by the web frontend for
/// clickable on-screen keys.
#[wasm_bindgen]
pub fn key_table_json() -> String {
    let mut s = String::from("[");
    for (i, k) in keys::KEYS.iter().enumerate() {
        if i != 0 {
            s.push(',');
        }
        s.push_str(&format!(
            "{{\"label\":\"{}\",\"ki\":{},\"ko\":{},\"x0\":{},\"y0\":{},\"x1\":{},\"y1\":{}}}",
            k.label, k.ki, k.ko, k.x0, k.y0, k.x1, k.y1
        ));
    }
    s.push(']');
    s
}
