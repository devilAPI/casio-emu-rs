//! Calculator machine: memory map, keyboard and LCD.
//!
//! The CPU core is shared by all models; the board around it (ROM windows,
//! RAM placement, VRAM geometry and keyboard interface) comes from
//! [`Model`](crate::model::Model).

use crate::cpu::{Bus, Cpu};
use crate::model::{self, BoardKind, KeyboardKind, Model};

/// Default (fx-991ES PLUS C) LCD geometry, kept as aliases for old callers.
pub const LCD_WIDTH: usize = 96;
pub const LCD_HEIGHT: usize = 32;
pub const VRAM_BASE: u16 = 0xF800;
pub const KEY_MAILBOX: u16 = 0x8E00;

/// ROM image size used by the default model: two 64 KiB code pages.
pub const ROM_SIZE: usize = 0x20000;

pub use crate::model::{KI_PORT, KO_DRIVE_HI, KO_DRIVE_LO, KO_MASK_HI, KO_MASK_LO};

/// Last address of ROM/constant data in ClassWiz segment 0 (0x00000..0x0CFFF).
const CLASSWIZ_SEG0_END: u32 = model::CLASSWIZ_SEG0_SIZE as u32 - 1;

/// Last address of ROM/constant data in CW ii segment 0 (0x00000..0x08FFF).
const CLASSWIZ2_SEG0_END: u32 = 0x08FFF;

/// Number of maskable interrupt sources on the CW ii board.
const CW2_INT_COUNT: usize = 21;
/// First maskable interrupt index (INT_MASKABLE in CasioEmuX).
const CW2_INT_BASE: usize = 5;
/// Timer 0 / keyboard external interrupt 0 source indices.
const CW2_TIMER0_INT: usize = 4;
const CW2_KEY_INT: usize = 0;

/// CW ii peripheral state that has no direct counterpart on the older boards.
#[derive(Clone, Debug)]
pub struct CwiiState {
    /// Two 64-row x 32-byte framebuffers.  On real hardware the CPU sees them
    /// both through the 0xF800 window, selected by bit 2 of 0xF037.  On
    /// emulator-derived firmware the second one lives at 0x89000.
    pub vram: [[u8; model::CLASSWIZ2_VRAM_SIZE]; 2],
    /// Screen control registers (0xF030/31/32/37).
    pub screen_range: u8,
    pub screen_mode: u8,
    pub screen_contrast: u8,
    pub screen_select: u8,
    /// Interrupt mask/pending registers (0xF010..0xF017).  Bits 1..=21 map to
    /// the 21 maskable interrupt sources; bit 0 is the non-maskable source.
    pub int_mask: u32,
    pub int_pending: u32,
    pub irq_active: [bool; CW2_INT_COUNT],
    /// Timer 0 (0xF020..0xF025).
    pub timer_interval: u16,
    pub timer_counter: u16,
    pub timer_control: u8,
    pub timer_fcon: u8,
    pub timer_prescaler: u8,
    pub timer_ticks: u8,
    pub lsclk_counter: u32,
    /// Last DSR prefix seen by the bus (reported at 0xF000).
    pub dsr_sfr: u8,
    /// External interrupt control at 0xF018.
    pub exicon: u8,
    /// Standby control state (0xF008/F009/F312).
    pub halted: bool,
    pub stopped: bool,
    pub stpacp_last: u8,
    pub stop_acceptor_enabled: bool,
    pub f312_last: u8,
    pub shutdown_acceptor_enabled: bool,
    /// Battery level detector (PowerSupply.cpp, 0xF0D0..0xF0D2).  BLDFlag=1
    /// means the battery is healthy; a 0 makes the firmware warn about a dead
    /// battery.
    pub bld_threshold: u8,
    pub bld_mode: u8,
    pub bld_control: u8,
    pub bld_flag: u8,
    pub bld_con2: u8,
    pub bld_test_ticks: u32,
    /// Solar panel indicator at 0xF310 (low nibble stored, bit 4 = present).
    pub spi_indicator: u8,
    pub solar_available: bool,
    /// Enhanced calculator (BCD coprocessor) registers.
    pub bcd_cmd: u8,
    pub bcd_con: u8,
    pub bcd_mcn: u8,
    pub bcd_mcr: u8,
    pub bcd_flag: u8,
    pub bcd_llz: u8,
    pub bcd_mlz: u8,
    pub bcd_reg: [[u8; 12]; 4],
}

impl Default for CwiiState {
    fn default() -> Self {
        Self {
            vram: [[0; model::CLASSWIZ2_VRAM_SIZE]; 2],
            screen_range: 0,
            screen_mode: 0,
            screen_contrast: 0,
            screen_select: 0,
            int_mask: 0,
            int_pending: 0,
            irq_active: [false; CW2_INT_COUNT],
            timer_interval: 0,
            timer_counter: 0,
            timer_control: 0,
            timer_fcon: 0,
            timer_prescaler: 0,
            timer_ticks: 0,
            lsclk_counter: 0,
            dsr_sfr: 0,
            exicon: 0,
            halted: false,
            stopped: false,
            stpacp_last: 0,
            stop_acceptor_enabled: false,
            f312_last: 0,
            shutdown_acceptor_enabled: false,
            bld_threshold: 0,
            bld_mode: 0,
            bld_control: 0,
            bld_flag: 1,
            bld_con2: 0,
            bld_test_ticks: 0,
            spi_indicator: 0,
            solar_available: false,
            bcd_cmd: 0,
            bcd_con: 6,
            bcd_mcn: 0,
            bcd_mcr: 0,
            bcd_flag: 0,
            bcd_llz: 0,
            bcd_mlz: 0,
            bcd_reg: [[0; 12]; 4],
        }
    }
}

/// CPU execution state controlled by SBYCON (0xF009).  Halt/STOP freeze the
/// core; accepting an interrupt resumes it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunMode {
    Run,
    Halt,
    Stop,
}

pub struct Machine {
    /// Configuration of the calculator this machine emulates.
    pub model: Model,
    pub rom: Vec<u8>,
    /// RAM window. Indexed from the board's RAM base.
    pub ram: [u8; 0x8000],
    pub keypad: [u8; 0x30],
    pub sfr: [u8; model::SFR_SIZE],
    last_ready: u8,
    /// One-shot key waiting for the emulator mailbox to become ready.
    pub pending_key: Option<(u8, u8)>,
    /// Currently held key for real-hardware keyboards.
    pub held_key: Option<(u8, u8)>,
    /// Live value of the KO drive register (F046/F047).
    pub ko_drive: u16,
    /// Live value of the KO mask register (F044/F045).
    pub ko_mask: u16,
    /// ClassWiz II-only peripheral state (unused by ES PLUS / ClassWiz).
    pub cw: CwiiState,
    /// True once a key has been pressed on a CW ii board, used to generate a
    /// single keyboard external interrupt.
    cw_key_irq_pending: bool,
    // --- ClassWiz (HW id 4) peripherals ------------------------------------
    /// ClassWiz timer: interval, counter, prescaler and running flag.
    cw_timer_interval: u16,
    cw_timer_counter: u16,
    cw_timer_prescaler: u32,
    cw_timer_ticks: u32,
    cw_timer_running: bool,
    /// Avoid retriggering the ClassWiz keyboard IRQ while a key is held.
    cw_key_irq_raised: bool,
    /// StandbyControl (F008/F009) on ClassWiz boards.
    cw_standby: bool,
    cw_stop_acceptor: bool,
    // --- Real-hardware peripherals (HardwareSfr models only) ---------------
    /// Whether any pressed key passed the input filter when it was pressed.
    has_input: bool,
    /// Timer raise latch (the reference Timer::raise_required).
    timer_raise: bool,
    /// Prescaler tick counter and the tick on which the timer advances.
    timer_ext_counter: u32,
    timer_ext_next: u32,
    timer_int_done: u32,
    /// STPACP sequence detector / SBYCON state.
    stpacp_last: u8,
    stop_acceptor_enabled: bool,
    run_mode: RunMode,
}

impl Machine {
    /// Build a machine using the default model (fx-991ES PLUS C).
    pub fn new(rom: Vec<u8>) -> Self {
        Self::with_model(rom, model::default())
    }

    /// Build a machine for a specific calculator model.
    pub fn with_model(rom: Vec<u8>, model: Model) -> Self {
        let mut rom = rom;
        rom.resize(model.rom_size, 0xFF);
        let mut machine = Self {
            model,
            rom,
            ram: [0; 0x8000],
            keypad: [0; 0x30],
            sfr: [0; model::SFR_SIZE],
            last_ready: 0,
            pending_key: None,
            held_key: None,
            ko_drive: 0,
            ko_mask: 0,
            cw: CwiiState::default(),
            cw_key_irq_pending: false,
            cw_timer_interval: 1,
            cw_timer_counter: 0,
            cw_timer_prescaler: 1,
            cw_timer_ticks: 0,
            cw_timer_running: false,
            cw_key_irq_raised: false,
            cw_standby: false,
            cw_stop_acceptor: false,
            has_input: false,
            timer_raise: false,
            timer_ext_counter: 0,
            // Timer::Reset calls DivideTicks() once: the first prescaler
            // boundary is after 131072/10000 = 13 instructions.
            timer_ext_next: model::CPU_CYCLES_PER_SECOND / model::TIMER_EXT_FREQUENCY,
            timer_int_done: 1,
            stpacp_last: 0,
            stop_acceptor_enabled: false,
            run_mode: RunMode::Run,
        };
        // fx-991CW / fx-991CN CW have a solar cell; fx-9910CW and fx-880BTG
        // use AAA batteries only.
        if matches!(machine.model.id, "fx991cw" | "fx991cncw") {
            machine.cw.solar_available = true;
        }
        machine
    }

    /// RAM base address for this board.
    pub fn ram_base(&self) -> u32 {
        match self.model.board {
            BoardKind::EsPlus => model::ESPLUS_RAM_BASE,
            BoardKind::ClassWiz => model::CLASSWIZ_RAM_BASE,
            BoardKind::ClassWizII => model::CLASSWIZ2_RAM_BASE,
        }
    }

    /// RAM size for this board.
    pub fn ram_size(&self) -> usize {
        match self.model.board {
            BoardKind::EsPlus => model::ESPLUS_RAM_SIZE,
            BoardKind::ClassWiz => model::CLASSWIZ_RAM_SIZE,
            BoardKind::ClassWizII => model::CLASSWIZ2_RAM_SIZE,
        }
    }

