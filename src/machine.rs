//! fx-991ES PLUS C machine: memory map, keyboard mailbox and LCD.

use crate::cpu::{Bus, Cpu};

pub const LCD_WIDTH: usize = 96;
pub const LCD_HEIGHT: usize = 32;
pub const VRAM_BASE: u16 = 0xF800;
pub const KEY_MAILBOX: u16 = 0x8E00;

/// ROM image size used by this model: two 64 KiB code pages.
pub const ROM_SIZE: usize = 0x20000;

pub struct Machine {
    pub rom: Vec<u8>,
    pub ram: [u8; 0x8000],
    pub keypad: [u8; 0x30],
    pub sfr: [u8; 0x1000],
    last_ready: u8,
    pub pending_key: Option<(u8, u8)>,
}

impl Machine {
    pub fn new(rom: Vec<u8>) -> Self {
        let mut rom = rom;
        rom.resize(ROM_SIZE, 0xFF);
        Self {
            rom,
            ram: [0; 0x8000],
            keypad: [0; 0x30],
            sfr: [0; 0x1000],
            last_ready: 0,
            pending_key: None,
        }
    }

    /// Reads one byte from the data address space.
    fn read_data_byte(&mut self, addr: u32) -> u8 {
        match addr {
            0x00000..=0x07FFF => self.rom[addr as usize],
            0x08000..=0x08DFF => self.ram[(addr - 0x08000) as usize],
            0x08E00..=0x08E2F => self.keypad[(addr - 0x08E00) as usize],
            0x0F000..=0x0FFFF => self.sfr[(addr - 0x0F000) as usize],
            0x10000..=0x1FFFF => self.rom[addr as usize],
            0x80000..=0x8FFFF => self.rom[(addr - 0x80000) as usize],
            _ => 0,
        }
    }

    /// Writes one byte to the data address space.
    fn write_data_byte(&mut self, addr: u32, value: u8) {
        match addr {
            0x08000..=0x08DFF => self.ram[(addr - 0x08000) as usize] = value,
            0x08E00..=0x08E2F => self.keypad[(addr - 0x08E00) as usize] = value,
            0x0F000..=0x0FFFF => self.sfr[(addr - 0x0F000) as usize] = value,
            // ROM and ROM window are read-only.
            _ => {}
        }
    }

    /// Reads one byte from the code address space.
    fn read_code_byte(&self, addr: u32) -> u8 {
        match addr {
            0x00000..=0x1FFFF => self.rom[addr as usize],
            _ => 0xFF,
        }
    }

    /// Runs one instruction and maintains the key latch.
    pub fn tick(&mut self, cpu: &mut Cpu) {
        let ready = self.keypad[(KEY_MAILBOX - 0x8E00) as usize];
        if self.last_ready == 0 && ready != 0 {
            self.keypad[1] = 0;
            self.keypad[2] = 0;
        }
        if ready != 0 {
            if let Some((ki, ko)) = self.pending_key.take() {
                self.keypad[1] = ki;
                self.keypad[2] = ko;
            }
        }
        self.last_ready = ready;
        cpu.step(self);
    }

    pub fn press(&mut self, ki: u8, ko: u8) {
        self.pending_key = Some((ki, ko));
    }

    pub fn release(&mut self) {
        self.pending_key = None;
    }

    /// FNV-1a checksum of the mutable machine memory.  Used by the accuracy
    /// harness to detect divergence that is not visible in the registers.
    pub fn checksum(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        for b in self
            .ram
            .iter()
            .chain(self.sfr.iter())
            .chain(self.keypad.iter())
        {
            h ^= *b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
        h
    }

    /// Renders the 96x32 LCD into a one-byte-per-pixel buffer.
    pub fn render(&self) -> [u8; LCD_WIDTH * LCD_HEIGHT] {
        let mut out = [0u8; LCD_WIDTH * LCD_HEIGHT];
        for y in 0..8 {
            for x in 0..48 {
                for py in 0..4 {
                    let off = (y << 6) + (py << 4) + (x >> 2);
                    let byte = self.sfr[(VRAM_BASE - 0xF000) as usize + off];
                    for px in 0..2 {
                        let bit = (byte >> (7 - (((x & 3) << 1) + px))) & 1;
                        out[(y * 4 + py) * LCD_WIDTH + x * 2 + px] = bit;
                    }
                }
            }
        }
        out
    }
}

impl Bus for Machine {
    fn read_code(&mut self, csr: u8, offset: u16, size: usize) -> u64 {
        let base = ((csr as u32) << 16) | offset as u32;
        let mut val = 0u64;
        for i in 0..size {
            val |= (self.read_code_byte(base.wrapping_add(i as u32)) as u64) << (8 * i);
        }
        val
    }

    fn write_code(&mut self, _csr: u8, _offset: u16, _size: usize, _value: u64) {
        // Code memory is ROM.
    }

    fn read_data(&mut self, dsr: u8, offset: u16, size: usize) -> u64 {
        let base = ((dsr as u32) << 16) | offset as u32;
        let mut val = 0u64;
        for i in 0..size {
            val |= (self.read_data_byte(base.wrapping_add(i as u32)) as u64) << (8 * i);
        }
        val
    }

    fn write_data(&mut self, dsr: u8, offset: u16, size: usize, value: u64) {
        let base = ((dsr as u32) << 16) | offset as u32;
        for i in 0..size {
            self.write_data_byte(
                base.wrapping_add(i as u32),
                ((value >> (8 * i)) & 0xff) as u8,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn load_test_rom() -> Option<Vec<u8>> {
        let path = std::env::var("FX991_ROM").unwrap_or_else(|_| "fx991es.rom.bin".to_string());
        std::fs::read(path).ok()
    }

    #[test]
    fn rom_boot_checkpoint() {
        let Some(rom) = load_test_rom() else {
            return;
        };
        let mut machine = Machine::new(rom);
        let mut cpu = Cpu::new();
        cpu.reset(&mut machine);
        for _ in 0..200_000 {
            machine.tick(&mut cpu);
        }
        assert_eq!(cpu.regs.pc, 0xdaa6);
        assert_eq!(cpu.regs.csr, 1);
        assert_eq!(cpu.regs.psw, 0x00);
    }

    #[test]
    #[ignore = "runs 6M instructions; set FX991_ROM and use --ignored"]
    fn key_one_checkpoint() {
        let Some(rom) = load_test_rom() else {
            return;
        };
        let mut machine = Machine::new(rom);
        let mut cpu = Cpu::new();
        cpu.reset(&mut machine);
        for step in 0..6_000_000u32 {
            if step == 2_000_000 {
                machine.press(0x01, 0x01);
            }
            machine.tick(&mut cpu);
        }
        assert_eq!(cpu.regs.pc, 0x538a);
        assert_eq!(cpu.regs.psw, 0x00);
        assert_eq!(machine.render().iter().filter(|&&p| p != 0).count(), 32);
    }

    #[test]
    fn vram_layout_top_left() {
        let mut m = Machine::new(vec![0; 0x20000]);
        m.sfr[(VRAM_BASE - 0xF000) as usize] = 0x80 | 0x20; // x=0 and x=2, y=0
        let lcd = m.render();
        assert_eq!(lcd[0], 1);
        assert_eq!(lcd[2], 1);
        assert_eq!(lcd[1], 0);
    }
}
