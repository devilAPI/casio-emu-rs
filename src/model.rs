//! Per-calculator-model configuration.
//!
//! One nX-U8/100 core can run firmware for several Casio calculators.  What
//! changes between models is the board around the CPU: ROM size, memory map,
//! LCD geometry and — most importantly — how key presses reach the firmware.
//!
//! The fx-991ES PLUS **C** ROM was dumped from Casio's *Windows emulator*
//! (`SimU8`).  That emulator exposes keys through a small mailbox at 0x8E00:
//! 0x8E00 = ready flag, 0x8E01 = KI, 0x8E02 = KO.
//!
//! ROMs dumped from a **real calculator** (such as the fx-570ES PLUS image
//! published in `ZaoHan415/fxesplus`, or the fx-570VN X / fx-580VN X image in
//! `user202729/fxesplus`) do not use that mailbox.  They scan the keyboard
//! through hardware SFRs: KI is read from 0xF040 (active low), the driven KO
//! lines come from 0xF046/0xF047 and 0xF044/0xF045 masks the KO lines reserved
//! for the model-ID (Pd) resistors.

use crate::keys::{self, CalcKey};

/// Self-test signature offset in the 128 KiB (ES PLUS) images.  Six ASCII
/// bytes: `GY454XE` on the fx-570ES PLUS, `LY727XA` on the fx-991ES PLUS C.
pub const SIGNATURE_OFFSET: usize = 0x1FFF4;

/// Self-test signature offset in the 256 KiB (ClassWiz) images.  The
/// fx-570VN X / fx-580VN X real-hardware ROM stores `CY-298` here.
pub const SIGNATURE_OFFSET_CLASSWIZ: usize = 0x3FFEE;

/// Self-test signature offset in the 1 MiB ClassWiz II image (`71CASG`).
pub const CW_SIGNATURE_OFFSET: usize = 0x8FDE0;

/// ClassWiz II version/checksum record offset (`EY-023B` + little-endian
/// checksum on the fx-880BTG).  Also present at 0x5FFEE and 0xF1FEE.
pub const CW_VERSION_OFFSET: usize = 0x071FEE;

/// How the firmware expects key presses to be delivered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyboardKind {
    /// Casio Windows-emulator mailbox: 0x8E00 ready, 0x8E01 KI, 0x8E02 KO.
    EmulatorMailbox,
    /// Real hardware SFRs: KI at 0xF040, KO drive at 0xF046/0xF047,
    /// KO mask at 0xF044/0xF045.
    HardwareSfr,
}

/// Board generation.  This selects the memory map, RAM placement and VRAM
/// geometry; the CPU core itself is identical for both.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoardKind {
    /// fx-ES PLUS: two 64 KiB code pages, 3.5 KiB RAM at 0x08000,
    /// 96x32 LCD with 16-byte VRAM rows.
    EsPlus,
    /// ClassWiz (HW_CLASSWIZ, hardware id 4): four 64 KiB code pages plus a
    /// mirror at 0x50000, 8 KiB RAM at 0x0D000, 192x64 LCD with 32-byte
    /// VRAM rows (24 display bytes + 8 unused per row).
    ClassWiz,
    /// ClassWiz II / "CW ii" (hardware id 5): 512 KiB or 1 MiB ROM, 24 KiB RAM
    /// at 0x9000, dual 192x64 framebuffers and a BCD coprocessor.  Supported
    /// by the extended memory map and interrupt controller in `machine`.
    ClassWizII,
}

// ---------------------------------------------------------------------------
// Board-derived constants (kept here so both the machine and the tests agree).
// ---------------------------------------------------------------------------