    /// Load a raw RAM snapshot into this board's RAM window.
    ///
    /// The window is cleared first, then the first `min(data.len(), ram_size())`
    /// bytes of `data` are copied at the board's RAM base (ES PLUS `0x08000`,
    /// ClassWiz `0x0D000`).  Bytes past the end of the window are ignored, so
    /// a 8 KiB ClassWiz dump loaded into an ES PLUS machine is clipped to
    /// `0xE00` bytes and can never overwrite the key mailbox or SFRs.
    ///
    /// This matches the `ram=` option of user202729/CasioEmu, whose
    /// `BatteryBackedRAM::LoadRAMImage()` writes the raw file straight into
    /// the same battery-backed RAM buffer.  Returns the number of bytes
    /// actually written.
    pub fn load_ram(&mut self, data: &[u8]) -> usize {
        let size = self.ram_size();
        self.ram[..size].fill(0);
        let copied = data.len().min(size);
        self.ram[..copied].copy_from_slice(&data[..copied]);
        copied
    }

    /// Base of the ROM-window mirror (data segment 8 / segment 5).
    pub fn rom_mirror_base(&self) -> u32 {
        match self.model.board {
            BoardKind::EsPlus => model::ESPLUS_ROM_MIRROR_BASE,
            BoardKind::ClassWiz => model::CLASSWIZ_ROM_MIRROR_BASE,
            BoardKind::ClassWizII => model::CLASSWIZ2_ROM_MIRROR_BASE,
        }
    }

    /// VRAM row stride and number of displayed bytes per row.
    pub fn vram_geometry(&self) -> (usize, usize) {
        match self.model.board {
            BoardKind::EsPlus => (model::ESPLUS_VRAM_ROW_STRIDE, model::ESPLUS_VRAM_DISP_BYTES),
            BoardKind::ClassWiz | BoardKind::ClassWizII => (
                model::CLASSWIZ_VRAM_ROW_STRIDE,
                model::CLASSWIZ_VRAM_DISP_BYTES,
            ),
        }
    }

    /// Whether the ClassWiz (HW id 4) peripheral model is active for this
    /// board.  ES PLUS keeps its own IRQ/timer path and CW ii has its own.
    fn has_classwiz_peripherals(&self) -> bool {
        self.model.board == BoardKind::ClassWiz
    }

    /// Translate a CW ii code address (CSR segment + 16-bit offset) into a ROM
    /// offset.  Mirrors `MMU::ReadCode` for `HW_CLASSWIZ_II`: segment 8 aliases
    /// bank 0, the upper three bits of the CSR are ignored, segment 6 and the
    /// upper half of segment 7 are unmapped, and the reset-vector window at the
    /// top of segment 0 reads as erased ROM.
    pub fn cw2_code_addr(csr: u8, offset: u16) -> Option<u32> {
        if csr == 8 {
            return Some(offset as u32);
        }
        let seg = csr & 7;
        if seg == 6 || (seg == 7 && offset >= 0x2000) {
            return None;
        }
        if seg == 0 && offset >= 0xFE00 {
            return None;
        }
        Some(((seg as u32) << 16) | offset as u32)
    }

    /// Translate a CW ii data address into a ROM offset (ROM-window decoding).
    /// Segment 0 exposes only the first 0x9000 bytes (the remainder is RAM and
    /// the SFR page); segment 8 exposes ROM bank 8 except for the second
    /// framebuffer at 0x89000; all other segments map directly onto ROM pages.
    fn cw2_data_rom_addr(addr: u32) -> Option<usize> {
        let seg = (addr >> 16) & 0x1F;
        let off = addr & 0xFFFF;
        match seg {
            0 => (off <= CLASSWIZ2_SEG0_END).then_some(off as usize),
            1..=7 => Some(addr as usize),
            8 => {
                if (model::CLASSWIZ2_VRAM2_BASE
                    ..model::CLASSWIZ2_VRAM2_BASE + model::CLASSWIZ2_VRAM_SIZE as u32)
                    .contains(&addr)
                {
                    None
                } else if off < 0xFFE0 {
                    Some(addr as usize)
                } else {
                    None
                }
            }
            9..=15 => Some(addr as usize),
            _ => None,
        }
    }

    /// Read a byte from a CW ii SFR address (0xF000..0xFFFF, excluding VRAM
    /// and keyboard KI which are routed by `read_data_byte`).
    fn read_cw2_sfr(&self, addr: u32) -> u8 {
        let off = (addr - model::SFR_BASE) as usize;
        match addr {
            0xF000 => self.cw.dsr_sfr,
            0xF00A => self.cw.timer_fcon,
            0xF010..=0xF013 => (self.cw.int_mask >> ((off - 0x10) * 8)) as u8,
            0xF014..=0xF017 => (self.cw.int_pending >> ((off - 0x14) * 8)) as u8,
            0xF018 => self.cw.exicon,
            0xF020 => self.cw.timer_interval as u8,
            0xF021 => (self.cw.timer_interval >> 8) as u8,
            0xF022 => self.cw.timer_counter as u8,
            0xF023 => (self.cw.timer_counter >> 8) as u8,
            0xF025 => self.cw.timer_control & 1,
            0xF028 => self.sfr[off],
            0xF030 => self.cw.screen_range & 0x07,
            0xF031 => self.cw.screen_mode & 0x07,
            0xF032 => self.cw.screen_contrast & 0x3F,
            0xF037 => self.cw.screen_select & 0x04,
            0xF040 => self.read_ki(),
            0xF050 => 0, // Pd value: real hardware ties this low
            0xF0D0 => self.cw.bld_threshold & 0x1F,
            0xF0D1 => (self.cw.bld_mode << 2) | (self.cw.bld_flag << 1) | self.cw.bld_control,
            0xF0D2 => self.cw.bld_con2 & 0x37,
            0xF310 => {
                (self.cw.spi_indicator & 0x0F) | if self.cw.solar_available { 0x10 } else { 0 }
            }
            0xF400 => self.cw.bcd_cmd,
            0xF402 => self.cw.bcd_con,
            0xF404 => self.cw.bcd_mcn & 0x1F,
            0xF405 => 0, // macro running flag (no BCD macros implemented yet)
            0xF410 => {
                (if self.cw.bcd_flag & 0x80 != 0 {
                    0x80
                } else {
                    0
                }) | (if self.cw.bcd_flag & 0x40 != 0 {
                    0x40
                } else {
                    0
                })
            }
            0xF414 => self.cw.bcd_llz,
            0xF415 => self.cw.bcd_mlz,
            0xF480..=0xF48B => self.cw.bcd_reg[0][off - 0x480],
            0xF4A0..=0xF4AB => self.cw.bcd_reg[1][off - 0x4A0],
            0xF4C0..=0xF4CB => self.cw.bcd_reg[2][off - 0x4C0],
            0xF4E0..=0xF4EB => self.cw.bcd_reg[3][off - 0x4E0],
            _ => self.sfr[off],
        }
    }

    /// Writes a byte to a CW ii SFR address.
    fn write_cw2_sfr(&mut self, addr: u32, value: u8) {
        let off = (addr - model::SFR_BASE) as usize;
        if self.model.keyboard == KeyboardKind::HardwareSfr {
            match addr {
                KO_MASK_LO => self.ko_mask = (self.ko_mask & 0xFF00) | value as u16,
                KO_MASK_HI => self.ko_mask = (self.ko_mask & 0x00FF) | ((value as u16) << 8),
                KO_DRIVE_LO => self.ko_drive = (self.ko_drive & 0xFF00) | value as u16,
                KO_DRIVE_HI => self.ko_drive = (self.ko_drive & 0x00FF) | ((value as u16) << 8),
                _ => {}
            }
        }
        match addr {
            0xF008 => {
                if value & 0xF0 == 0xA0 && self.cw.stpacp_last & 0xF0 == 0x50 {
                    self.cw.stop_acceptor_enabled = true;
                }
                self.cw.stpacp_last = value;
            }
            0xF009 => {
                if value & 0x01 != 0 {
                    self.cw.halted = true;
                }
                if value & 0x02 != 0 && self.cw.stop_acceptor_enabled {
                    self.cw.stop_acceptor_enabled = false;
                    self.cw.stopped = true;
                }
            }
            0xF312 => {
                if value == 0x3C && self.cw.f312_last == 0x5A {
                    self.cw.shutdown_acceptor_enabled = true;
                }
                self.cw.f312_last = value;
                if self.cw.shutdown_acceptor_enabled && value & 0xF0 == 0 {
                    self.cw.screen_mode = 3;
                    self.sfr[0x31] = 3;
                    self.cw.stopped = true;
                    self.cw.shutdown_acceptor_enabled = false;
                }
            }
            0xF00A => {
                self.cw.timer_fcon = value & 0x73;
                self.sfr[off] = value & 0x73;
            }
            0xF010..=0xF013 => {
                let shift = (off - 0x10) * 8;
                self.cw.int_mask =
                    (self.cw.int_mask & !(0xFFu32 << shift)) | ((value as u32) << shift);
                self.cw.int_mask &= 0x3F_FFFE;
                self.sfr[off] = value;
            }
            0xF014..=0xF017 => {
                let shift = (off - 0x14) * 8;
                self.cw.int_pending =
                    (self.cw.int_pending & !(0xFFu32 << shift)) | ((value as u32) << shift);
                self.cw.int_pending &= 0x3F_FFFF;
                for i in 0..CW2_INT_COUNT {
                    self.cw.irq_active[i] = self.cw.int_pending & (1 << (i + 1)) != 0;
                }
                self.sfr[off] = value;
            }
            0xF018 => {
                self.cw.exicon = value;
                self.sfr[off] = value;
            }
            0xF020 => self.cw.timer_interval = (self.cw.timer_interval & 0xFF00) | value as u16,
            0xF021 => {
                self.cw.timer_interval = (self.cw.timer_interval & 0x00FF) | ((value as u16) << 8)
            }
            0xF022 | 0xF023 => self.cw.timer_counter = 0,
            0xF024 => {
                self.cw.timer_prescaler = value & 0x07;
                self.sfr[off] = value & 0x0F;
            }
            0xF025 => {
                self.cw.timer_control = value & 1;
                self.sfr[off] = value & 1;
            }
            0xF028 => self.sfr[off] = value,
            0xF030 => self.cw.screen_range = value & 0x07,
            0xF031 => self.cw.screen_mode = value & 0x07,
            0xF032 => self.cw.screen_contrast = value & 0x3F,
            0xF037 => self.cw.screen_select = value & 0x04,
            0xF040 => {} // KI is read-only
            0xF041 | 0xF042 => self.sfr[off] = value,
            0xF0D0 => self.cw.bld_threshold = value & 0x1F,
            0xF0D1 => {
                self.cw.bld_mode = (value >> 2) & 1;
                if value & 1 != 0 {
                    // Start a battery test; our virtual battery is healthy.
                    self.cw.bld_control = 1;
                    self.cw.bld_test_ticks = 512;
                } else {
                    self.cw.bld_control = 0;
                    self.cw.bld_test_ticks = 0;
                }
            }
            0xF0D2 => self.cw.bld_con2 = value & 0x37,
            0xF310 => self.cw.spi_indicator = value & 0x0F,
            0xF400 => self.cw.bcd_cmd = value,
            0xF402 => self.cw.bcd_con = value.clamp(1, 6),
            0xF404 => self.cw.bcd_mcn = value & 0x1F,
            0xF405 => {} // BCD macros are not implemented yet
            0xF410 => self.cw.bcd_flag = value & 0xC0,
            0xF414 | 0xF415 => {} // read-only leading-zero counters
            0xF480..=0xF48B => self.cw.bcd_reg[0][off - 0x480] = value,
            0xF4A0..=0xF4AB => self.cw.bcd_reg[1][off - 0x4A0] = value,
            0xF4C0..=0xF4CB => self.cw.bcd_reg[2][off - 0x4C0] = value,
            0xF4E0..=0xF4EB => self.cw.bcd_reg[3][off - 0x4E0] = value,
            _ => self.sfr[off] = value,
        }
    }

