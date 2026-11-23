# Casio fx-570VN X (ClassWiz) model notes

This document records the parameters used by the `fx570vnx` model descriptor
and the limits of the current implementation.  It is derived from the
`HW_CLASSWIZ` emulation in **user202729/CasioEmu** (stable branch) and
**Xyzstk/CasioEmuX** (stable branch), not from Casio documentation.

## ROM provenance

| Image | URL | Git blob | SHA-256 |
|---|---|---|---|
| Real hardware (supported by `fx570vnx`) | <https://raw.githubusercontent.com/user202729/fxesplus/master/580vnx/rom.bin> | `3105c5e44b6795fe57c36ab9eef18e020fea4699` | `7a164c650d6e6bc89e220caf8ac8349724adc2021e59649d428adae6b509e267` |
| Casio Windows-emulator variant (not supported) | <https://raw.githubusercontent.com/user202729/fxesplus/master/580vnx_emu/rom.bin> | `f956589e7ce744ca2fd5a3d588f66f01cf25818a` | not recorded here |

The ROM is Casio copyrighted.  It is **not** stored in this repository and
must never be committed.  The URLs/hashes above are for identification only.

The published image is labelled `fx-580VN X`; the fx-570VN X and fx-580VN X
use the same ClassWiz hardware and firmware image (both configure the
reference emulator with `hardware_id = 4`, `real_hardware = 1`,
`pd_value = 0`).  The model ID `fx570vnx` is therefore the name of our
descriptor, not a claim that a distinct ROM was analysed.

## Exact parameters

| Parameter | Value | Source |
|---|---|---|
| `id` | `fx570vnx` | CLI |
| `name` | `Casio fx-570VN X (ClassWiz)` | CLI |
| `rom_size` | `0x40000` (262,144 bytes / 256 KiB) | ROM blob size |
| `signature` | `CY-298` (6 ASCII bytes) | bytes at `0x3FFEE` in both ROMs |
| `signature_offset` | `0x3FFEE` | ROM inspection (ES PLUS uses `0x1FFF4`) |
| `board` | `ClassWiz` (`HW_CLASSWIZ`, hardware id 4) | `model.lua` |
| `keyboard` | `HardwareSfr` (`real_hardware = 1`) | `model.lua` |
| `keys` | `keys::VNX_KEYS` (59 buttons) | `model.lua` button map |
| `lcd_width` | 192 pixels (24 displayed bytes x 8) | `Screen.cpp`: `ROW_SIZE_DISP = 24` |
| `lcd_height` | 64 rows (63 dot rows + 1 status/icon row) | `Screen.cpp`: `N_ROW = 63`, plus one status row |
| `vram_base` | `0xF800` | `Screen.cpp`: `region_buffer.Setup(0xF800, ...)` |
| VRAM row stride | 32 bytes, of which 24 are displayed | `Screen.cpp`: `ROW_SIZE = 32`, `OFFSET = 32`, `ROW_SIZE_DISP = 24` |
| VRAM size | `64 * 32 = 0x800` bytes (`0xF800..0xFFFF`) | derived |
| `key_mailbox` | unused on real hardware; set to `0x8E00` for uniformity | `Keyboard.cpp` (`!real_hardware` branch uses `0x48E00`) |
| `csr_mask` | `0x0F` | `model.lua` and `CPU::Fetch()` masking |

Changes to the shared `Model` descriptor needed for this model:
`signature_offset: usize` (per-model self-test signature location) and
`board: BoardKind` (selects memory map / VRAM geometry).  All other fields
keep their existing meaning.

## Memory map

Code segments (CSR is masked with `csr_mask = 0x0F`):

| CSR | Address window | ROM offset |
|---|---|---|
| 0 | `0x00000..0x0CFFF` | `0x00000` (rest of page 0 unmapped) |
| 1 | `0x10000..0x1FFFF` | `0x10000` |
| 2 | `0x20000..0x2FFFF` | `0x20000` |
| 3 | `0x30000..0x3FFFF` | `0x30000` |
| 4 | `0x40000..0x4FFFF` | unmapped (reads `0xFF`) |
| 5 | `0x50000..0x5FFFF` | `0x00000` (mirror of page 0) |
| 6..15 | — | unmapped |

Source: `ROMWindow.cpp`, `case HW_CLASSWIZ`.  `Chipset.cpp` generates segment
dispatch for segments 0..5, i.e. four code pages plus the page-0 mirror and
one unpopulated segment.