/// ES PLUS: data RAM window 0x08000..0x08DFF.
pub const ESPLUS_RAM_BASE: u32 = 0x08000;
pub const ESPLUS_RAM_SIZE: usize = 0x0E00;
/// ES PLUS: emulator key mailbox window 0x08E00..0x08E2F.
pub const ESPLUS_MAILBOX_BASE: u32 = 0x08E00;
pub const ESPLUS_MAILBOX_SIZE: usize = 0x30;
/// ES PLUS: ROM window mirror at data segment 8.
pub const ESPLUS_ROM_MIRROR_BASE: u32 = 0x80000;

/// ClassWiz: 8 KiB battery-backed RAM at data segment 0, offset 0xD000.
pub const CLASSWIZ_RAM_BASE: u32 = 0x0D000;
pub const CLASSWIZ_RAM_SIZE: usize = 0x2000;
/// ClassWiz: ROM window mirror at data segment 5 (ROM offset 0).
pub const CLASSWIZ_ROM_MIRROR_BASE: u32 = 0x50000;
/// Number of ROM bytes visible in code/data segment 0 (the rest is unmapped).
pub const CLASSWIZ_SEG0_SIZE: usize = 0x0D000;

/// ClassWiz II: 24 KiB battery-backed RAM at data segment 0, offset 0x9000.
pub const CLASSWIZ2_RAM_BASE: u32 = 0x09000;
pub const CLASSWIZ2_RAM_SIZE: usize = 0x6000;
/// ClassWiz II: ROM-window mirror in data segment 8 (code segment 8 instead
/// mirrors ROM bank 0).
pub const CLASSWIZ2_ROM_MIRROR_BASE: u32 = 0x80000;
/// ClassWiz II: second 192x64 framebuffer (0x800 bytes: status + 63 rows).
pub const CLASSWIZ2_VRAM2_BASE: u32 = 0x89000;
/// Size of one CW ii framebuffer (64 rows x 32 bytes).
pub const CLASSWIZ2_VRAM_SIZE: usize = 0x800;

/// Shared SFR window.  Both boards place the SFRs, VRAM and keyboard KI/KO
/// registers here; the difference is the RAM that precedes them in segment 0.
pub const SFR_BASE: u32 = 0x0F000;
pub const SFR_SIZE: usize = 0x1000;

// ClassWiz (hardware id 4) peripheral registers.  These deliberately use
// `CW_`-prefixed names: the ES PLUS fix below uses the same physical SFR
// addresses with a different bit layout (source - 4 rather than source + 1),
// and both models must keep their own semantics in the merged board code.
/// ClassWiz interrupt controller.  Bit `n+1` of the mask/pending registers
/// corresponds to maskable source `n` (interrupt vector `5 + n`); bit 0 is the
/// non-maskable source.  The firmware enables source 0 (keyboard) and source 4
/// (timer) at boot by writing `0x22` to the mask register.
pub const CW_INT_MASK: u32 = 0x0F010;
pub const CW_INT_PENDING: u32 = 0x0F014;
/// Source 0 (keyboard) pending/mask bit and interrupt vector index.
pub const CW_IRQ_KEYBOARD_BIT: u16 = 0x0002;
pub const CW_IRQ_KEYBOARD_INDEX: u8 = 5;
/// Source 4 (timer) pending/mask bit and interrupt vector index.
pub const CW_IRQ_TIMER_BIT: u16 = 0x0020;
pub const CW_IRQ_TIMER_INDEX: u8 = 9;

/// ClassWiz timer SFRs.  `F020` is the 16-bit interval, `F022` clears the
/// counter on write, bits 0..2 of `F024` select a divide-by-2^n clock and
/// bit 3 selects the high-speed source; `F025` bit 0 enables the timer.
pub const CW_TIMER_INTERVAL: u32 = 0x0F020;
pub const CW_TIMER_COUNTER: u32 = 0x0F022;
pub const CW_TIMER_PRESCALER: u32 = 0x0F024;
pub const CW_TIMER_CONTROL: u32 = 0x0F025;