    // --- ClassWiz (HW id 4) peripherals ------------------------------------

    /// Low byte of an SFR (convenience for the 16-bit interrupt/timer regs).
    fn sfr_byte(&self, addr: u32) -> u8 {
        self.sfr[(addr - model::SFR_BASE) as usize]
    }

    fn set_sfr_byte(&mut self, addr: u32, value: u8) {
        let i = (addr - model::SFR_BASE) as usize;
        self.sfr[i] = value;
    }

    /// Raise a ClassWiz maskable source by setting its pending bit.  Source
    /// `n` uses bit `n + 1` in `data_int_pending`.
    fn cw_raise_source(&mut self, bit: u16) {
        let pending = u16::from_le_bytes([
            self.sfr_byte(model::CW_INT_PENDING),
            self.sfr_byte(model::CW_INT_PENDING + 1),
        ]) | bit;
        self.set_sfr_byte(model::CW_INT_PENDING, pending as u8);
        self.set_sfr_byte(model::CW_INT_PENDING + 1, (pending >> 8) as u8);
    }

    /// Advance the ClassWiz timer by one instruction.  Called even while the
    /// CPU is stopped so an expired timer can wake the chipset.
    fn cw_timer_tick(&mut self) {
        if !self.has_classwiz_peripherals() || !self.cw_timer_running {
            return;
        }
        // The firmware uses a prescaler of 2 (F024=1) for short delays and
        // 128 (F024=0x0F) for the periodic tick, matching the reference core.
        self.cw_timer_ticks += 1;
        if self.cw_timer_ticks < self.cw_timer_prescaler {
            return;
        }
        self.cw_timer_ticks = 0;
        self.cw_timer_counter = self.cw_timer_counter.wrapping_add(1);
        if self.cw_timer_counter >= self.cw_timer_interval.max(1) {
            self.cw_timer_counter = 0;
            self.cw_raise_source(model::CW_IRQ_TIMER_BIT);
        }
        self.set_sfr_byte(model::CW_TIMER_COUNTER, self.cw_timer_counter as u8);
        self.set_sfr_byte(
            model::CW_TIMER_COUNTER + 1,
            (self.cw_timer_counter >> 8) as u8,
        );
    }

    /// Any pending ClassWiz interrupt wakes the chipset from HALT/STOP.
    fn cw_interrupt_pending(&self) -> bool {
        let pending = u16::from_le_bytes([
            self.sfr_byte(model::CW_INT_PENDING),
            self.sfr_byte(model::CW_INT_PENDING + 1),
        ]);
        pending & (model::CW_IRQ_KEYBOARD_BIT | model::CW_IRQ_TIMER_BIT) != 0
    }

    /// Deliver any unmasked, pending ClassWiz interrupts (lowest source first).
    fn cw_poll_interrupts(&mut self, cpu: &mut Cpu) {
        if !self.has_classwiz_peripherals() {
            return;
        }
        let mask = u16::from_le_bytes([
            self.sfr_byte(model::CW_INT_MASK),
            self.sfr_byte(model::CW_INT_MASK + 1),
        ]);
        let pending = u16::from_le_bytes([
            self.sfr_byte(model::CW_INT_PENDING),
            self.sfr_byte(model::CW_INT_PENDING + 1),
        ]);
        for (bit, index) in [
            (model::CW_IRQ_KEYBOARD_BIT, model::CW_IRQ_KEYBOARD_INDEX),
            (model::CW_IRQ_TIMER_BIT, model::CW_IRQ_TIMER_INDEX),
        ] {
            if pending & bit == 0 || mask & bit == 0 {
                continue;
            }
            if cpu.accept_maskable_interrupt(self, index) {
                self.cw_clear_source(bit);
                break;
            }
        }
    }

    fn cw_clear_source(&mut self, bit: u16) {
        let pending = u16::from_le_bytes([
            self.sfr_byte(model::CW_INT_PENDING),
            self.sfr_byte(model::CW_INT_PENDING + 1),
        ]) & !bit;
        self.set_sfr_byte(model::CW_INT_PENDING, pending as u8);
        self.set_sfr_byte(model::CW_INT_PENDING + 1, (pending >> 8) as u8);
    }

    /// Whether this board uses the real-hardware IRQ/timer peripherals.
    /// The emulator-mailbox firmware is already accuracy-validated and must
    /// never see any of the behaviour below.
    fn hardware_irqs(&self) -> bool {
        self.model.keyboard == KeyboardKind::HardwareSfr
    }

    /// Read a byte from the hardware keyboard KI register (active low).
    /// Only KO0..6 carry keys; KO7..9 are the reserved model-ID (Pd) sense
    /// lines and never pull KI low on real hardware.
    fn read_ki(&self) -> u8 {
        let mut value = 0xFFu8;
        if let Some((ki, ko)) = self.held_key {
            let driven = self.ko_drive & !self.ko_mask & model::KO_BITS_MASK;
            if driven & (ko as u16) != 0 {
                value &= !ki;
            }
        }
        value
    }

    /// Model-ID value presented at 0xF050.  Only emulator-derived firmware
    /// exposes this register; real hardware leaves KO7..9 unconnected.
    fn pd_value(&self) -> u8 {
        0
    }

    /// 16-bit little-endian SFR read (mask/pending are 13-bit fields).
    fn sfr16(&self, base: u32) -> u16 {
        let i = (base - model::SFR_BASE) as usize;
        u16::from_le_bytes([self.sfr[i], self.sfr[i + 1]])
    }

    fn sfr16_masked(&self, base: u32) -> u16 {
        self.sfr16(base) & ((1u16 << model::INT_MANAGED_COUNT) - 1)
    }

    /// Mark an IRQ pending and, when interrupts are enabled at the CPU,
    /// deliver it.  Returns true when the pending bit was newly set.
    fn raise_irq(&mut self, cpu: &mut Cpu, source: usize) -> bool {
        let bit = 1u16 << (source - model::INT_MANAGED_BASE);
        let mask = self.sfr16_masked(model::INT_MASK);
        let pending = self.sfr16_masked(model::INT_PENDING);
        if mask & bit == 0 || pending & bit != 0 {
            return false;
        }
        let i = (model::INT_PENDING - model::SFR_BASE) as usize;
        let new_pending = pending | bit;
        self.sfr[i] = new_pending as u8;
        self.sfr[i + 1] = (new_pending >> 8) as u8;
        // AcceptInterrupt resumes the core even when MIE is clear.
        self.run_mode = RunMode::Run;
        cpu.raise_maskable(self, source);
        true
    }

    /// One prescaler step of the 10000 Hz timer (Timer::DivideTicks).
    fn timer_divide_ticks(&mut self) {
        self.timer_int_done += 1;
        if self.timer_int_done == model::TIMER_EXT_FREQUENCY {
            self.timer_int_done = 0;
            self.timer_ext_counter = 0;
        }
        self.timer_ext_next =
            model::CPU_CYCLES_PER_SECOND * (self.timer_int_done + 1) / model::TIMER_EXT_FREQUENCY;

        if self.sfr[(model::TIMER_CONTROL - model::SFR_BASE) as usize] & 1 != 0 {
            if self.sfr16(model::TIMER_COUNTER) == self.sfr16(model::TIMER_INTERVAL) {
                let i = (model::TIMER_COUNTER - model::SFR_BASE) as usize;
                self.sfr[i] = 0;
                self.sfr[i + 1] = 0;
                if self.sfr16_masked(model::INT_MASK) & (1 << (model::IRQ_TIMER - 4)) != 0 {
                    self.timer_raise = true;
                }
            }
            let counter = self.sfr16(model::TIMER_COUNTER).wrapping_add(1);
            let i = (model::TIMER_COUNTER - model::SFR_BASE) as usize;
            self.sfr[i] = counter as u8;
            self.sfr[i + 1] = (counter >> 8) as u8;
        }
    }

    /// Peripheral tick, run before every instruction for HardwareSfr boards.
    fn tick_peripherals(&mut self, cpu: &mut Cpu) {
        // Keyboard::Tick raises source 5 while a filtered key is held.
        if self.has_input {
            self.raise_irq(cpu, model::IRQ_KEYBOARD);
        }

        // Timer::Tick: advance the prescaler and retry a pending timer IRQ.
        if self.timer_ext_counter == self.timer_ext_next {
            self.timer_divide_ticks();
        }
        self.timer_ext_counter = self.timer_ext_counter.wrapping_add(1);

        if self.timer_raise && self.raise_irq(cpu, model::IRQ_TIMER) {
            self.timer_raise = false;
        }
    }

    /// Reads one byte from the data address space.
    fn read_data_byte(&mut self, addr: u32) -> u8 {
        let ram_base = self.ram_base();
        let mirror = self.rom_mirror_base();

        if self.model.board == BoardKind::ClassWizII {
            if (model::CLASSWIZ2_VRAM2_BASE
                ..model::CLASSWIZ2_VRAM2_BASE + model::CLASSWIZ2_VRAM_SIZE as u32)
                .contains(&addr)
            {
                return self.cw.vram[1][(addr - model::CLASSWIZ2_VRAM2_BASE) as usize];
            }
            if (model::SFR_BASE..model::SFR_BASE + model::SFR_SIZE as u32).contains(&addr) {
                if addr >= 0xF800 {
                    let bank = if self.cw.screen_select & 0x04 != 0 {
                        1
                    } else {
                        0
                    };
                    return self.cw.vram[bank][(addr - 0xF800) as usize];
                }
                return self.read_cw2_sfr(addr);
            }
            if (ram_base..ram_base + self.ram_size() as u32).contains(&addr) {
                return self.ram[(addr - ram_base) as usize];
            }
            return Self::cw2_data_rom_addr(addr)
                .map(|i| self.rom.get(i).copied().unwrap_or(0))
                .unwrap_or(0);
        }

        match self.model.board {
            BoardKind::EsPlus => match addr {
                0x00000..=0x07FFF => self.rom[addr as usize],
                0x08000..=0x08DFF => self.ram[(addr - ram_base) as usize],
                0x08E00..=0x08E2F => self.keypad[(addr - model::ESPLUS_MAILBOX_BASE) as usize],
                0x0F000..=0x0FFFF => {
                    if self.hardware_irqs() {
                        match addr {
                            KI_PORT => return self.read_ki(),
                            KO_MASK_LO | KO_MASK_HI | KO_DRIVE_LO | KO_DRIVE_HI => {
                                return self.sfr[(addr - model::SFR_BASE) as usize]
                                    & (model::KO_BITS_MASK >> (8 * (addr & 1))) as u8;
                            }
                            // Standby registers read as zero on hardware.
                            model::STPACP | model::SBYCON => return 0,
                            model::PD_PORT => return self.pd_value(),
                            _ => {}
                        }
                    }
                    self.sfr[(addr - model::SFR_BASE) as usize]
                }
                0x10000..=0x1FFFF => self.rom[addr as usize],
                0x80000..=0x8FFFF => self.rom[(addr - mirror) as usize],
                _ => 0,
            },
            BoardKind::ClassWiz | BoardKind::ClassWizII => match addr {
                0x00000..=CLASSWIZ_SEG0_END => self.rom[addr as usize],
                0x0D000..=0x0EFFF => self.ram[(addr - ram_base) as usize],
                0x0F000..=0x0FFFF => {
                    if addr == KI_PORT {
                        return self.read_ki();
                    }
                    self.sfr[(addr - model::SFR_BASE) as usize]
                }
                0x10000..=0x3FFFF => self.rom[addr as usize],
                0x50000..=0x5FFFF => self.rom[(addr - mirror) as usize],
                _ => 0,
            },
        }
    }

