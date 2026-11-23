//! WebAssembly bindings for the fx-991ES PLUS C emulator.

use fx991es_rs::cpu::Cpu;
use fx991es_rs::keys;
use fx991es_rs::machine::Machine;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub struct Emulator {
    machine: Machine,
    cpu: Cpu,
}

#[wasm_bindgen]
impl Emulator {
    #[wasm_bindgen(constructor)]
    pub fn new(rom: &[u8]) -> Emulator {
        console_error_panic_hook::set_once();
        let mut machine = Machine::new(rom.to_vec());
        let mut cpu = Cpu::new();
        cpu.reset(&mut machine);
        Emulator { machine, cpu }
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