/// Screen control SFRs.  The display is only drawn when the low three bits of
/// the mode register are 4 (blank), 5 (normal) or 6 (calculating).
pub const SCREEN_RANGE: u32 = 0x0F030;
pub const SCREEN_MODE: u32 = 0x0F031;
pub const SCREEN_CONTRAST: u32 = 0x0F032;

/// Hardware keyboard SFR addresses (identical on ES PLUS and ClassWiz).
pub const KI_PORT: u32 = 0x0F040;
pub const KO_FILTER: u32 = 0x0F042;
pub const KO_MASK_LO: u32 = 0x0F044;
pub const KO_MASK_HI: u32 = 0x0F045;
pub const KO_DRIVE_LO: u32 = 0x0F046;
pub const KO_DRIVE_HI: u32 = 0x0F047;
/// Model-ID (Pd) register.  Only the Casio Windows-emulator variants expose
/// it; real hardware leaves the KO7..9 sense lines unconnected.
pub const PD_PORT: u32 = 0x0F050;
/// KO drive/mask registers are 10 bits wide; KO7..9 are reserved for the
/// model-ID resistors and never connect to a calculator key.
pub const KO_BITS_MASK: u16 = 0x03FF;

/// Standby control (STPACP/SBYCON).  The key sequence 0x50 then 0xA0 followed
/// by SBYCON.1 puts the chip into STOP until an interrupt is accepted.
pub const STPACP: u32 = 0x0F008;
pub const SBYCON: u32 = 0x0F009;

/// Maskable interrupt control SFRs.  SFR bit = IRQ source - 4; source 5 is
/// the keyboard, source 9 the timer.
pub const INT_MASK: u32 = 0x0F010;
pub const INT_PENDING: u32 = 0x0F014;
pub const INT_MANAGED_BASE: usize = 4;
/// SFR bits 0..12 represent IRQ sources 4..16 (13 maskable sources).
pub const INT_MANAGED_COUNT: usize = 13;
pub const IRQ_KEYBOARD: usize = 5;
pub const IRQ_TIMER: usize = 9;

/// Timer SFRs: 16-bit interval, write-any-byte-to-clear counter, run bit.
pub const TIMER_INTERVAL: u32 = 0x0F020;
pub const TIMER_COUNTER: u32 = 0x0F022;
pub const TIMER_CONTROL: u32 = 0x0F025;

/// The nX-U8 in these calculators runs at 128 KiB/s (131072 cycles per
/// second); the timer prescaler divides that into 10000 ticks per second.
pub const CPU_CYCLES_PER_SECOND: u32 = 128 * 1024;
pub const TIMER_EXT_FREQUENCY: u32 = 10_000;

/// VRAM geometry: bytes per row and bytes actually displayed per row.
/// The ClassWiz LCD is 192x63 dots plus one icon/status row (64 rows total).
pub const ESPLUS_VRAM_ROW_STRIDE: usize = 16;
pub const ESPLUS_VRAM_DISP_BYTES: usize = 12; // 12 * 8 = 96 pixels
pub const CLASSWIZ_VRAM_ROW_STRIDE: usize = 32;
pub const CLASSWIZ_VRAM_DISP_BYTES: usize = 24; // 24 * 8 = 192 pixels

/// Everything that differs between supported calculators.
#[derive(Clone, Copy, Debug)]
pub struct Model {
    pub id: &'static str,
    pub name: &'static str,
    /// Expected ROM image size in bytes.
    pub rom_size: usize,
    /// ASCII self-test signature at [`signature_offset`].
    pub signature: &'static [u8],
    /// Offset of that signature inside the ROM image.
    pub signature_offset: usize,
    /// Board generation: selects the memory map and VRAM geometry.
    pub board: BoardKind,
    pub keyboard: KeyboardKind,
    /// Physical key matrix (KI/KO bitmasks plus on-screen hitboxes).
    pub keys: &'static [CalcKey],
    /// LCD width in pixels.
    pub lcd_width: usize,
    /// LCD height in pixels (including the status row).
    pub lcd_height: usize,
    /// Base address of the VRAM in the SFR window.
    pub vram_base: u16,
    /// Key mailbox base, present only on emulator-derived firmware.
    pub key_mailbox: u16,
    /// Bits of the code segment register that are implemented.
    pub csr_mask: u8,
}