Data memory (DSR is *not* masked; segment 8 matters on the older board):

| DSR | Address window | Content |
|---|---|---|
| 0 | `0x00000..0x0CFFF` | ROM page 0 (read-only) |
| 0 | `0x0D000..0x0EFFF` | 8 KiB battery-backed RAM |
| 0 | `0x0F000..0x0FFFF` | SFR page (keyboard, screen, timer, VRAM) |
| 1/2/3 | `0x10000..0x3FFFF` | ROM pages 1..3 |
| 5 | `0x50000..0x5FFFF` | ROM page 0 mirror |
| others | — | unmapped (reads `0x00`) |

Source: `BatteryBackedRAM.cpp` (`HW_CLASSWIZ`: base `0xD000`, size `0x2000`).

## Keyboard / SFRs

The keyboard SFR addresses are unchanged from the ES PLUS board:

| Address | Meaning |
|---|---|
| `0xF040` | KI, active low (8 bits) |
| `0xF042` | input filter |
| `0xF044..0xF045` | KO mask, 10 bits (`& 0x03FF`) |
| `0xF046..0xF047` | KO drive, 10 bits (`& 0x03FF`) |

`model.lua` encodes each button as `code = (KO_index << 4) | KI_index`;
this emulator stores active-high bit masks, so
`ki = 1 << (code & 0xF)` and `ko = 1 << (code >> 4)`.  The matrix codes for
the ClassWiz and the fx-570ES PLUS are the same (e.g. `7` = `0x02`,
`0` = `0x64`, `EXE` = `0x60`, `DEL` = `0x32`); only the physical button
rectangles differ, which is why `VNX_KEYS` carries its own grid (41x52).

The real-hardware ROM has no 0x8E00 mailbox.  The Windows-emulator variant
maps a mailbox at `0x40000 + 0x8E00 = 0x48E00` plus a Pd value at `0xF050`;
that variant is explicitly out of scope.

Other SFRs from the reference emulator that this core does not implement:

- interrupt mask/pending at `0xF010`/`0xF014` (16-bit),
- standby control at `0xF008`/`0xF009`,
- timer at `0xF020..0xF025` with interrupt source 9,
- miscellaneous model-ID/unknown registers (`0xF000`, `0xF048..0xF04F`,
  `0xF220..0xF223`; 13 unknown bytes on ClassWiz vs 5 on ES PLUS),
- screen mode/range/contrast at `0xF030..0xF032` (status display modes 4/5/6).

## Validation limits

- All committed tests are **ROM-independent**: they use synthetic `0x00`/`0xFF`
  images and check model lookup, signature detection, memory-map boundaries,
  CSR masking, KI/KO gating and VRAM rendering.  No copyrighted bytes are
  embedded in tests or source.
- A local smoke test with the real hardware image (fetched only to `/tmp`,
  never added to the repo) confirms signature auto-detection and that the
  CPU executes at least 200,000 instructions without a panic
  (`pc=a1d0 csr=00 psw=8e` after 200k steps).  **The LCD stayed blank** in
  that smoke test: timers, interrupts and screen mode handling are not
  implemented, so boot cannot reach a validated interactive state.
- VRAM rendering exposes the raw status row as 192 monochrome pixels.  The
  real status bar consists of alpha-blended icon sprites from the device
  image (20 sprites: S, A, M, STO, MATH, D, R, G, FIX, SCI, E, CMPLX, ANGLE,
  WDOWN, arrows, PAUSE, SUN), so it is **not** pixel-accurate.
- The ClassWiz "larger font" is drawn by firmware glyphs in ROM; this
  emulator only supplies the dot-matrix framebuffer.  A ClassWiz device skin
  is not bundled.  If an ES PLUS skin is supplied with `--model fx570vnx`,
  its LCD/keypad rectangles are still the ES PLUS ones.
- Keyboard ghosting (multiple simultaneous keys) and the KO mask/resistor
  model-ID behaviour are simplified: only one held key is tracked and the
  KI line is gated by `ko_drive & ~ko_mask`.
- The emulator-variant ROM (`f956589e...`, mailbox at `0x48E00`) is not
  emulated; only the real-hardware image is supported.
- There is no cycle-accurate checkpoint for this model.  Boot/key/draw
  checkpoints exist only for the fx-991ES PLUS C and are unaffected by this
  change.