    /// Writes one byte to the data address space.
    fn write_data_byte(&mut self, addr: u32, mut value: u8) {
        let ram_base = self.ram_base();
        let ram_size = self.ram_size();

        if self.model.board == BoardKind::ClassWizII {
            if (model::CLASSWIZ2_VRAM2_BASE
                ..model::CLASSWIZ2_VRAM2_BASE + model::CLASSWIZ2_VRAM_SIZE as u32)
                .contains(&addr)
            {
                self.cw.vram[1][(addr - model::CLASSWIZ2_VRAM2_BASE) as usize] = value;
                return;
            }
            if (model::SFR_BASE..model::SFR_BASE + model::SFR_SIZE as u32).contains(&addr) {
                if addr >= 0xF800 {
                    let bank = if self.cw.screen_select & 0x04 != 0 {
                        1
                    } else {
                        0
                    };
                    self.cw.vram[bank][(addr - 0xF800) as usize] = value;
                } else {
                    self.write_cw2_sfr(addr, value);
                }
                return;
            }
            if addr >= ram_base && addr - ram_base < ram_size as u32 {
                self.ram[(addr - ram_base) as usize] = value;
            }
            // ROM windows are read-only.
            return;
        }

        let in_ram = addr >= ram_base && addr - ram_base < ram_size as u32;
        if in_ram {
            self.ram[(addr - ram_base) as usize] = value;
            return;
        }

        match self.model.board {
            BoardKind::EsPlus => {
                if (model::ESPLUS_MAILBOX_BASE
                    ..model::ESPLUS_MAILBOX_BASE + model::ESPLUS_MAILBOX_SIZE as u32)
                    .contains(&addr)
                {
                    self.keypad[(addr - model::ESPLUS_MAILBOX_BASE) as usize] = value;
                    return;
                }
            }
            BoardKind::ClassWiz | BoardKind::ClassWizII => {
                // No emulator mailbox on real-hardware ClassWiz ROMs.
            }
        }

        if (model::SFR_BASE..model::SFR_BASE + model::SFR_SIZE as u32).contains(&addr) {
            if self.hardware_irqs() {
                match addr {
                    model::STPACP if self.model.board == BoardKind::EsPlus => {
                        // 0x50 followed by 0xA0 arms the STOP acceptor.
                        self.stop_acceptor_enabled = value == 0xA0 && self.stpacp_last == 0x50;
                        self.stpacp_last = value;
                        // STPACP reads as zero, but keep the last value for
                        // the sequence detector.
                        return;
                    }
                    model::SBYCON if self.model.board == BoardKind::EsPlus => {
                        if value & 1 != 0 {
                            self.run_mode = RunMode::Halt;
                        } else if value & 2 != 0 && self.stop_acceptor_enabled {
                            self.stop_acceptor_enabled = false;
                            self.run_mode = RunMode::Stop;
                        }
                        return;
                    }
                    model::INT_MASK | model::INT_PENDING
                        if self.model.board == BoardKind::EsPlus =>
                    {
                        let field_mask = (1u16 << model::INT_MANAGED_COUNT) - 1;
                        let reg_base = if addr == model::INT_PENDING {
                            model::INT_PENDING
                        } else {
                            model::INT_MASK
                        };
                        value &= (field_mask >> (8 * (addr - reg_base))) as u8;
                    }
                    // Interval 0 is illegal; the hardware forces it to 1
                    // after the byte write (handled below).
                    model::TIMER_INTERVAL if self.model.board == BoardKind::EsPlus => {}
                    model::TIMER_COUNTER if self.model.board == BoardKind::EsPlus => {
                        // Writing either byte clears the whole counter.
                        let i = (model::TIMER_COUNTER - model::SFR_BASE) as usize;
                        self.sfr[i] = 0;
                        self.sfr[i + 1] = 0;
                        return;
                    }
                    model::TIMER_CONTROL if self.model.board == BoardKind::EsPlus => {
                        self.timer_raise = false;
                        self.sfr[(addr - model::SFR_BASE) as usize] = value & 1;
                        return;
                    }
                    model::PD_PORT if self.model.board == BoardKind::EsPlus => {
                        // Read-only model-ID register on emulator ROMs.
                        return;
                    }
                    KO_MASK_LO => {
                        self.ko_mask = (self.ko_mask & 0xFF00) | value as u16;
                        self.ko_mask &= model::KO_BITS_MASK;
                    }
                    KO_MASK_HI => {
                        self.ko_mask = (self.ko_mask & 0x00FF) | ((value as u16) << 8);
                        self.ko_mask &= model::KO_BITS_MASK;
                    }
                    KO_DRIVE_LO => {
                        self.ko_drive = (self.ko_drive & 0xFF00) | value as u16;
                        self.ko_drive &= model::KO_BITS_MASK;
                    }
                    KO_DRIVE_HI => {
                        self.ko_drive = (self.ko_drive & 0x00FF) | ((value as u16) << 8);
                        self.ko_drive &= model::KO_BITS_MASK;
                    }
                    _ => {}
                }
            }
            self.sfr[(addr - model::SFR_BASE) as usize] = value;
            if self.model.board == BoardKind::EsPlus {
                if self.hardware_irqs()
                    && addr == model::TIMER_INTERVAL
                    && self.sfr16(model::TIMER_INTERVAL) == 0
                {
                    let i = (model::TIMER_INTERVAL - model::SFR_BASE) as usize;
                    self.sfr[i] = 1;
                }
            } else if self.has_classwiz_peripherals() {
                // ClassWiz (HW id 4) uses the same SFR addresses but a
                // different timer/pending-bit layout from ES PLUS.
                if addr == model::CW_TIMER_INTERVAL || addr == model::CW_TIMER_INTERVAL + 1 {
                    let interval = u16::from_le_bytes([
                        self.sfr_byte(model::CW_TIMER_INTERVAL),
                        self.sfr_byte(model::CW_TIMER_INTERVAL + 1),
                    ]);
                    // A zero interval would fire continuously; treat as one.
                    self.cw_timer_interval = if interval == 0 { 1 } else { interval };
                } else if addr == model::CW_TIMER_COUNTER || addr == model::CW_TIMER_COUNTER + 1 {
                    self.cw_timer_counter = 0;
                    self.cw_timer_ticks = 0;
                    self.set_sfr_byte(model::CW_TIMER_COUNTER, 0);
                    self.set_sfr_byte(model::CW_TIMER_COUNTER + 1, 0);
                } else if addr == model::CW_TIMER_PRESCALER {
                    self.cw_timer_prescaler = 1u32 << (value & 0x07);
                } else if addr == model::CW_TIMER_CONTROL {
                    // Enabling/disabling does not clear the counter; only a
                    // write to F022 does that.
                    self.cw_timer_running = value & 0x01 != 0;
                } else if addr == model::STPACP {
                    if value & 0xF0 == 0xA0 && self.stpacp_last & 0xF0 == 0x50 {
                        self.cw_stop_acceptor = true;
                    }
                    self.stpacp_last = value;
                } else if addr == model::SBYCON {
                    if value & 0x01 != 0 {
                        self.cw_standby = true;
                    }
                    if value & 0x02 != 0 && self.cw_stop_acceptor {
                        self.cw_stop_acceptor = false;
                        self.cw_standby = true;
                    }
                }
            }
        }
        // ROM and ROM window are read-only.
    }

    /// Reads one byte from the code address space (segment already selected).
    fn read_code_byte(&self, addr: u32) -> u8 {
        match self.model.board {
            BoardKind::EsPlus => match addr {
                0x00000..=0x1FFFF => self.rom[addr as usize],
                0x80000..=0x8FFFF => self.rom[(addr - model::ESPLUS_ROM_MIRROR_BASE) as usize],
                _ => 0xFF,
            },
            BoardKind::ClassWiz => match addr {
                // Segment 0 is special: the reference core reads the whole
                // 64 KiB ROM page in code space, even though data space only
                // exposes 0x0000..0xCFFF (RAM/SFR live above that).
                0x00000..=0x0FFFF => self.rom[addr as usize],
                0x10000..=0x3FFFF => self.rom[addr as usize],
                0x50000..=0x5FFFF => self.rom[(addr - model::CLASSWIZ_ROM_MIRROR_BASE) as usize],
                _ => 0xFF,
            },
            BoardKind::ClassWizII => Self::cw2_code_addr((addr >> 16) as u8, addr as u16)
                .and_then(|a| self.rom.get(a as usize).copied())
                .unwrap_or(0xFF),
        }
    }