impl Model {
    /// Name of the public ROM dump for this model inside `roms/`.
    ///
    /// Most dumps use the model id verbatim; the fx-82EX dump is published
    /// under its hardware revision suffix.
    pub fn rom_filename(&self) -> String {
        let stem = match self.id {
            "fx82ex" => "fx82ex-vere",
            other => other,
        };
        format!("{stem}.rom.bin")
    }

    /// Whether this build's board emulation can run the firmware.  CW ii is
    /// supported by the extended memory map, dual framebuffers and interrupt
    /// controller in [`crate::machine::Machine`].
    pub fn execution_supported(&self) -> bool {
        true
    }
}

/// Casio fx-991ES PLUS C — the original supported model.  Its firmware comes
/// from the official `fx-ES PLUS Emulator Ver. 4` and uses the SimU8 mailbox.
pub const FX991ES_PLUS_C: Model = Model {
    id: "fx991es-plus-c",
    name: "Casio fx-991ES PLUS C",
    rom_size: 0x20000,
    signature: b"LY727X",
    signature_offset: SIGNATURE_OFFSET,
    board: BoardKind::EsPlus,
    keyboard: KeyboardKind::EmulatorMailbox,
    keys: keys::KEYS,
    lcd_width: 96,
    lcd_height: 32,
    vram_base: 0xF800,
    key_mailbox: 0x8E00,
    csr_mask: 0x01,
};

/// Casio fx-570ES PLUS — real-hardware ROM published in
/// `ZaoHan415/fxesplus` (`dumps/rom.bin`, SHA-256
/// `1a1b575f130b3fa4c79a598881181f5a137edb45e398cd9b78ae5baf9b071071`).
/// Same CPU, RAM and LCD as the C model, but keys arrive through the hardware
/// KI/KO SFRs instead of the emulator mailbox.
pub const FX570ES_PLUS: Model = Model {
    id: "fx570es-plus",
    name: "Casio fx-570ES PLUS",
    rom_size: 0x20000,
    signature: b"GY454X",
    signature_offset: SIGNATURE_OFFSET,
    board: BoardKind::EsPlus,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::KEYS,
    lcd_width: 96,
    lcd_height: 32,
    vram_base: 0xF800,
    // Not used by this model; kept for a uniform descriptor.
    key_mailbox: 0x8E00,
    csr_mask: 0x01,
};

/// Casio fx-570VN X (ClassWiz generation).  The published ROM is the
/// fx-580VN X real-hardware image from `user202729/fxesplus` (`580vnx/rom.bin`,
/// git blob `3105c5e44b6795fe57c36ab9eef18e020fea4699`, SHA-256
/// `7a164c650d6e6bc89e220caf8ac8349724adc2021e59649d428adae6b509e267`).
/// The fx-570VN X and fx-580VN X share this ROM image/hardware (both are
/// hardware id 4 / HW_CLASSWIZ, `real_hardware = 1`, `pd_value = 0`).
///
/// Differences from the ES PLUS board: 256 KiB ROM, four 64 KiB code segments
/// plus a mirror at 0x50000 (`csr_mask = 0x0F`), 8 KiB RAM at 0x0D000, a
/// 192x64 VRAM (63 dot rows + one icon row, 32-byte stride with 24 bytes
/// displayed), and the larger ClassWiz keypad layout.  Keys still arrive via
/// the KI/KO hardware SFRs at 0xF040/0xF046.
pub const FX570VNX: Model = Model {
    id: "fx570vnx",
    name: "Casio fx-570VN X (ClassWiz)",
    rom_size: 0x40000,
    signature: b"CY-298",
    signature_offset: SIGNATURE_OFFSET_CLASSWIZ,
    board: BoardKind::ClassWiz,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::VNX_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    // Unused on real hardware.  The Casio Windows-emulator variant of this
    // ROM moves the mailbox to 0x40000 + 0x8E00 (0x48E00, outside u16 range);
    // we do not emulate that image.
    key_mailbox: 0x8E00,
    csr_mask: 0x0F,
};

