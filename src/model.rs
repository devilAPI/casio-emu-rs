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

pub const CW_VERSION_OFFSET_ALT: usize = 0x05FFEE;

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

// ---------------------------------------------------------------------------
// 128 KiB ES PLUS / ES PLUS 2 / fx-5800P family (new ROM archive).
//
// `hardware_id` and `real_hardware` below come from each folder's `model.lua`
// where present (`HW_ES_PLUS = 3`, `HW_FX_5800P = 345`); for the folders
// without a `model.lua` they were decoded from the `nx-U8/... config.bin`
// header at 0x72 (csr_mask), 0x74 (chip id) and 0x76 (real_hardware).
// Real-hardware dumps (`real_hardware = 1`) use the KI/KO SFR keyboard;
// emulator dumps (`real_hardware = 0`) use the 0x8E00 mailbox.
//
// Several images share a self-test signature.  `detect()` returns the FIRST
// match in [`ALL`], so the real-hardware descriptor is always registered
// before its emulator twin; select the emulator twin explicitly with
// `--model <id>`.  See docs/MODELS-ESPLUS-FAMILY.md.
// ---------------------------------------------------------------------------

/// Casio fx-115ES PLUS 2, Casio Windows-emulator ROM (`fx115esplus2_emu`).
/// `hardware_id = HW_ES_PLUS (3)`, `real_hardware = 0`, self-test `CY-874A`.
pub const FX115ES_PLUS2_EMU: Model = Model {
    id: "fx115esplus2-emu",
    name: "Casio fx-115ES PLUS 2 (emulator ROM)",
    rom_size: 0x20000,
    signature: b"CY-874",
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

/// Casio fx-570ES, Casio Windows-emulator ROM (`fx570es_emu`).
/// `hardware_id = HW_ES_PLUS (3)`, `real_hardware = 0`.  This image has no
/// self-test string at 0x1FFF4 (the last 256 bytes are zero), so it can only
/// be selected explicitly with `--model fx570es-emu`.
pub const FX570ES_EMU: Model = Model {
    id: "fx570es-emu",
    name: "Casio fx-570ES (emulator ROM)",
    rom_size: 0x20000,
    signature: b"",
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

/// Casio fx-570ES PLUS 2, Casio Windows-emulator ROM (`fx570esplus2_emu`).
/// `hardware_id = HW_ES_PLUS (3)`, `real_hardware = 0`, self-test `CY-854A`.
pub const FX570ES_PLUS2_EMU: Model = Model {
    id: "fx570esplus2-emu",
    name: "Casio fx-570ES PLUS 2 (emulator ROM)",
    rom_size: 0x20000,
    signature: b"CY-854",
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

/// Casio fx-570VN PLUS — real-hardware ES PLUS ROM (`fx570vnplus`).
/// `hardware_id = HW_ES_PLUS (3)`, `real_hardware = 1`, self-test `LY710XA`.
/// This descriptor wins auto-detection for `LY710X`; the emulator dump with
/// the same signature is `FX570VN_PLUS_EMU`.
pub const FX570VN_PLUS: Model = Model {
    id: "fx570vnplus",
    name: "Casio fx-570VN PLUS",
    rom_size: 0x20000,
    signature: b"LY710X",
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

/// Casio fx-570VN PLUS 2 — real-hardware ES PLUS 2 ROM (`fx570vnplus2`).
/// `hardware_id = HW_ES_PLUS (3)`, `real_hardware = 1`, self-test `CY-860A`.
/// This descriptor wins auto-detection for `CY-860`; use
/// `--model fx570vnplus2-emu` for the emulator dump of the same model.
pub const FX570VN_PLUS2: Model = Model {
    id: "fx570vnplus2",
    name: "Casio fx-570VN PLUS 2",
    rom_size: 0x20000,
    signature: b"CY-860",
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

/// Casio fx-570VN PLUS 2, Casio Windows-emulator ROM (`fx570vnplus2_emu`).
/// Self-test `CY-860A`, identical to the real-hardware `FX570VN_PLUS2`, so
/// auto-detection picks the real-hardware descriptor: select this one with
/// `--model fx570vnplus2-emu`.
pub const FX570VN_PLUS2_EMU: Model = Model {
    id: "fx570vnplus2-emu",
    name: "Casio fx-570VN PLUS 2 (emulator ROM)",
    rom_size: 0x20000,
    signature: b"CY-860",
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

/// Casio fx-570VN PLUS, Casio Windows-emulator ROM (`fx570vnplus_emu`).
/// Self-test `LY710XA`, identical to the real-hardware `FX570VN_PLUS`, so
/// auto-detection picks the real-hardware descriptor: select this one with
/// `--model fx570vnplus-emu`.
pub const FX570VN_PLUS_EMU: Model = Model {
    id: "fx570vnplus-emu",
    name: "Casio fx-570VN PLUS (emulator ROM)",
    rom_size: 0x20000,
    signature: b"LY710X",
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

/// Casio fx-5800P (`fx5800p`).  `model.lua` declares
/// `hardware_id = HW_FX_5800P (345)`, `real_hardware = 1`,
/// `csr_mask = 0x000f`, `rom_path = "rom.bin"` and a separate
/// `flash_path = "flash.bin"` (512 KiB).  We load the 128 KiB `rom.bin`, the
/// executable code image; the data flash is not mapped by the ES PLUS board.
/// No self-test string exists at 0x1FFF4 (`Gy344` at 0x1E8B2 / `GY344 Ver1`
/// in flash are version markers, not signatures), so `--model fx5800p` is
/// required.  Its physical keypad differs from the shared 49+ON ES PLUS
/// matrix (codes 0x70/0x67/… drive KO7); `keys::KEYS` is a best-effort
/// fallback, see docs/MODELS-ESPLUS-FAMILY.md.
pub const FX5800P: Model = Model {
    id: "fx5800p",
    name: "Casio fx-5800P",
    rom_size: 0x20000,
    signature: b"",
    signature_offset: SIGNATURE_OFFSET,
    board: BoardKind::EsPlus,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::KEYS,
    lcd_width: 96,
    lcd_height: 32,
    vram_base: 0xF800,
    // Not used: the 5800P dump has no emulator mailbox.
    key_mailbox: 0x0000,
    csr_mask: 0x0F,
};

/// Casio fx-82AU PLUS II, Casio Windows-emulator ROM (`fx82auplusii_emu`).
/// `model.lua` gives `model_name = "CASIO fx-82AU PLUS II (Emulator ROM)"`,
/// `hardware_id = HW_ES_PLUS (3)`, `real_hardware = 0`; self-test `LY711XA`.
pub const FX82AU_PLUS_II_EMU: Model = Model {
    id: "fx82auplusii-emu",
    name: "Casio fx-82AU PLUS II (emulator ROM)",
    rom_size: 0x20000,
    signature: b"LY711X",
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

/// Casio fx-82AU PLUS II, 2nd edition real-hardware dump (standalone
/// `rom.bin`, SHA-256 prefix `479957cc…`).  `hardware_id = HW_ES_PLUS (3)`,
/// `real_hardware = 1`, self-test `CY-863A`.  Same ES PLUS board and 49+ON
/// key matrix as the other real-hardware models in this family.
pub const FX82AU_PLUS_II_2: Model = Model {
    id: "fx82auplusii2",
    name: "Casio fx-82AU PLUS II (2nd edition)",
    rom_size: 0x20000,
    signature: b"CY-863",
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
    keys: keys::CW2_KEYS,
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
    keys: keys::CW2_KEYS,
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
    keys: keys::CW2_KEYS,
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
    keys: keys::CW2_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0,
    csr_mask: 0x0F,
};

/// Every model this build knows about.
const fn classwiz_x(id: &'static str, name: &'static str, signature: &'static [u8]) -> Model {
    Model {
        id,
        name,
        rom_size: 0x40000,
        signature,
        signature_offset: SIGNATURE_OFFSET_CLASSWIZ,
        board: BoardKind::ClassWiz,
        keyboard: KeyboardKind::HardwareSfr,
        keys: keys::VNX_KEYS,
        lcd_width: 192,
        lcd_height: 64,
        vram_base: 0xF800,
        key_mailbox: 0x8E00,
        csr_mask: 0x0F,
    }
}

/// Casio CY-23D — archive folder `cy23d` (SHA-256 229e3825...),
/// `real_hardware = 1`.  The disassembly and the 0x3FFEE signature both
/// confirm the `CY-23D` identity.
pub const CY23D: Model = classwiz_x("cy23d", "CY-23D", b"CY-23D");

/// Casio fx-82CN X, revision A — archive folder `fx82cnx` (SHA-256
/// 89cdfbe7...), `real_hardware = 1`, signature `CY-236A`.
pub const FX82CNX_A: Model = classwiz_x("fx82cnx-a", "Casio fx-82CN X (CY-236A)", b"CY-236A");

/// Casio fx-82CN X, revision B — archive folder `fx-82 CN X` (SHA-256
/// bbf3c5f8...), `real_hardware = 0`, signature `CY-236B`.  Same marketing
/// name as [`FX82CNX_A`] but a distinct firmware image.
pub const FX82CNX_B: Model = classwiz_x("fx82cnx-b", "Casio fx-82CN X (CY-236B)", b"CY-236B");

/// Casio fx-83GT X — archive folder `fx-83 GT X` (SHA-256 be0312bf...),
/// `real_hardware = 0`.
pub const FX83GTX: Model = classwiz_x("fx83gtx", "Casio fx-83GT X", b"CY-213A");

/// Casio fx-JP700 — archive folders `fxjp700` and `fx-JP700` are the same
/// image (SHA-256 2d6bf786...), `real_hardware = 1`.
pub const FXJP700: Model = classwiz_x("fxjp700", "Casio fx-JP700", b"CY-242A");

/// Casio fx-JP900 — archive folders `fxjp900` and `fx-JP900` are the same
/// image (SHA-256 f8990442...), `real_hardware = 1`.
pub const FXJP900: Model = classwiz_x("fxjp900", "Casio fx-JP900", b"CY-243F");

/// Casio fx-530AZ — archive folder `fx530az` (SHA-256 35ee817a...),
/// `real_hardware = 1`.
pub const FX530AZ: Model = classwiz_x("fx530az", "Casio fx-530AZ", b"CY-240A");

/// Casio fx-82AR X — archive folder `fx82arx` (SHA-256 f921562f...),
/// `real_hardware = 1`.
pub const FX82ARX: Model = classwiz_x("fx82arx", "Casio fx-82AR X", b"CY-256A");

/// Casio fx-82CE X — archive folder `fx82cex` (SHA-256 b39443de...),
/// `real_hardware = 1`.
pub const FX82CEX: Model = classwiz_x("fx82cex", "Casio fx-82CE X", b"CY-291E");

/// Casio fx-85GT X — archive folder `fx85gtx` (SHA-256 c1bf1bdb...),
/// `real_hardware = 1`.
pub const FX85GTX: Model = classwiz_x("fx85gtx", "Casio fx-85GT X", b"CY-214A");

/// Casio fx-92+ Spéciale Collège — archive folder `fx92p` (SHA-256
/// 095a9b26...), `real_hardware = 1`.
pub const FX92P: Model = classwiz_x("fx92p", "Casio fx-92+ Spéciale Collège", b"CY-295A");

/// Casio fx-92 Spéciale Collège — archive folder `fx92sc` (SHA-256
/// d9cabc97...), `real_hardware = 1`.
pub const FX92SC: Model = classwiz_x("fx92sc", "Casio fx-92 Spéciale Collège", b"CY-246A");

/// Casio fx-97SG X — archive folder `fx97sgx` (SHA-256 c25e9739...),
/// `real_hardware = 1`.
pub const FX97SGX: Model = classwiz_x("fx97sgx", "Casio fx-97SG X", b"CY-297E");

/// Casio fx-991CE X — archive folder `fx991cex` (SHA-256 d277fcf4...),
/// `real_hardware = 1`.
pub const FX991CEX: Model = classwiz_x("fx991cex", "Casio fx-991CE X", b"CY-294F");

/// Casio fx-991DEX, revision A — archive folder `fx991dex` (SHA-256
/// 0d32e349...), `real_hardware = 1`, signature `CY-251A`.  Distinct from the
/// already-supported `fx991dex-verf` (`CY-251F`).
pub const FX991DEX: Model = classwiz_x("fx991dex", "Casio fx-991DEX (CY-251A)", b"CY-251A");

/// Casio fx-580VN X "font CWII" firmware — archive folder
/// `fx-580VN X (font CWII)` (SHA-256 f3259b16...).  Signature detection is
/// impossible: it shares `CY-298A` with [`FX580VNX`], which wins by order, so
/// this descriptor has an empty signature and must be selected explicitly.
pub const FX580VNX_CWII: Model = classwiz_x("fx580vnx-cwii", "Casio fx-580VN X (font CWII)", b"");

/// Every model this build knows about.
pub const FX810DECW: Model = Model {
    id: "fx810decw",
    name: "Casio fx-810 DE CW",
    rom_size: 0x80000,
    signature: b"EY-047A",
    signature_offset: CW_VERSION_OFFSET,
    board: BoardKind::ClassWizII,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::CW2_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0,
    csr_mask: 0x0F,
};

/// Casio fx-8200AU — 512 KiB ClassWiz II, `EY-044A` mirrored at 0x5FFEE and
/// 0x71FEE.  `model.lua`: hardware_id 5, real_hardware 1, csr_mask 0x000F.
pub const FX8200AU: Model = Model {
    id: "fx8200au",
    name: "Casio fx-8200AU",
    rom_size: 0x80000,
    signature: b"EY-044A",
    signature_offset: CW_VERSION_OFFSET,
    board: BoardKind::ClassWizII,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::CW2_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0,
    csr_mask: 0x0F,
};

/// Casio fx-82DE CW — 512 KiB ClassWiz II, `EY-012A` mirrored at 0x5FFEE and
/// 0x71FEE.  `model.lua`: hardware_id 5, real_hardware 1, csr_mask 0x000F.
pub const FX82DECW: Model = Model {
    id: "fx82decw",
    name: "Casio fx-82DE CW",
    rom_size: 0x80000,
    signature: b"EY-012A",
    signature_offset: CW_VERSION_OFFSET,
    board: BoardKind::ClassWizII,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::CW2_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0,
    csr_mask: 0x0F,
};

/// Casio fx-92B Secondaire — 1 MiB ClassWiz II, `EY-007A` mirrored at 0x5FFEE
/// and 0x71FEE.  `model.lua`: hardware_id 5, real_hardware 1.
pub const FX92BS: Model = Model {
    id: "fx92bs",
    name: "Casio fx-92B Secondaire",
    rom_size: 0x100000,
    signature: b"EY-007A",
    signature_offset: CW_VERSION_OFFSET,
    board: BoardKind::ClassWizII,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::CW2_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0,
    csr_mask: 0x0F,
};

/// Casio fx-999CN CW — 1 MiB ClassWiz II, `EY-028A` mirrored at 0x5FFEE and
/// 0x71FEE.  `model.lua`: hardware_id 5, real_hardware 1.
pub const FX999CNCW: Model = Model {
    id: "fx999cncw",
    name: "Casio fx-999CN CW",
    rom_size: 0x100000,
    signature: b"EY-028A",
    signature_offset: CW_VERSION_OFFSET,
    board: BoardKind::ClassWizII,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::CW2_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0,
    csr_mask: 0x0F,
};

/// Casio fx-JP700 CW — 512 KiB ClassWiz II, `EY-030A` mirrored at 0x5FFEE and
/// 0x71FEE.  `romjp7cw.bin` in the source folder is byte-identical to
/// `rom.bin`, so only one dump is registered/copied.
pub const FXJP700CW: Model = Model {
    id: "fxjp700cw",
    name: "Casio fx-JP700 CW",
    rom_size: 0x80000,
    signature: b"EY-030A",
    signature_offset: CW_VERSION_OFFSET,
    board: BoardKind::ClassWizII,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::CW2_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0,
    csr_mask: 0x0F,
};

/// Casio Graph Light — 512 KiB ClassWiz II emulator dump, `EY-090A` at 0x71FEE.
/// Folder `graphlight_emu` has no `model.lua`; the name comes from the folder
/// and the `GRAPHLIGHT` product (ClassWiz-family French graphing calculator).
pub const GRAPHLIGHT_EMU: Model = Model {
    id: "graphlight-emu",
    name: "Casio Graph Light (emulator ROM)",
    rom_size: 0x80000,
    signature: b"EY-090A",
    signature_offset: CW_VERSION_OFFSET,
    board: BoardKind::ClassWizII,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::CW2_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0,
    csr_mask: 0x0F,
};

/// Casio fx-991CW — 1 MiB emulator dump from folder `fx991cw_emu`, `EY-005A`
/// at 0x71FEE (0x5FFEE is erased, unlike the 512 KiB `fx991cw` dump whose
/// `EY-005` at 0x5FFEE is detected first).  Registered after `FX991CW` so the
/// existing model keeps auto-detection.
pub const FX991CW_EMU: Model = Model {
    id: "fx991cw-emu",
    name: "Casio fx-991CW (emulator ROM)",
    rom_size: 0x100000,
    signature: b"EY-005A",
    signature_offset: CW_VERSION_OFFSET,
    board: BoardKind::ClassWizII,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::CW2_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0,
    csr_mask: 0x0F,
};

/// Casio fx-991CN CW — archive dump from folder `fx991cncw` (SHA-256
/// `55ec7507…80aaf`, **not** byte-identical to the already registered
/// `fx991cncw` dump).  It carries `EY-027A` at 0x5FFEE (erased on the older
/// image), so use that offset for unambiguous auto-detection.  The older
/// descriptor keeps its `71CASG` signature at 0x8FDE0.
pub const FX991CNCW_ALT: Model = Model {
    id: "fx991cncw-alt",
    name: "Casio fx-991CN CW (archive dump)",
    rom_size: 0x100000,
    signature: b"EY-027A",
    signature_offset: CW_VERSION_OFFSET_ALT,
    board: BoardKind::ClassWizII,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::CW2_KEYS,
    lcd_width: 192,
    lcd_height: 64,
    vram_base: 0xF800,
    key_mailbox: 0,
    csr_mask: 0x0F,
};

/// Casio fx-880BTG — real-hardware archive dump from folder `fx880btg`
/// (SHA-256 `2fa677a4…ac7c7`, distinct from the existing `fx880btg.rom.bin`).
/// **Signature collision:** both dumps contain the identical `EY-023B` record
/// at 0x71FEE, so `detect()` always returns the first-registered `fx880btg`.
/// Select this descriptor explicitly with `--model fx880btg-alt` to load the
/// archive dump.  `model.lua` reports hardware_id 5, real_hardware 1.
pub const FX880BTG_ALT: Model = Model {
    id: "fx880btg-alt",
    name: "Casio fx-880BTG (archive dump)",
    rom_size: 0x100000,
    signature: b"EY-023B",
    signature_offset: CW_VERSION_OFFSET,
    board: BoardKind::ClassWizII,
    keyboard: KeyboardKind::HardwareSfr,
    keys: keys::CW2_KEYS,
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
    FX570VN_PLUS,
    FX570VN_PLUS_EMU,
    FX570VN_PLUS2,
    FX570VN_PLUS2_EMU,
    FX115ES_PLUS2_EMU,
    FX570ES_PLUS2_EMU,
    FX570ES_EMU,
    FX5800P,
    FX82AU_PLUS_II_EMU,
    FX82AU_PLUS_II_2,
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
    CY23D,
    FX82CNX_A,
    FX82CNX_B,
    FX83GTX,
    FXJP700,
    FXJP900,
    FX530AZ,
    FX82ARX,
    FX82CEX,
    FX85GTX,
    FX92P,
    FX92SC,
    FX97SGX,
    FX991CEX,
    FX991DEX,
    FX580VNX_CWII,
    FX991CNCW_ALT,
    FX991CW_EMU,
    FX810DECW,
    FX8200AU,
    FX82DECW,
    FX92BS,
    FX999CNCW,
    FXJP700CW,
    GRAPHLIGHT_EMU,
    FX880BTG_ALT,
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

    /// Build a 128 KiB ROM with the given self-test string at 0x1FFF4.
    fn esplus_rom_with_signature(sig: &[u8]) -> Vec<u8> {
        rom_with_signature(0x20000, SIGNATURE_OFFSET, sig)
    }

    #[test]
    fn detects_new_esplus_family_signatures() {
        // Full seven-byte signatures as they appear in the ROMs; the
        // descriptors store the six-byte prefix used by detect().
        let cases: &[(&[u8], &str)] = &[
            (b"CY-874A", "fx115esplus2-emu"),
            (b"CY-854A", "fx570esplus2-emu"),
            (b"LY710XA", "fx570vnplus"),
            (b"CY-860A", "fx570vnplus2"),
            (b"LY711XA", "fx82auplusii-emu"),
            (b"CY-863A", "fx82auplusii2"),
        ];
        for (sig, id) in cases {
            let rom = esplus_rom_with_signature(sig);
            assert_eq!(detect(&rom).map(|m| m.id), Some(*id), "signature {sig:?}");
        }
        // Pre-existing signatures still resolve to the original descriptors.
        assert_eq!(
            detect(&esplus_rom_with_signature(b"GY454XE")).map(|m| m.id),
            Some("fx570es-plus")
        );
        assert_eq!(
            detect(&esplus_rom_with_signature(b"LY713XA")).map(|m| m.id),
            Some("fx82esplusa")
        );
    }

    #[test]
    fn ambiguous_esplus_signatures_prefer_real_hardware() {
        // Two dumps share LY710XA and two share CY-860A.  The real-hardware
        // descriptor is registered first so its working KI/KO keyboard wins;
        // the emulator twin is still selectable with --model.
        for (sig, expected, emulator) in [
            (b"LY710XA" as &[u8], "fx570vnplus", "fx570vnplus-emu"),
            (b"CY-860A" as &[u8], "fx570vnplus2", "fx570vnplus2-emu"),
        ] {
            let rom = esplus_rom_with_signature(sig);
            assert_eq!(detect(&rom).map(|m| m.id), Some(expected));
            let twin = by_id(emulator).expect("emulator twin must be registered");
            assert_eq!(twin.signature, expected_signature_prefix(expected));
            assert_eq!(twin.keyboard, KeyboardKind::EmulatorMailbox);
            assert!(twin.id != expected);
        }
    }

    fn expected_signature_prefix(id: &str) -> &'static [u8] {
        match id {
            "fx570vnplus" => b"LY710X",
            "fx570vnplus2" => b"CY-860",
            _ => unreachable!(),
        }
    }

    #[test]
    fn signatureless_family_models_require_explicit_selection() {
        for id in ["fx570es-emu", "fx5800p"] {
            let m = by_id(id).expect("model must be registered");
            assert!(m.signature.is_empty());
            // An all-zero 128 KiB image (as in both ROMs at 0x1FFF4) is not
            // auto-detected; the CLI must fall back to an explicit --model.
            assert!(detect(&[0u8; 0x20000]).is_none());
        }
    }

    #[test]
    fn new_esplus_family_rom_sizes_and_board() {
        let models = [
            FX115ES_PLUS2_EMU,
            FX570ES_EMU,
            FX570ES_PLUS2_EMU,
            FX570VN_PLUS,
            FX570VN_PLUS2,
            FX570VN_PLUS2_EMU,
            FX570VN_PLUS_EMU,
            FX5800P,
            FX82AU_PLUS_II_EMU,
            FX82AU_PLUS_II_2,
        ];
        for m in models {
            assert_eq!(m.rom_size, 0x20000, "{}", m.id);
            assert_eq!(m.signature_offset, SIGNATURE_OFFSET, "{}", m.id);
            assert_eq!(m.board, BoardKind::EsPlus, "{}", m.id);
            assert_eq!(m.lcd_width, 96, "{}", m.id);
            assert_eq!(m.lcd_height, 32, "{}", m.id);
            assert_eq!(m.vram_base, 0xF800, "{}", m.id);
            assert!(m.execution_supported(), "{}", m.id);
        }
        assert_eq!(FX5800P.csr_mask, 0x0F);
        for m in models.into_iter().filter(|m| m.id != "fx5800p") {
            assert_eq!(m.csr_mask, 0x01, "{}", m.id);
        }
    }

    #[test]
    fn new_esplus_family_keyboard_kinds() {
        for m in [FX570VN_PLUS, FX570VN_PLUS2, FX82AU_PLUS_II_2, FX5800P] {
            assert_eq!(m.keyboard, KeyboardKind::HardwareSfr, "{}", m.id);
        }
        for m in [
            FX115ES_PLUS2_EMU,
            FX570ES_EMU,
            FX570ES_PLUS2_EMU,
            FX570VN_PLUS_EMU,
            FX570VN_PLUS2_EMU,
            FX82AU_PLUS_II_EMU,
        ] {
            assert_eq!(m.keyboard, KeyboardKind::EmulatorMailbox, "{}", m.id);
            assert_eq!(m.key_mailbox, 0x8E00, "{}", m.id);
        }
    }

    #[test]
    fn new_esplus_family_reuses_the_shared_key_matrix() {
        // All new descriptors point at the same 49-position + ON ES PLUS
        // matrix, so PC key bindings and GUI hitboxes keep working.
        for m in [
            FX115ES_PLUS2_EMU,
            FX570ES_EMU,
            FX570ES_PLUS2_EMU,
            FX570VN_PLUS,
            FX570VN_PLUS2,
            FX570VN_PLUS2_EMU,
            FX570VN_PLUS_EMU,
            FX5800P,
            FX82AU_PLUS_II_EMU,
            FX82AU_PLUS_II_2,
        ] {
            assert_eq!(m.keys.len(), keys::KEYS.len(), "{}", m.id);
            assert_eq!(m.keys[0].label, keys::KEYS[0].label, "{}", m.id);
            assert_eq!(
                (m.keys[0].ki, m.keys[0].ko),
                (keys::KEYS[0].ki, keys::KEYS[0].ko),
                "{}",
                m.id
            );
            let last = m.keys.len() - 1;
            assert_eq!(m.keys[last].label, keys::KEYS[last].label, "{}", m.id);
            // The ON key is the 0xFF code (ki = 0, ko = 0) in the reference
            // model.lua files; the shared table models it as a matrix entry.
            assert!(m.keys.iter().any(|k| k.ki == 0x00 && k.ko == 0x00));
        }
    }

    #[test]
    fn new_esplus_family_ids_are_lookupable() {
        for id in [
            "fx115esplus2-emu",
            "fx570es-emu",
            "fx570esplus2-emu",
            "fx570vnplus",
            "fx570vnplus2",
            "fx570vnplus2-emu",
            "fx570vnplus-emu",
            "fx5800p",
            "fx82auplusii-emu",
            "fx82auplusii2",
        ] {
            assert_eq!(by_id(id).map(|m| m.id), Some(id));
            assert_eq!(m_rom_filename(id), format!("{id}.rom.bin"));
        }
    }

    fn m_rom_filename(id: &str) -> String {
        by_id(id).unwrap().rom_filename()
    }

    /// All distinct 256 KiB ClassWiz X dumps from the archive, with the exact
    /// seven-byte (CY-23D: six-byte) self-test string found at 0x3FFEE.
    const CWX_CASES: &[(&Model, &[u8])] = &[
        (&CY23D, b"CY-23D"),
        (&FX82CNX_A, b"CY-236A"),
        (&FX82CNX_B, b"CY-236B"),
        (&FX83GTX, b"CY-213A"),
        (&FXJP700, b"CY-242A"),
        (&FXJP900, b"CY-243F"),
        (&FX530AZ, b"CY-240A"),
        (&FX82ARX, b"CY-256A"),
        (&FX82CEX, b"CY-291E"),
        (&FX85GTX, b"CY-214A"),
        (&FX92P, b"CY-295A"),
        (&FX92SC, b"CY-246A"),
        (&FX97SGX, b"CY-297E"),
        (&FX991CEX, b"CY-294F"),
        (&FX991DEX, b"CY-251A"),
    ];

    #[test]
    fn detects_classwiz_x_family_by_signature() {
        for (model, sig) in CWX_CASES {
            let rom = rom_with_signature(model.rom_size, model.signature_offset, sig);
            assert_eq!(detect(&rom).map(|m| m.id), Some(model.id), "sig {sig:?}");
        }
    }

    #[test]
    fn classwiz_x_family_uses_the_shared_classwiz_board() {
        for (model, _) in CWX_CASES
            .iter()
            .copied()
            .chain([(&FX580VNX_CWII, b"" as &[u8])])
        {
            assert_eq!(model.rom_size, 0x40000, "id {}", model.id);
            assert_eq!(model.signature_offset, SIGNATURE_OFFSET_CLASSWIZ);
            assert_eq!(model.board, BoardKind::ClassWiz);
            assert_eq!(model.keyboard, KeyboardKind::HardwareSfr);
            assert_eq!(model.lcd_width, 192);
            assert_eq!(model.lcd_height, 64);
            assert_eq!(model.vram_base, 0xF800);
            assert_eq!(model.csr_mask, 0x0F);
            assert!(!model.keys.is_empty());
            assert!(model.execution_supported());
        }
    }

    #[test]
    fn classwiz_x_roms_are_not_detected_when_truncated() {
        for (model, sig) in CWX_CASES {
            let mut rom = rom_with_signature(model.rom_size, model.signature_offset, sig);
            rom.truncate(0x20000);
            assert!(
                detect(&rom).is_none(),
                "truncated image must not match {}",
                model.id
            );
        }
    }

    #[test]
    fn fx82cnx_revisions_are_distinguished() {
        for (sig, id) in [
            (b"CY-236A" as &[u8], "fx82cnx-a"),
            (b"CY-236B", "fx82cnx-b"),
        ] {
            let rom = rom_with_signature(0x40000, SIGNATURE_OFFSET_CLASSWIZ, sig);
            assert_eq!(detect(&rom).map(|m| m.id), Some(id));
        }
    }

    #[test]
    fn new_fx991dex_does_not_shadow_existing_verf() {
        for (sig, id) in [
            (b"CY-251A" as &[u8], "fx991dex"),
            (b"CY-251F", "fx991dex-verf"),
        ] {
            let rom = rom_with_signature(0x40000, SIGNATURE_OFFSET_CLASSWIZ, sig);
            assert_eq!(detect(&rom).map(|m| m.id), Some(id));
        }
    }

    #[test]
    fn font_cwii_variant_is_explicit_selection_only() {
        // Its signature collides with the earlier, already-registered
        // fx580vnx descriptor, so auto-detection must pick fx580vnx.
        let rom = rom_with_signature(0x40000, SIGNATURE_OFFSET_CLASSWIZ, b"CY-298A");
        assert_eq!(detect(&rom).map(|m| m.id), Some("fx580vnx"));
        assert!(FX580VNX_CWII.signature.is_empty());
        assert_eq!(by_id("fx580vnx-cwii").map(|m| m.id), Some("fx580vnx-cwii"));
    }

    #[test]
    fn classwiz_x_ids_are_lookupable() {
        for (model, _) in CWX_CASES {
            assert_eq!(by_id(model.id).map(|m| m.id), Some(model.id));
            assert_eq!(model.rom_filename(), format!("{}.rom.bin", model.id));
        }
    }

    #[test]
    fn detects_cw2_archive_family() {
        // (signature, ROM size, signature offset, id) — no ROM files needed.
        let cases: &[(&[u8], usize, usize, &str)] = &[
            (b"EY-047A", 0x80000, CW_VERSION_OFFSET, "fx810decw"),
            (b"EY-044A", 0x80000, CW_VERSION_OFFSET, "fx8200au"),
            (b"EY-012A", 0x80000, CW_VERSION_OFFSET, "fx82decw"),
            (b"EY-007A", 0x100000, CW_VERSION_OFFSET, "fx92bs"),
            (b"EY-028A", 0x100000, CW_VERSION_OFFSET, "fx999cncw"),
            (b"EY-030A", 0x80000, CW_VERSION_OFFSET, "fxjp700cw"),
            (b"EY-090A", 0x80000, CW_VERSION_OFFSET, "graphlight-emu"),
            (b"EY-005A", 0x100000, CW_VERSION_OFFSET, "fx991cw-emu"),
            (b"EY-027A", 0x100000, CW_VERSION_OFFSET_ALT, "fx991cncw-alt"),
        ];
        for (sig, size, off, id) in cases {
            let rom = rom_with_signature(*size, *off, sig);
            assert_eq!(detect(&rom).map(|m| m.id), Some(*id), "id {id}");
            // A dump shorter than the declared ROM size must not be detected.
            assert!(detect(&rom[..size - 1]).is_none(), "truncated {id}");
            assert_eq!(by_id(id).map(|m| m.rom_size), Some(*size));
        }
    }

    #[test]
    fn cw2_archive_descriptors_match_the_cw2_board() {
        for id in [
            "fx810decw",
            "fx8200au",
            "fx82decw",
            "fx880btg-alt",
            "fx92bs",
            "fx991cncw-alt",
            "fx991cw-emu",
            "fx999cncw",
            "fxjp700cw",
            "graphlight-emu",
        ] {
            let m = by_id(id).unwrap_or_else(|| panic!("missing model {id}"));
            assert_eq!(m.board, BoardKind::ClassWizII, "model {id}");
            assert_eq!(m.keyboard, KeyboardKind::HardwareSfr, "model {id}");
            assert_eq!(m.lcd_width, 192, "model {id}");
            assert_eq!(m.lcd_height, 64, "model {id}");
            assert_eq!(m.vram_base, 0xF800, "model {id}");
            assert_eq!(m.csr_mask, 0x0F, "model {id}");
            assert!(!m.keys.is_empty(), "model {id}");
            assert!(m.execution_supported(), "model {id}");
            assert_eq!(m.rom_filename(), format!("{id}.rom.bin"));
            assert!(
                m.signature_offset == CW_VERSION_OFFSET
                    || m.signature_offset == CW_VERSION_OFFSET_ALT,
                "model {id}"
            );
        }
    }

    #[test]
    fn cw2_duplicate_dumps_need_explicit_model() {
        // Both fx-880BTG dumps carry the same EY-023B record; auto-detection
        // deliberately keeps resolving to the originally registered model.
        let mut rom = vec![0xFFu8; 0x100000];
        rom[CW_VERSION_OFFSET..CW_VERSION_OFFSET + 7].copy_from_slice(b"EY-023B");
        assert_eq!(detect(&rom).map(|m| m.id), Some("fx880btg"));
        let alt = by_id("fx880btg-alt").unwrap();
        assert_eq!(alt.signature, b"EY-023B");
        assert_eq!(alt.signature_offset, CW_VERSION_OFFSET);
        assert_eq!(alt.rom_size, 0x100000);

        // The older fx-991CN CW dump has 0x5FFEE erased and uses 71CASG;
        // the archive dump has EY-027A at 0x5FFEE and is detected first.
        let mut older = vec![0xFFu8; 0x100000];
        older[0x8FDE0..0x8FDE6].copy_from_slice(b"71CASG");
        assert_eq!(detect(&older).map(|m| m.id), Some("fx991cncw"));
        let alt = by_id("fx991cncw-alt").unwrap();
        assert_eq!(alt.signature_offset, CW_VERSION_OFFSET_ALT);
        assert_eq!(alt.signature, b"EY-027A");
    }
}