    /// Advance the CW ii peripherals and deliver a pending maskable interrupt.
    /// Returns `false` when SBYCON put the core into HALT/STOP and no interrupt
    /// is pending yet (the CPU must not execute an instruction this tick).
    fn cw2_tick(&mut self, cpu: &mut Cpu) -> bool {
        // Battery-level detector: a healthy virtual battery always passes.
        if self.cw.bld_test_ticks > 0 {
            self.cw.bld_test_ticks -= 1;
            if self.cw.bld_test_ticks == 0 {
                self.cw.bld_flag = 1;
                self.cw.bld_control = 0;
            }
        }
        // FCON low bits = 1 selects the low-speed clock (16384 Hz, i.e. one
        // tick per 256 cycles).  F024 bit 3 selects HSCLK instead, which ticks
        // once per cycle.  Both continue to run while the core is halted.
        let mut timer_tick = false;
        if self.cw.timer_fcon & 0x03 == 1 {
            self.cw.lsclk_counter += 1;
            if self.cw.lsclk_counter >= 256 {
                self.cw.lsclk_counter = 0;
                timer_tick = self.sfr[0x24] & 0x08 == 0;
            }
        }
        if self.sfr[0x24] & 0x08 != 0 {
            timer_tick = true;
        }
        if timer_tick && self.cw.timer_control & 1 != 0 {
            let div = 1usize << self.cw.timer_prescaler;
            self.cw.timer_ticks += 1;
            if self.cw.timer_ticks as usize >= div {
                self.cw.timer_ticks = 0;
                if self.cw.timer_interval == 0 {
                    self.cw.timer_counter = 0;
                } else {
                    self.cw.timer_counter = self.cw.timer_counter.wrapping_add(1);
                    if self.cw.timer_counter >= self.cw.timer_interval {
                        self.cw.timer_counter = 0;
                        self.cw.int_pending |= 1 << (CW2_TIMER0_INT + 1);
                        self.cw.irq_active[CW2_TIMER0_INT] = true;
                    }
                }
            }
        }

        if self.cw_key_irq_pending {
            self.cw.int_pending |= 1 << (CW2_KEY_INT + 1);
            self.cw.irq_active[CW2_KEY_INT] = true;
            self.cw_key_irq_pending = false;
        }

        // Any pending interrupt restarts the core after HALT/STOP.
        if (self.cw.halted || self.cw.stopped) && self.cw.int_pending == 0 {
            return false;
        }
        self.cw.halted = false;
        self.cw.stopped = false;

        // Lowest-numbered enabled source wins; EL must be 0 or 1 and MIE set.
        if cpu.master_interrupt_enable() && cpu.regs.psw & 0x03 <= 1 {
            for i in 0..CW2_INT_COUNT {
                let bit = 1 << (i + 1);
                if self.cw.int_pending & bit != 0 && self.cw.int_mask & bit != 0 {
                    self.cw.int_pending &= !bit;
                    self.cw.irq_active[i] = false;
                    cpu.raise(1, CW2_INT_BASE + i, self);
                    break;
                }
            }
        }
        true
    }

    /// Runs one instruction and maintains the key input path.
    pub fn tick(&mut self, cpu: &mut Cpu) {
        if self.model.board == BoardKind::ClassWizII {
            // CW ii owns its own memory map, framebuffers and IRQ controller.
            if !self.cw2_tick(cpu) {
                return;
            }
            cpu.step(self);
            return;
        }
        if self.has_classwiz_peripherals() {
            // ClassWiz (HW id 4) timer/standby/interrupt path.
            self.cw_timer_tick();
            if self.cw_standby && !self.cw_interrupt_pending() {
                return;
            }
            self.cw_standby = false;
            cpu.step(self);
            self.cw_poll_interrupts(cpu);
            return;
        }
        if self.hardware_irqs() {
            // Real-hardware firmware waits in HALT/STOP until a keyboard or
            // timer interrupt is accepted; peripherals keep ticking while the
            // core is frozen.  An accepted interrupt resumes execution.
            self.tick_peripherals(cpu);
            if self.run_mode == RunMode::Run {
                cpu.step(self);
            }
            return;
        }

        let mailbox = (self.model.key_mailbox - 0x8E00) as usize;
        let ready = self.keypad[mailbox];
        if self.last_ready == 0 && ready != 0 {
            self.keypad[mailbox + 1] = 0;
            self.keypad[mailbox + 2] = 0;
        }
        if ready != 0 {
            if let Some((ki, ko)) = self.pending_key.take() {
                self.keypad[mailbox + 1] = ki;
                self.keypad[mailbox + 2] = ko;
            }
        }
        self.last_ready = ready;
        cpu.step(self);
    }

    /// Press a key on the emulated keypad.
    ///
    /// For emulator-derived firmware this queues a one-shot key that is
    /// delivered when the mailbox becomes ready.  For real-hardware firmware
    /// it holds the key down until [`Machine::release`] is called.
    pub fn press(&mut self, ki: u8, ko: u8) {
        if self.model.keyboard == KeyboardKind::HardwareSfr {
            self.held_key = Some((ki, ko));
            match self.model.board {
                BoardKind::EsPlus => {
                    // Keyboard::RecalculateGhost samples the input filter at
                    // the moment of the press, not on every tick.
                    self.has_input =
                        ki & self.sfr[(model::KO_FILTER - model::SFR_BASE) as usize] != 0;
                }
                BoardKind::ClassWiz => {
                    // Real hardware raises external interrupt 0 on a key-down
                    // edge; delivery happens on the next tick.
                    if !self.cw_key_irq_raised {
                        self.cw_raise_source(model::CW_IRQ_KEYBOARD_BIT);
                        self.cw_key_irq_raised = true;
                    }
                }
                BoardKind::ClassWizII => {
                    // CW ii has a separate external-interrupt controller.
                    self.cw_key_irq_pending = true;
                }
            }
        } else {
            self.pending_key = Some((ki, ko));
        }
    }

    pub fn release(&mut self) {
        self.pending_key = None;
        self.held_key = None;
        self.has_input = false;
        self.cw_key_irq_raised = false;
    }

    /// FNV-1a checksum of the mutable machine memory.  Used by the accuracy
    /// harness to detect divergence that is not visible in the registers.
    pub fn checksum(&self) -> u64 {
        let mut h = 0xcbf2_9ce4_8422_2325u64;
        let mut hash_byte = |b: u8| {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        };
        for b in self
            .ram
            .iter()
            .chain(self.sfr.iter())
            .chain(self.keypad.iter())
        {
            hash_byte(*b);
        }
        // Only CW ii owns extra VRAM; ES PLUS/ClassWiz checksums stay exactly
        // as before so the validated C-model hashes remain comparable.
        if self.model.board == BoardKind::ClassWizII {
            for b in self.cw.vram.iter().flatten() {
                hash_byte(*b);
            }
        }
        h
    }

    /// Render the LCD into one byte per pixel.
    ///
    /// Returns a row-major buffer of `model.lcd_width * model.lcd_height`
    /// bytes.  Row 0 is the status/icon row; the remaining rows are the dot
    /// matrix.  The four high bits of each byte are used as pixel columns;
    /// ClassWiz VRAM rows are 32 bytes wide but only 24 are displayed.
    pub fn render(&self) -> Vec<u8> {
        let width = self.model.lcd_width;
        let height = self.model.lcd_height;
        let mut out = vec![0u8; width * height];
        let (stride, disp) = self.vram_geometry();
        if self.model.board == BoardKind::ClassWizII {
            // The two framebuffers are blended on real hardware (~30/70).  For
            // the pixel dump we OR them, which shows everything the firmware
            // has drawn regardless of which bank is currently selected.
            for row in 0..height {
                for byte in 0..disp {
                    let i = row * stride + byte;
                    let bits = self.cw.vram[0][i] | self.cw.vram[1][i];
                    for bit in 0..8 {
                        out[row * width + byte * 8 + bit] = (bits >> (7 - bit)) & 1;
                    }
                }
            }
            return out;
        }
        if self.has_classwiz_peripherals() {
            // The reference `Screen::Frame()` only draws for modes 4, 5 and 6,
            // and only mode 5 shows the dot matrix.  ES PLUS rendering is
            // deliberately unchanged: its validated behaviour draws VRAM
            // regardless of the mode register.
            let mode = self.sfr_byte(model::SCREEN_MODE) & 0x07;
            if mode != 5 {
                return out;
            }
        }
        let vram = (self.model.vram_base - model::SFR_BASE as u16) as usize;
        for row in 0..height {
            let base = vram + row * stride;
            for byte in 0..disp {
                let bits = self.sfr[base + byte];
                for bit in 0..8 {
                    out[row * width + byte * 8 + bit] = (bits >> (7 - bit)) & 1;
                }
            }
        }
        out
    }
}

impl Bus for Machine {
    fn read_code(&mut self, csr: u8, offset: u16, size: usize) -> u64 {
        if self.model.board == BoardKind::ClassWizII {
            let csr = csr & self.model.csr_mask;
            let mut val = 0u64;
            for i in 0..size {
                let byte = Self::cw2_code_addr(csr, offset.wrapping_add(i as u16))
                    .and_then(|a| self.rom.get(a as usize).copied())
                    .unwrap_or(0xFF);
                val |= (byte as u64) << (8 * i);
            }
            return val;
        }
        // HW_CLASSWIZ implements 4 bits of CSR, HW_ES_PLUS only 1.
        let csr = csr & self.model.csr_mask;
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
        // 0xF000 reports the last DSR *prefix*, so a plain (non-prefixed)
        // access must not overwrite it.
        if self.model.board == BoardKind::ClassWizII && dsr != 0 {
            self.cw.dsr_sfr = dsr & 0x1F;
        }
        let base = ((dsr as u32) << 16) | offset as u32;
        let mut val = 0u64;
        for i in 0..size {
            val |= (self.read_data_byte(base.wrapping_add(i as u32)) as u64) << (8 * i);
        }
        val
    }