/// Casio fx-82ES PLUS A — real-hardware ROM from `user202729/fxesplus`
/// (`82espa/rom.bin`, SHA-256
/// `ab58e84c8ea265b81fb9eda9a590ee0e3c35f62e3f07b35c7431fd0ad03bf384`).
/// Same ES PLUS board and 49-key matrix as the fx-570ES PLUS.
pub const FX82ES_PLUS_A: Model = Model {
    id: "fx82esplusa",
    name: "Casio fx-82ES PLUS A",
    rom_size: 0x20000,
    signature: b"LY713X",
    signature_offset: SIGNATURE_OFFSET,
    board: BoardKind::EsPlus,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::KEYS,
    lcd_width: 96,
    lcd_height: 32,
    vram_base: 0xF800,
    key_mailbox: 0x8E00,
    csr_mask: 0x01,
};

/// Casio fx-82ES PLUS — ROM from Casio's `fx-ES PLUS Emulator`
/// (`user202729/fxesplus`, `82esp/emu_rom.bin`, SHA-256
/// `a1458a72fbdd9452b31cd791605e087c3a52191a4b9955d1a34c2c71aaeae35d`).
/// Uses the 0x8E00 mailbox like the fx-991ES PLUS C.
pub const FX82ESP_EMU: Model = Model {
    id: "fx82esp-emu",
    name: "Casio fx-82ES PLUS (emulator ROM)",
    rom_size: 0x20000,
    signature: b"GY450X",
    signature_offset: SIGNATURE_OFFSET,
    board: BoardKind::EsPlus,
    keyboard: KeyboardKind::EmulatorMailbox,
    keys: keys::KEYS,
    lcd_width: 96,
    lcd_height: 32,
    vram_base: 0xF800,
    key_mailbox: 0x8E00,
    csr_mask: 0x01,
};

/// Casio fx-991CN X — the same ClassWiz board as the fx-570VN X with a
/// different firmware image (`user202729/fxesplus` → `991cnx/rom.bin`,
/// git blob `850fcc634c31dcff316e99f831a337098c162184`).  Its self-test
/// string is `CY-239` at `0x3FFEE`; the emulator-derived image carries the
/// same string, so use `--model fx991cnx-emu` explicitly for that one.
pub const FX991CNX: Model = Model {
    id: "fx991cnx",
    name: "Casio fx-991CN X (ClassWiz)",
    rom_size: 0x40000,
    signature: b"CY-239",
    signature_offset: SIGNATURE_OFFSET_CLASSWIZ,
    board: BoardKind::ClassWiz,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::VNX_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0x8E00,
    csr_mask: 0x0F,
};

/// Emulator-derived fx-991CN X ROM (`991cnx_emu/rom.bin`, git blob
/// `f1c0caf015790450ad82f0b93f350f68a0617a85`).  The official emulator moves
/// the key mailbox to 0x48E00, outside the 16-bit mailbox address space this
/// build models, so select this descriptor for ROM loading/inspection only.
pub const FX991CNX_EMU: Model = Model {
    id: "fx991cnx-emu",
    name: "Casio fx-991CN X (emulator ROM)",
    rom_size: 0x40000,
    signature: b"",
    signature_offset: SIGNATURE_OFFSET_CLASSWIZ,
    board: BoardKind::ClassWiz,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::VNX_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0x8E00,
    csr_mask: 0x0F,
};

/// Casio fx-991CN CW — ClassWiz II (`hardware_id = 5`), 1 MiB ROM.  The nX-U8
/// core is unchanged, but the board uses 24 KiB of RAM at 0x9000, two 192x64
/// framebuffers, four-bit code segments and a BCD coprocessor.  This build
/// recognises the ROM (self-test `71CASG` at 0x8FDE0) but does not execute it.
pub const FX991CNCW: Model = Model {
    id: "fx991cncw",
    name: "Casio fx-991CN CW",
    rom_size: 0x100000,
    signature: b"71CASG",
    signature_offset: CW_SIGNATURE_OFFSET,
    board: BoardKind::ClassWizII,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::VNX_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0x0000,
    csr_mask: 0x0F,
};

/// Casio fx-580VN X.  Identical hardware/ROM to the fx-570VN X (self-test
/// `CY-298A`), marketed under a different model number.
pub const FX580VNX: Model = Model {
    id: "fx580vnx",
    name: "Casio fx-580VN X",
    rom_size: 0x40000,
    signature: b"CY-298A",
    signature_offset: SIGNATURE_OFFSET_CLASSWIZ,
    board: BoardKind::ClassWiz,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::VNX_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0x8E00,
    csr_mask: 0x0F,
};

/// Casio fx-991EX (ClassWiz EX generation, self-test `CY-235F`).
pub const FX991EX: Model = Model {
    id: "fx991ex",
    name: "Casio fx-991EX",
    rom_size: 0x40000,
    signature: b"CY-235F",
    signature_offset: SIGNATURE_OFFSET_CLASSWIZ,
    board: BoardKind::ClassWiz,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::VNX_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0x8E00,
    csr_mask: 0x0F,
};

/// Casio fx-82EX VerE (ClassWiz EX entry level, self-test `CY-230E`).
pub const FX82EX: Model = Model {
    id: "fx82ex",
    name: "Casio fx-82EX",
    rom_size: 0x40000,
    signature: b"CY-230E",
    signature_offset: SIGNATURE_OFFSET_CLASSWIZ,
    board: BoardKind::ClassWiz,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::VNX_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0x8E00,
    csr_mask: 0x0F,
};

/// Casio fx-991DE X VerF (German ClassWiz, self-test `CY-251F`).
pub const FX991DEX_VERF: Model = Model {
    id: "fx991dex-verf",
    name: "Casio fx-991DE X (VerF)",
    rom_size: 0x40000,
    signature: b"CY-251F",
    signature_offset: SIGNATURE_OFFSET_CLASSWIZ,
    board: BoardKind::ClassWiz,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::VNX_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0x8E00,
    csr_mask: 0x0F,
};

/// Casio fx-991DE X VerE.  This dump has no readable self-test signature, so
/// select it explicitly with `--model fx991dex-vere`.
pub const FX991DEX_VERE: Model = Model {
    id: "fx991dex-vere",
    name: "Casio fx-991DE X (VerE)",
    rom_size: 0x40000,
    signature: b"",
    signature_offset: SIGNATURE_OFFSET_CLASSWIZ,
    board: BoardKind::ClassWiz,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::VNX_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0x8E00,
    csr_mask: 0x0F,
};

/// Casio fx-991CW (ClassWiz II, 512 KiB ROM, self-test `EY-005A`).  Board
/// support is not implemented, so this descriptor is for recognition only.
pub const FX991CW: Model = Model {
    id: "fx991cw",
    name: "Casio fx-991CW",
    rom_size: 0x80000,
    signature: b"EY-005",
    signature_offset: 0x5FFEE,
    board: BoardKind::ClassWizII,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::VNX_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0,
    csr_mask: 0x0F,
};