    fn write_data(&mut self, dsr: u8, offset: u16, size: usize, value: u64) {
        if self.model.board == BoardKind::ClassWizII && dsr != 0 {
            self.cw.dsr_sfr = dsr & 0x1F;
        }
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
    use crate::model;

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

    #[test]
    fn hardware_keyboard_is_active_low_and_ko_gated() {
        let mut m = Machine::with_model(vec![0; 0x20000], model::FX570ES_PLUS);
        assert_eq!(Bus::read_data(&mut m, 0, KI_PORT as u16, 1), 0xFF);
        m.press(0x01, 0x01);

        // No KO line driven yet: KI stays high.
        assert_eq!(Bus::read_data(&mut m, 0, KI_PORT as u16, 1), 0xFF);

        // Drive KO0 while mask leaves it enabled: KI goes low for bit 0.
        Bus::write_data(&mut m, 0, KO_DRIVE_LO as u16, 1, 0x01);
        Bus::write_data(&mut m, 0, KO_MASK_LO as u16, 1, 0x80);
        assert_eq!(Bus::read_data(&mut m, 0, KI_PORT as u16, 1), 0xFE);

        // Masking KO0 hides the key again.
        Bus::write_data(&mut m, 0, KO_MASK_LO as u16, 1, 0x7F);
        assert_eq!(Bus::read_data(&mut m, 0, KI_PORT as u16, 1), 0xFF);

        m.release();
        assert_eq!(Bus::read_data(&mut m, 0, KI_PORT as u16, 1), 0xFF);
    }

    // --- Real-hardware peripherals (ROM-free) --------------------------------

    fn hardware_peripheral_machine() -> Machine {
        let mut rom = vec![0u8; 0x20000];
        // Interrupt vectors: source 5 -> 0x5678, source 9 -> 0x1234.
        rom[0x0A] = 0x78;
        rom[0x0B] = 0x56;
        rom[0x12] = 0x34;
        rom[0x13] = 0x12;
        Machine::with_model(rom, model::FX570ES_PLUS)
    }

    fn enabled_cpu(pc: u16) -> Cpu {
        let mut cpu = Cpu::new();
        cpu.regs.pc = pc;
        cpu.regs.csr = 1;
        cpu.regs.psw = 0x08; // MIE
        cpu
    }

    #[test]
    fn keyboard_irq_sets_pending_and_vectors_to_source_5() {
        let mut m = hardware_peripheral_machine();
        let mut cpu = enabled_cpu(0x4000);
        Bus::write_data(&mut m, 0, model::INT_MASK as u16, 1, 1 << 1);
        Bus::write_data(&mut m, 0, model::KO_FILTER as u16, 1, 0xFF);
        // A key pressed while the input filter is zero must not raise.
        Bus::write_data(&mut m, 0, model::KO_FILTER as u16, 1, 0);
        m.press(0x01, 0x01);
        m.tick_peripherals(&mut cpu);
        assert_eq!(m.sfr16_masked(model::INT_PENDING), 0);
        assert_eq!(cpu.regs.pc, 0x4000);

        // Re-press with the filter enabled: pending bit 1 and vector 0x5678.
        m.release();
        Bus::write_data(&mut m, 0, model::KO_FILTER as u16, 1, 0xFF);
        m.press(0x01, 0x01);
        m.tick_peripherals(&mut cpu);
        assert_eq!(m.sfr16_masked(model::INT_PENDING), 1 << 1);
        assert_eq!(cpu.regs.pc, 0x5678);
        assert_eq!(cpu.regs.csr, 0);
        assert_eq!(cpu.regs.elr[1], 0x4000);
        assert_eq!(cpu.regs.ecsr[1], 1);
        assert_eq!(cpu.regs.psw & 0x08, 0); // MIE cleared
        assert_eq!(cpu.regs.psw & 0x03, 1); // exception level 1
    }

    #[test]
    fn masked_or_disabled_irqs_are_not_delivered() {
        let mut m = hardware_peripheral_machine();
        let mut cpu = enabled_cpu(0x4000);
        Bus::write_data(&mut m, 0, model::KO_FILTER as u16, 1, 0xFF);
        m.press(0x01, 0x01);
        // No SFR mask yet.
        m.tick_peripherals(&mut cpu);
        assert_eq!(m.sfr16_masked(model::INT_PENDING), 0);
        assert_eq!(cpu.regs.pc, 0x4000);

        // Enable the source but clear MIE: pending is latched, no vectoring.
        Bus::write_data(&mut m, 0, model::INT_MASK as u16, 1, 1 << 1);
        cpu.regs.psw &= !0x08;
        m.tick_peripherals(&mut cpu);
        assert_eq!(m.sfr16_masked(model::INT_PENDING), 1 << 1);
        assert_eq!(cpu.regs.pc, 0x4000);
    }

    #[test]
    fn timer_counts_and_raises_source_9() {
        let mut m = hardware_peripheral_machine();
        let mut cpu = enabled_cpu(0x4000);
        Bus::write_data(&mut m, 0, model::INT_MASK as u16, 1, 1 << 5);
        Bus::write_data(&mut m, 0, model::TIMER_INTERVAL as u16, 1, 2);
        Bus::write_data(&mut m, 0, model::TIMER_CONTROL as u16, 1, 1);
        let mut delivered = false;
        for _ in 0..200 {
            m.tick_peripherals(&mut cpu);
            if cpu.regs.pc == 0x1234 {
                delivered = true;
                break;
            }
        }
        assert!(delivered, "timer IRQ never reached vector 0x1234");
        assert_ne!(m.sfr16_masked(model::INT_PENDING) & (1 << 5), 0);
    }

    #[test]
    fn timer_sfr_write_semantics() {
        let mut m = hardware_peripheral_machine();
        // Writing either counter byte clears both.
        Bus::write_data(&mut m, 0, model::TIMER_COUNTER as u16, 1, 0x12);
        Bus::write_data(&mut m, 0, model::TIMER_COUNTER as u16 + 1, 1, 0x34);
        assert_eq!(m.sfr16(model::TIMER_COUNTER), 0x3400);
        Bus::write_data(&mut m, 0, model::TIMER_COUNTER as u16, 1, 0xAA);
        assert_eq!(m.sfr16(model::TIMER_COUNTER), 0);

        // Interval zero is forced to one.
        Bus::write_data(&mut m, 0, model::TIMER_INTERVAL as u16, 1, 0);
        Bus::write_data(&mut m, 0, model::TIMER_INTERVAL as u16 + 1, 1, 0);
        assert_eq!(m.sfr16(model::TIMER_INTERVAL), 1);

        // Only 13 interrupt bits exist; high SFR bits are ignored.
        Bus::write_data(&mut m, 0, model::INT_MASK as u16 + 1, 1, 0xFF);
        assert_eq!(m.sfr16_masked(model::INT_MASK), 0x1F00);
        Bus::write_data(&mut m, 0, model::INT_PENDING as u16 + 1, 1, 0xFF);
        assert_eq!(m.sfr16_masked(model::INT_PENDING), 0x1F00);
    }

    #[test]
    fn ko_mask_is_ten_bits_and_pd_is_read_only() {
        let mut m = hardware_peripheral_machine();
        Bus::write_data(&mut m, 0, model::KO_MASK_HI as u16, 1, 0xFF);
        assert_eq!(m.ko_mask, 0x0300);
        assert_eq!(Bus::read_data(&mut m, 0, model::KO_MASK_HI as u16, 1), 0x03);
        Bus::write_data(&mut m, 0, model::KO_DRIVE_HI as u16, 1, 0xFF);
        assert_eq!(m.ko_drive, 0x0300);
        assert_eq!(Bus::read_data(&mut m, 0, model::PD_PORT as u16, 1), 0);
        Bus::write_data(&mut m, 0, model::PD_PORT as u16, 1, 0xAA);
        assert_eq!(Bus::read_data(&mut m, 0, model::PD_PORT as u16, 1), 0);
    }

    #[test]
    fn standby_stops_cpu_until_an_interrupt() {
        let mut m = hardware_peripheral_machine();
        let mut cpu = enabled_cpu(0x4000);
        Bus::write_data(&mut m, 0, model::STPACP as u16, 1, 0x50);
        Bus::write_data(&mut m, 0, model::STPACP as u16, 1, 0xA0);
        Bus::write_data(&mut m, 0, model::SBYCON as u16, 1, 0x02);
        for _ in 0..10 {
            m.tick(&mut cpu);
        }
        assert_eq!(cpu.regs.pc, 0x4000);
        assert_eq!(cpu.cycles, 0);

        // An accepted keyboard IRQ resumes execution.
        Bus::write_data(&mut m, 0, model::INT_MASK as u16, 1, 1 << 1);
        Bus::write_data(&mut m, 0, model::KO_FILTER as u16, 1, 0xFF);
        m.press(0x01, 0x01);
        m.tick(&mut cpu);
        assert!((0x5678..0x5680).contains(&cpu.regs.pc));
        assert!(cpu.cycles > 0);
    }

    #[test]
    fn emulator_mailbox_model_ignores_hardware_irqs() {
        let mut m = Machine::new(vec![0u8; 0x20000]);
        let mut cpu = enabled_cpu(0x4000);
        Bus::write_data(&mut m, 0, model::INT_MASK as u16, 1, 0xFF);
        Bus::write_data(&mut m, 0, model::KO_FILTER as u16, 1, 0xFF);
        Bus::write_data(&mut m, 0, model::TIMER_CONTROL as u16, 1, 1);
        m.press(0x01, 0x01);
        for _ in 0..100 {
            m.tick(&mut cpu);
        }
        assert_eq!(m.sfr16_masked(model::INT_PENDING), 0);
        assert_ne!(cpu.regs.pc, 0x4000); // the core keeps stepping normally
        assert_ne!(cpu.regs.pc, 0x5678); // and never vectors
    }

    #[test]
    fn emulator_mailbox_keyboard_still_queues_a_key() {
        let mut m = Machine::new(vec![0; 0x20000]);
        m.press(0x01, 0x01);
        m.keypad[0] = 0x90; // ready flag set
        let mut cpu = Cpu::new();
        m.tick(&mut cpu);
        assert_eq!(m.keypad[1], 0x01);
        assert_eq!(m.keypad[2], 0x01);
        assert!(m.pending_key.is_none());
    }

    // --- fx-570VN X / ClassWiz ------------------------------------------------

    fn vnx_machine() -> Machine {
        Machine::with_model(vec![0; 0x40000], model::FX570VNX)
    }

    #[test]
    fn vnx_rom_is_256k_and_resized() {
        let m = Machine::with_model(vec![0xAA; 16], model::FX570VNX);
        assert_eq!(m.rom.len(), 0x40000);
        assert_eq!(m.rom[0], 0xAA);
        assert_eq!(m.rom[16], 0xFF);
    }

    #[test]
    fn vnx_memory_map_boundaries() {
        let mut m = vnx_machine();
        // Each 4 KiB block gets a distinct byte so ROM pages can be told apart.
        for (i, b) in m.rom.iter_mut().enumerate() {
            *b = ((i >> 12) & 0xFF) as u8;
        }

        // Segment 0: only the first 0xD000 bytes are ROM; then 8 KiB RAM,
        // then the SFR page.
        assert_eq!(Bus::read_data(&mut m, 0, 0x0000, 1), m.rom[0] as u64);
        assert_eq!(Bus::read_data(&mut m, 0, 0xCFFF, 1), m.rom[0xCFFF] as u64);
        Bus::write_data(&mut m, 0, 0xD000, 1, 0x5A);
        Bus::write_data(&mut m, 0, 0xEFFF, 1, 0xA5);
        assert_eq!(m.ram[0], 0x5A);
        assert_eq!(m.ram[0x1FFF], 0xA5);
        assert_eq!(Bus::read_data(&mut m, 0, 0xF000, 1), 0x00); // SFR page

        // Code segments 1..3 are direct images of ROM pages 1..3.
        assert_eq!(Bus::read_code(&mut m, 1, 0, 1), m.rom[0x10000] as u64);
        assert_eq!(Bus::read_code(&mut m, 2, 0, 1), m.rom[0x20000] as u64);
        assert_eq!(Bus::read_code(&mut m, 3, 0, 1), m.rom[0x30000] as u64);
        // Segment 5 mirrors ROM page 0.
        assert_eq!(Bus::read_code(&mut m, 5, 0, 1), m.rom[0] as u64);
        assert_eq!(Bus::read_data(&mut m, 5, 0, 1), m.rom[0] as u64);
        // Unmapped segments read as erased ROM.
        assert_eq!(Bus::read_code(&mut m, 4, 0, 1), 0xFF);

        // csr_mask = 0x0F drops high CSR bits: 0x15 aliases segment 5 (mirror),
        // 0x14 aliases unmapped segment 4.  DSR is deliberately not masked
        // (the ES PLUS board still needs data segment 8).
        assert_eq!(Bus::read_code(&mut m, 0x15, 0, 1), m.rom[0] as u64);
        assert_eq!(Bus::read_code(&mut m, 0x14, 0, 1), 0xFF);
    }

    #[test]
    fn vnx_vram_render_is_192x64() {
        let mut m = vnx_machine();
        // ClassWiz only renders the dot matrix in screen mode 5.
        m.sfr[(model::SCREEN_MODE - model::SFR_BASE) as usize] = 5;
        let lcd = m.render();
        assert_eq!(lcd.len(), 192 * 64);

        // Dot-matrix row 1 starts at VRAM offset 32 (one status row of 32
        // bytes precedes it).  Set the top-left dot and a dot at x=8.
        let base = 0x800 + 32;
        m.sfr[base] = 0x80; // x=0 of row 1
        m.sfr[base + 1] = 0x80; // x=8 of row 1
        let lcd = m.render();
        assert_eq!(lcd[192], 1);
        assert_eq!(lcd[192 + 8], 1);
        assert_eq!(lcd[192 + 1], 0);

        // Row stride is 32 bytes, of which only 24 are displayed.
        m.sfr[base + 32] = 0x40; // x=1 of row 2
        let lcd = m.render();
        assert_eq!(lcd[2 * 192 + 1], 1);
        // Byte 24 is outside the displayed area.
        m.sfr[base + 24] = 0xFF;
        assert_eq!(m.render()[192 + 192], 0);
    }

    #[test]
    fn vnx_keyboard_uses_hardware_sfrs() {
        let mut m = vnx_machine();
        assert_eq!(m.model.keyboard, KeyboardKind::HardwareSfr);
        assert_eq!(Bus::read_data(&mut m, 0, KI_PORT as u16, 1), 0xFF);

        // '7' on the ClassWiz matrix: KI bit 2, KO bit 0.
        let (ki, ko) = crate::keys::pc_key("7").unwrap();
        assert_eq!((ki, ko), (0x04, 0x01));
        m.press(ki, ko);
        assert_eq!(Bus::read_data(&mut m, 0, KI_PORT as u16, 1), 0xFF);
        Bus::write_data(&mut m, 0, KO_DRIVE_LO as u16, 1, ko as u64);
        assert_eq!(Bus::read_data(&mut m, 0, KI_PORT as u16, 1), (!ki) as u64);

        // EXE at KO bit 6, KI bit 0.
        let (ki, ko) = crate::keys::pc_key("EXE").unwrap();
        assert_eq!((ki, ko), (0x01, 0x40));
    }

    #[test]
    fn vnx_mailbox_is_not_emulated() {
        let m = vnx_machine();
        // Writing to the emulator mailbox address (0x48E00) must not change
        // any SFR or RAM state on a real-hardware model.
        let before = m.checksum();
        let mut m = m;
        Bus::write_data(&mut m, 4, 0x8E00, 1, 0xAB);
        assert_eq!(m.checksum(), before);
    }

    // --- ClassWiz (HW id 4) peripherals (ROM-free) --------------------------

    #[test]
    fn classwiz_code_segment_0_covers_the_whole_rom_page() {
        let mut m = vnx_machine();
        // Above 0xCFFF, data segment 0 is RAM/SFR, but code segment 0 is the
        // whole 64 KiB ROM page.  The firmware calls code at 0xD736 and 0xFFFE.
        for (a, b) in [(0xD736u32, 0xC8u8), (0xFFFE, 0xFE)] {
            m.rom[a as usize] = b;
            assert_eq!(Bus::read_code(&mut m, 0, a as u16, 1), b as u64);
        }
        // Data accesses in the same window still hit RAM/SFR.
        Bus::write_data(&mut m, 0, 0xD736, 1, 0x5A);
        assert_eq!(m.ram[0x736], 0x5A);
        assert_eq!(Bus::read_code(&mut m, 0, 0xD736, 1), 0xC8);
    }

    #[test]
    fn classwiz_timer_raises_interrupt_after_prescaled_interval() {
        let mut m = vnx_machine();
        let mut cpu = Cpu::new();
        cpu.reset(&mut m);
        // interval = 10, prescaler = 2^1 = 2, timer enabled.
        Bus::write_data(&mut m, 0, model::CW_TIMER_INTERVAL as u16, 2, 10);
        Bus::write_data(&mut m, 0, model::CW_TIMER_PRESCALER as u16, 1, 1);
        Bus::write_data(&mut m, 0, model::CW_TIMER_COUNTER as u16, 2, 0);
        Bus::write_data(&mut m, 0, model::CW_TIMER_CONTROL as u16, 1, 1);
        assert_eq!(m.sfr_byte(model::CW_INT_PENDING), 0);
        for _ in 0..19 {
            m.tick(&mut cpu);
        }
        assert_eq!(m.sfr_byte(model::CW_INT_PENDING), 0);
        m.tick(&mut cpu);
        assert_ne!(
            m.sfr_byte(model::CW_INT_PENDING) & (model::CW_IRQ_TIMER_BIT as u8),
            0
        );
        assert_eq!(m.sfr_byte(model::CW_TIMER_COUNTER), 0);
    }

    #[test]
    fn classwiz_key_press_raises_keyboard_interrupt() {
        let mut m = vnx_machine();
        assert_eq!(m.sfr_byte(model::CW_INT_PENDING), 0);
        m.press(0x01, 0x01);
        assert_eq!(
            m.sfr_byte(model::CW_INT_PENDING) & model::CW_IRQ_KEYBOARD_BIT as u8,
            model::CW_IRQ_KEYBOARD_BIT as u8
        );
        // A second press while held must not retrigger the edge interrupt.
        m.sfr[model::CW_INT_PENDING as usize - model::SFR_BASE as usize] = 0;
        m.press(0x01, 0x01);
        assert_eq!(m.sfr_byte(model::CW_INT_PENDING), 0);
        m.release();
        m.press(0x01, 0x01);
        assert_ne!(m.sfr_byte(model::CW_INT_PENDING), 0);
    }

    #[test]
    fn classwiz_screen_mode_controls_rendering() {
        let mut m = vnx_machine();
        let base = (0xF800 - model::SFR_BASE) as usize;
        m.sfr[base + 32] = 0x80 | 0x01; // x=0 and x=7 of the first dot row

        for mode in [0u8, 1, 2, 3, 7] {
            m.sfr[(model::SCREEN_MODE - model::SFR_BASE) as usize] = mode;
            assert_eq!(
                m.render().iter().filter(|&&p| p != 0).count(),
                0,
                "mode {mode}"
            );
        }
        for mode in [4u8, 5, 6] {
            m.sfr[(model::SCREEN_MODE - model::SFR_BASE) as usize] = mode;
            let lcd = m.render();
            if mode == 5 {
                assert_eq!(lcd[192], 1);
                assert_eq!(lcd[192 + 7], 1);
            } else {
                assert_eq!(lcd.iter().filter(|&&p| p != 0).count(), 0);
            }
        }
    }

    #[test]
    fn classwiz_standby_stops_cpu_until_interrupt() {
        let mut m = vnx_machine();
        let mut cpu = Cpu::new();
        cpu.reset(&mut m);
        // Arm STOP with the 0x50 -> 0xA0 handshake, then request it.
        Bus::write_data(&mut m, 0, 0xF008, 1, 0x50);
        Bus::write_data(&mut m, 0, 0xF008, 1, 0xA0);
        Bus::write_data(&mut m, 0, 0xF009, 1, 0x02);
        let cycles = cpu.cycles;
        for _ in 0..100 {
            m.tick(&mut cpu);
        }
        assert_eq!(cpu.cycles, cycles);
        // A keyboard edge wakes the chipset even though the CPU is stopped.
        m.press(0x01, 0x01);
        m.tick(&mut cpu);
        assert!(cpu.cycles > cycles);
    }

    #[test]
    fn es_plus_board_has_no_classwiz_peripherals() {
        let mut m = Machine::new(vec![0; 0x20000]);
        let mut cpu = Cpu::new();
        cpu.reset(&mut m);
        assert!(!m.has_classwiz_peripherals());
        // ES PLUS never sees the ClassWiz timer or screen-mode logic.
        Bus::write_data(&mut m, 0, model::TIMER_CONTROL as u16, 1, 1);
        m.sfr[(model::SCREEN_MODE - model::SFR_BASE) as usize] = 0;
        m.sfr[(VRAM_BASE - model::SFR_BASE as u16) as usize] = 0x80;
        for _ in 0..10 {
            m.tick(&mut cpu);
        }
        assert_eq!(m.render()[0], 1);
    }

    // --- raw RAM snapshots ---------------------------------------------------

    #[test]
    fn load_ram_into_es_plus_window() {
        let mut m = Machine::new(vec![0; 0x20000]);
        let data: Vec<u8> = (0..model::ESPLUS_RAM_SIZE).map(|i| i as u8).collect();
        assert_eq!(m.ram_base(), model::ESPLUS_RAM_BASE);
        assert_eq!(m.load_ram(&data), model::ESPLUS_RAM_SIZE);
        assert_eq!(m.ram[0], 0);
        assert_eq!(m.ram[0x100], 0x00); // 0x100 mod 256
        assert_eq!(m.ram[model::ESPLUS_RAM_SIZE - 1], 0xFF);
        // Visible through the data bus at DSR 0.
        assert_eq!(Bus::read_data(&mut m, 0, 0x08000, 1), 0);
        assert_eq!(Bus::read_data(&mut m, 0, 0x08100, 1), 0x00);
        // The mailbox just above RAM is untouched.
        assert!(m.keypad.iter().all(|&b| b == 0));
    }

    #[test]
    fn load_ram_into_classwiz_window() {
        let mut m = vnx_machine();
        let data: Vec<u8> = (0..model::CLASSWIZ_RAM_SIZE)
            .map(|i| (i >> 4) as u8)
            .collect();
        assert_eq!(m.ram_base(), model::CLASSWIZ_RAM_BASE);
        assert_eq!(m.load_ram(&data), model::CLASSWIZ_RAM_SIZE);
        assert_eq!(Bus::read_data(&mut m, 0, 0x0D000, 1), 0);
        assert_eq!(Bus::read_data(&mut m, 0, 0x0DABC, 1), 0x0AB);
        assert_eq!(Bus::read_data(&mut m, 0, 0x0EFFF, 1), 0xFF);
        // The SFR page just above RAM is untouched.
        assert!(m.sfr.iter().all(|&b| b == 0));
    }

    #[test]
    fn load_ram_clips_oversized_data_and_replaces_previous_contents() {
        let mut m = vnx_machine();
        let big = vec![0x5A; model::CLASSWIZ_RAM_SIZE + 0x1000];
        assert_eq!(m.load_ram(&big), model::CLASSWIZ_RAM_SIZE);
        assert!(m.ram[..model::CLASSWIZ_RAM_SIZE].iter().all(|&b| b == 0x5A));
        assert!(m.ram[model::CLASSWIZ_RAM_SIZE..].iter().all(|&b| b == 0));

        // A shorter snapshot clears bytes not covered by the new image.
        assert_eq!(m.load_ram(&[0x11, 0x22]), 2);
        assert_eq!(m.ram[0], 0x11);
        assert_eq!(m.ram[1], 0x22);
        assert_eq!(m.ram[2], 0);
        assert!(m.ram[model::CLASSWIZ_RAM_SIZE - 1] == 0);
    }

    #[test]
    fn load_ram_never_writes_outside_the_ram_window() {
        for model in [model::FX991ES_PLUS_C, model::FX570VNX] {
            let mut m = Machine::with_model(vec![0; model.rom_size], model);
            let before_rom = m.rom.clone();
            let before = m.checksum();
            m.load_ram(&[0xEE; 0x10000]);
            assert_eq!(m.rom, before_rom);
            // Only the window (0xE00 or 0x2000 bytes) changed; the checksum
            // difference is bounded by the rest of the 0x8000-byte array.
            assert_ne!(m.checksum(), before);
            assert!(m.sfr.iter().all(|&b| b == 0));
            assert!(m.keypad.iter().all(|&b| b == 0));
        }
    }

    // --- ClassWiz II (CW ii, hardware id 5) ---------------------------------

    fn cw2_machine() -> Machine {
        Machine::with_model(vec![0; 0x100000], model::FX991CNCW)
    }

    #[test]
    fn cw2_geometry_and_rom_padding() {
        let m = Machine::with_model(vec![0xC3; 4], model::FX991CW);
        assert_eq!(m.rom.len(), 0x80000);
        assert_eq!(m.rom[0], 0xC3);
        assert_eq!(m.rom[4], 0xFF);
        assert_eq!(m.ram_base(), 0x9000);
        assert_eq!(m.ram_size(), 0x6000);
        assert_eq!(m.rom_mirror_base(), 0x80000);
        assert_eq!(m.vram_geometry(), (32, 24));
        assert_eq!(m.model.lcd_width, 192);
        assert_eq!(m.model.lcd_height, 64);
    }

    #[test]
    fn cw2_data_memory_map() {
        let mut m = cw2_machine();
        for (i, b) in m.rom.iter_mut().enumerate() {
            *b = ((i >> 12) & 0xFF) as u8;
        }
        let tag = |addr: u32| -> u8 { ((addr >> 12) & 0xFF) as u8 };

        // Segment 0: ROM only below 0x9000; 24 KiB RAM; SFR page.
        assert_eq!(Bus::read_data(&mut m, 0, 0x0000, 1), 0);
        assert_eq!(Bus::read_data(&mut m, 0, 0x8FFF, 1), tag(0x8FFF) as u64);
        Bus::write_data(&mut m, 0, 0x9000, 1, 0x5A);
        Bus::write_data(&mut m, 0, 0xEFFF, 1, 0xA5);
        assert_eq!(m.ram[0], 0x5A);
        assert_eq!(m.ram[0x5FFF], 0xA5);
        assert_eq!(Bus::read_data(&mut m, 0, 0x9000, 1), 0x5A);
        assert_eq!(Bus::read_data(&mut m, 0, 0xEFFF, 1), 0xA5);
        assert_eq!(Bus::read_data(&mut m, 0, 0xF000, 1), 0x00);

        for dsr in [1u8, 2, 3, 4, 5, 6, 7, 9, 10, 11, 12, 13, 14, 15] {
            assert_eq!(
                Bus::read_data(&mut m, dsr, 0x1234, 1),
                tag(((dsr as u32) << 16) | 0x1234) as u64,
                "data segment {dsr}"
            );
        }
        assert_eq!(Bus::read_data(&mut m, 8, 0x2345, 1), tag(0x82345) as u64);
        assert_eq!(Bus::read_data(&mut m, 8, 0x9000, 1), 0); // VRAM2

        Bus::write_data(&mut m, 8, 0x2345, 1, 0xFF);
        assert_eq!(m.rom[0x82345], tag(0x82345));
    }

    #[test]
    fn cw2_code_segment_decode() {
        let mut m = cw2_machine();
        for (i, b) in m.rom.iter_mut().enumerate() {
            *b = ((i >> 12) & 0xFF) as u8;
        }
        let tag = |addr: u32| -> u8 { ((addr >> 12) & 0xFF) as u8 };
        for csr in 0u8..=5 {
            assert_eq!(
                Bus::read_code(&mut m, csr, 0x1234, 1),
                tag(((csr as u32) << 16) | 0x1234) as u64,
                "code segment {csr}"
            );
        }
        assert_eq!(Bus::read_code(&mut m, 8, 0x1234, 1), tag(0x1234) as u64);
        assert_eq!(Bus::read_code(&mut m, 9, 0x1234, 1), tag(0x11234) as u64);
        assert_eq!(Bus::read_code(&mut m, 0x15, 0, 1), tag(0x50000) as u64);
        assert_eq!(Bus::read_code(&mut m, 6, 0, 1), 0xFF);
        assert_eq!(Bus::read_code(&mut m, 7, 0x2000, 1), 0xFF);
        assert_eq!(Bus::read_code(&mut m, 0, 0xFE00, 1), 0xFF);
        assert_eq!(Bus::read_code(&mut m, 7, 0x1FFF, 1), tag(0x71FFF) as u64);
    }

    #[test]
    fn cw2_dual_framebuffer_and_rendering() {
        let mut m = cw2_machine();
        assert_eq!(m.render().len(), 192 * 64);
        assert!(m.render().iter().all(|&p| p == 0));

        Bus::write_data(&mut m, 0, 0xF800 + 32, 1, 0x80);
        assert_eq!(m.render()[192], 1);
        Bus::write_data(&mut m, 0, 0xF800 + 32 + 24, 1, 0xFF);
        assert_eq!(m.render()[192 + 192], 0);

        Bus::write_data(&mut m, 0, 0xF037, 1, 0x04);
        assert_eq!(Bus::read_data(&mut m, 0, 0xF800 + 32, 1), 0);
        Bus::write_data(&mut m, 0, 0xF800 + 32, 1, 0x40);
        assert_eq!(Bus::read_data(&mut m, 8, 0x9000 + 32, 1), 0x40);
        assert_eq!(m.render()[192 + 1], 1);
        Bus::write_data(&mut m, 8, 0x9000 + 32, 1, 0x01);
        assert_eq!(m.render()[192 + 7], 1);
    }

    #[test]
    fn cw2_interrupt_and_timer_sfrs() {
        let mut m = cw2_machine();
        Bus::write_data(&mut m, 0, 0xF010, 1, 0x22);
        assert_eq!(Bus::read_data(&mut m, 0, 0xF010, 1), 0x22);
        Bus::write_data(&mut m, 0, 0xF014, 1, 0x20);
        assert_eq!(Bus::read_data(&mut m, 0, 0xF014, 1), 0x20);
        Bus::write_data(&mut m, 0, 0xF013, 1, 0xFF);
        assert_eq!(Bus::read_data(&mut m, 0, 0xF013, 1), 0x00);

        Bus::write_data(&mut m, 0, 0xF020, 2, 0x079E);
        assert_eq!(Bus::read_data(&mut m, 0, 0xF020, 2), 0x079E);
        Bus::write_data(&mut m, 0, 0xF025, 1, 1);
        assert_eq!(m.cw.timer_control, 1);

        Bus::write_data(&mut m, 0, 0xF031, 1, 0x57);
        assert_eq!(Bus::read_data(&mut m, 0, 0xF031, 1), 0x07);
        Bus::write_data(&mut m, 0, 0xF032, 1, 0xFF);
        assert_eq!(Bus::read_data(&mut m, 0, 0xF032, 1), 0x3F);
    }

    #[test]
    fn cw2_keyboard_uses_hardware_sfrs() {
        let mut m = cw2_machine();
        assert_eq!(Bus::read_data(&mut m, 0, KI_PORT as u16, 1), 0xFF);
        m.press(0x04, 0x01);
        assert!(m.cw_key_irq_pending);
        assert_eq!(Bus::read_data(&mut m, 0, KI_PORT as u16, 1), 0xFF);
        Bus::write_data(&mut m, 0, KO_DRIVE_LO as u16, 1, 0x01);
        Bus::write_data(&mut m, 0, KO_MASK_LO as u16, 1, 0x80);
        assert_eq!(Bus::read_data(&mut m, 0, KI_PORT as u16, 1), 0xFB);
        m.release();
        assert_eq!(Bus::read_data(&mut m, 0, KI_PORT as u16, 1), 0xFF);
    }

    #[test]
    fn cw2_dsr_sfr_reports_last_prefix() {
        let mut m = cw2_machine();
        Bus::read_data(&mut m, 0x12, 0x3456, 1);
        assert_eq!(Bus::read_data(&mut m, 0, 0xF000, 1), 0x12);
        Bus::write_data(&mut m, 0x08, 0x9000, 1, 0);
        assert_eq!(Bus::read_data(&mut m, 0, 0xF000, 1), 0x08);
    }

    #[test]
    fn cw2_battery_is_healthy_by_default() {
        let mut m = cw2_machine();
        // BLDFlag (bit 1) must be set; otherwise the firmware shows a dead
        // battery warning.
        assert_eq!(Bus::read_data(&mut m, 0, 0xF0D1, 1) as u8 & 0x02, 0x02);
    }

    #[test]
    fn cw2_battery_test_completes_and_stays_healthy() {
        let mut m = cw2_machine();
        let mut cpu = Cpu::new();
        Bus::write_data(&mut m, 0, 0xF0D1, 1, 0x01);
        assert_ne!(Bus::read_data(&mut m, 0, 0xF0D1, 1) as u8 & 0x01, 0);
        for _ in 0..600 {
            m.cw2_tick(&mut cpu);
        }
        let status = Bus::read_data(&mut m, 0, 0xF0D1, 1) as u8;
        assert_eq!(status & 0x02, 0x02, "battery flag must stay healthy");
        assert_eq!(status & 0x01, 0, "battery test must finish");
    }

    #[test]
    fn cw2_solar_indicator_depends_on_model() {
        let mut solar = Machine::with_model(vec![0; 0x100000], model::FX991CW);
        assert_ne!(Bus::read_data(&mut solar, 0, 0xF310, 1) as u8 & 0x10, 0);
        let mut battery_only = Machine::with_model(vec![0; 0x100000], model::FX9910CW);
        assert_eq!(
            Bus::read_data(&mut battery_only, 0, 0xF310, 1) as u8 & 0x10,
            0
        );
    }
}