/// Casio fx-9910CW 2nd edition (ClassWiz II, 512 KiB, self-test `FY-505A`).
pub const FX9910CW: Model = Model {
    id: "fx9910cw",
    name: "Casio fx-9910CW (2nd edition)",
    rom_size: 0x80000,
    signature: b"FY-505",
    signature_offset: 0x5FFEE,
    board: BoardKind::ClassWizII,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::VNX_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0,
    csr_mask: 0x0F,
};

/// Casio fx-880BTG — ClassWiz II (CW ii, hardware id 5), 1 MiB ROM.  The dump
/// in `roms/fx880btg.rom.bin` was verified to contain `EY-023B` with checksum
/// 0x8113 at 0x71FEE.  The nX-U8 core is shared, but the CW ii board (dual
/// framebuffers, BCD coprocessor, 24 KiB RAM at 0x9000) is not emulated yet.
pub const FX880BTG: Model = Model {
    id: "fx880btg",
    name: "Casio fx-880BTG",
    rom_size: 0x100000,
    signature: b"EY-023B",
    signature_offset: CW_VERSION_OFFSET,
    board: BoardKind::ClassWizII,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::VNX_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0,
    csr_mask: 0x0F,
};

/// Every model this build knows about.
pub const ALL: &[Model] = &[
    FX991ES_PLUS_C,
    FX570ES_PLUS,
    FX82ES_PLUS_A,
    FX82ESP_EMU,
    // fx-580VN X must precede fx-570VN X: its `CY-298A` signature also starts
    // with the shorter `CY-298` string.
    FX580VNX,
    FX570VNX,
    FX991EX,
    FX82EX,
    FX991DEX_VERF,
    FX991DEX_VERE,
    FX991CNX,
    FX991CNX_EMU,
    FX991CW,
    FX9910CW,
    FX880BTG,
    FX991CNCW,
];

/// Default model (the one the project was originally written for).
pub fn default() -> Model {
    FX991ES_PLUS_C
}

/// Look up a model by its CLI identifier.
pub fn by_id(id: &str) -> Option<Model> {
    ALL.iter().copied().find(|m| m.id.eq_ignore_ascii_case(id))
}

/// Identify a ROM by its self-test signature.
///
/// Returns `None` for unknown or truncated images so callers can fall back to
/// an explicit `--model` selection.  Each model uses its own signature offset
/// (0x1FFF4 on ES PLUS, 0x3FFEE on ClassWiz).
pub fn detect(rom: &[u8]) -> Option<Model> {
    ALL.iter().copied().find(|m| {
        if m.signature.is_empty() || rom.len() < m.rom_size {
            return false;
        }
        let off = m.signature_offset;
        rom.get(off..off + m.signature.len())
            .is_some_and(|sig| sig.starts_with(m.signature))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rom_with_signature(size: usize, off: usize, sig: &[u8]) -> Vec<u8> {
        let mut rom = vec![0xFFu8; size];
        rom[off..off + sig.len()].copy_from_slice(sig);
        rom
    }

    #[test]
    fn detects_fx570es_plus() {
        assert_eq!(
            detect(&rom_with_signature(0x20000, SIGNATURE_OFFSET, b"GY454XE")).map(|m| m.id),
            Some("fx570es-plus")
        );
    }

    #[test]
    fn detects_fx991es_plus_c() {
        assert_eq!(
            detect(&rom_with_signature(0x20000, SIGNATURE_OFFSET, b"LY727XA")).map(|m| m.id),
            Some("fx991es-plus-c")
        );
    }

    #[test]
    fn detects_fx570vnx() {
        let rom = rom_with_signature(0x40000, SIGNATURE_OFFSET_CLASSWIZ, b"CY-298");
        assert_eq!(detect(&rom).map(|m| m.id), Some("fx570vnx"));
        // A 128 KiB image must not be misdetected as ClassWiz.
        assert!(detect(&rom[..0x20000]).is_none());
    }

    #[test]
    fn unknown_signature_is_none() {
        assert!(detect(&rom_with_signature(
            0x40000,
            SIGNATURE_OFFSET_CLASSWIZ,
            b"UNKNOWN"
        ))
        .is_none());
        assert!(detect(&[0u8; 16]).is_none());
    }

    #[test]
    fn model_lookup_is_case_insensitive() {
        assert_eq!(by_id("FX570ES-PLUS").map(|m| m.id), Some("fx570es-plus"));
        assert_eq!(by_id("Fx570VnX").map(|m| m.id), Some("fx570vnx"));
        assert!(by_id("nope").is_none());
    }

    #[test]
    fn fx570vnx_parameters() {
        let m = FX570VNX;
        assert_eq!(m.rom_size, 0x40000);
        assert_eq!(m.signature_offset, SIGNATURE_OFFSET_CLASSWIZ);
        assert_eq!(m.signature, b"CY-298");
        assert_eq!(m.board, BoardKind::ClassWiz);
        assert_eq!(m.keyboard, KeyboardKind::HardwareSfr);
        assert_eq!(m.lcd_width, 192);
        assert_eq!(m.lcd_height, 64);
        assert_eq!(m.vram_base, 0xF800);
        assert_eq!(m.csr_mask, 0x0F);
        assert!(!m.keys.is_empty());
    }

    #[test]
    fn es_plus_models_share_the_nxu8_board() {
        for m in [FX991ES_PLUS_C, FX570ES_PLUS] {
            assert_eq!(m.board, BoardKind::EsPlus);
            assert_eq!(m.rom_size, 0x20000);
            assert_eq!(m.lcd_width, 96);
            assert_eq!(m.lcd_height, 32);
            assert_eq!(m.vram_base, 0xF800);
            assert_eq!(m.csr_mask, 0x01);
            assert!(!m.keys.is_empty());
        }
    }

    #[test]
    fn detects_added_classwiz_models() {
        let cases: &[(&[u8], &str)] = &[
            (b"CY-298A", "fx580vnx"),
            (b"CY-235F", "fx991ex"),
            (b"CY-230E", "fx82ex"),
            (b"CY-251F", "fx991dex-verf"),
            (b"EY-005A", "fx991cw"),
            (b"FY-505A", "fx9910cw"),
        ];
        for (sig, id) in cases {
            let off = if *id == "fx991cw" || *id == "fx9910cw" {
                0x5FFEE
            } else {
                SIGNATURE_OFFSET_CLASSWIZ
            };
            let mut rom = vec![0xFFu8; 0x80000];
            rom[off..off + sig.len()].copy_from_slice(sig);
            assert_eq!(detect(&rom).map(|m| m.id), Some(*id), "signature {sig:?}");
        }
    }

    #[test]
    fn cw_ii_models_are_executed() {
        for m in [FX991CW, FX9910CW, FX991CNCW, FX880BTG] {
            assert_eq!(m.board, BoardKind::ClassWizII);
            assert!(m.execution_supported());
            assert_eq!(m.lcd_width, 192);
            assert_eq!(m.lcd_height, 64);
            assert_eq!(m.csr_mask, 0x0F);
            assert!(matches!(m.keyboard, KeyboardKind::HardwareSfr));
        }
    }

    #[test]
    fn detects_fx880btg() {
        let mut rom = vec![0xFFu8; 0x100000];
        rom[CW_VERSION_OFFSET..CW_VERSION_OFFSET + 7].copy_from_slice(b"EY-023B");
        assert_eq!(detect(&rom).map(|m| m.id), Some("fx880btg"));
        // Truncated images must not be detected as a 1 MiB model.
        assert!(detect(&rom[..0x80000]).is_none());
    }

    #[test]
    fn every_id_is_unique() {
        let mut ids: Vec<_> = ALL.iter().map(|m| m.id).collect();
        ids.sort_unstable();
        assert!(ids.windows(2).all(|w| w[0] != w[1]));
    }
}
