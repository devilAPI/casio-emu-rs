# Casio fx-991CN CW VerA — model notes

Status in this build: **recognised model, ROM loads, firmware execution NOT supported.**

## 1. CPU / chip family (the critical question)

**The fx-991CN CW uses the same nX-U8 CPU core (the one this emulator decodes,
commonly referred to as nX-U8/100), not a different CPU.** The *board* around
the core is different: it is the "ClassWiz II" (CW ii) generation
(`hardware_id = 5`), whereas the original fx-991ES PLUS C uses the ES PLUS board
(`hardware_id = 3`).

Evidence:

1. Physics365's decompilation repository states plainly:
   `本仓库所关注的计算器均使用 nX-U8 CPU` — "all calculators covered by this
   repository use the nX-U8 CPU" — and ships a Ghidra nX-U8 language module plus
   an nX-U8 Core Instruction Manual. Its real-ROM collection explicitly contains
   `fx-991 CN CW (Ver A)`.
   <https://github.com/Physics365/991CN-X-CW-Decompilation>
2. CasioEmuX / CasioEmuNeo run the CW models on the *same* CPU implementation
   (`Chipset/CPU.cpp/.hpp`): identical register file (R0–R15, CR0–CR15, PC,
   CSR/ECSR, PSW/EPSW, SP, EA, DSR), identical PSW bit layout, identical
   opcode dispatch (`OP_DAA`, `OP_DAS`, `OP_DSR`, `OP_SWI`, `OP_MUL`, …), and
   references to the same "nX-U8 manual" sections as the ES PLUS code. The
   disassembler used for both `fx991cnx` and `fx991cncw` is `u8-disas`.
   <https://github.com/Xyzstk/CasioEmuX> ·
   <https://github.com/qiufuyu123/CasioEmuNeo>
3. `models/fx991cncw/model.lua` in CasioEmuX sets `hardware_id = 5`
   (`HW_CLASSWIZ_II`) but no alternative core is selected anywhere; the only
   difference from ES PLUS is the peripheral/memory configuration.

Consequence: the existing decoder in `src/cpu.rs` is the *right* instruction
decoder, but `src/machine.rs` implements the **ES PLUS board** only. Running CW
firmware on it would read unmapped memory as 0xFF/0 and never reach a valid
state, so this build deliberately reports unsupported instead of pretending.

## 2. ROM image

* Source (Casio copyrighted — **not** stored in this repository):
  <https://raw.githubusercontent.com/Physics365/991CN-X-CW-Decompilation_Files/main/真机ROM/991CN%20CW/rom.bin>
* Size: **1,048,576 bytes (1 MiB = 16 × 64 KiB pages)** — verified locally.
* Git blob: `6bb188ae8d60656c827fbe05053413e2bf693594`
* SHA-256: `4ab4791609cc8686fc6b4d1be34b3e6d19b3eb700abe8c9e3edd91ff2813471f`
* Header words: `[0x00000] = 0xF000` (initial SP), `[0x00002] = 0x636C` (initial
  PC in CSR 0). The ROM is erased (`0xFF`) from approximately `0xF2000` onward.
* Self-test signature: `"71CASG"` at **0x8FDE0** (followed by `"aYqgys"` at
  0x8FDE6, probably a check value). This is *not* the ES PLUS signature offset
  0x1FFF4; `model::detect()` now checks both offsets.

## 3. Model parameters (as implemented)

| Field | Value | Note |
|---|---|---|
| `id` | `fx991cncw` | CLI: `--model fx991cncw` |
| `name` | `Casio fx-991CN CW (VerA, real hardware)` | |
| `rom_size` | `0x100000` | 1 MiB; `Machine::with_model` pads short images with `0xFF` |
| `signature` | `b"71CASG"` | at 0x8FDE0, not 0x1FFF4 |
| `keyboard` | `KeyboardKind::HardwareSfr` | `real_hardware = 1`: KI/KO scan, no mailbox |
| `keys` | `keys::CWCW_KEYS` | 48 keys transcribed from `model.lua` |
| `lcd_width` | `192` | 24 bytes × 8 pixels per displayed row |
| `lcd_height` | `64` | 63 dot-matrix rows + 1 status row |
| `vram_base` | `0xF800` | primary framebuffer, 64 × 32 = 0x800 bytes |
| `key_mailbox` | `0x0000` | unused; emulator variants place it at `0x88E00` (does not fit u16) |
| `csr_mask` | `0x0F` | 4 implemented CSR bits → 16 possible 64 KiB code pages |

Board parameters **not** representable in the current `Model` struct (documented
here so a future board implementation can use them):

* RAM: `0x9000..0xEFFF` (0x6000 = 24 KiB) battery-backed; emulator variants add
  0x100 bytes at `0x89800`. (`real_hardware = 1` → no extra window.)
* Code/data ROM windows used by CasioEmuX for `HW_CLASSWIZ_II`:
  `0x00000–0x08FFF → ROM 0x00000`, `0x10000–0x1FFFF → ROM 0x10000`,
  `0x20000 → 0x20000`, `0x30000 → 0x30000`, `0x40000 → 0x40000`,
  `0x50000 → 0x50000`, `0x70000 → 0x70000`, and
  `0x80000–0x88DFF → ROM 0x00000` (bank-0 alias). Page `0x60000` is not mapped
  in the reference emulator; pages `0x90000–0xF0000` are not dispatched even
  though `csr_mask` permits them.
* Second framebuffer: `0x89000`, 0x800 bytes, blended with the primary one on
  CW ii (30 % / 70 %).
* Keyboard SFRs (same addresses as ES PLUS, wider KO field):
  KI `0xF040` (active low), input filter `0xF042`, KO mask `0xF044/0xF045`
  (10-bit, mask `0x03FF`), KO drive `0xF046/0xF047` (10-bit); keyboard IRQ
  index 5. Key codes follow `KO = 1 << (code >> 4)`, `KI = 1 << (code & 0x0F)`;
  `0xFF` is the ON/reset key. `pd_value = 0x00`.
* Other SFRs: DSR `0xF000`, standby `0xF008/0xF009`, timer `0xF020–0xF025`,
  screen range/mode/contrast `0xF030/0xF031/0xF032`, battery `0xF0D0–0xF0D2`,
  and a **BCD arithmetic coprocessor** at `0xF400`, `0xF402`, `0xF410`,
  `0xF414/0xF415`, `0xF480/0xF4A0/0xF4C0/0xF4E0`.

## 4. What works / what does not

**Works:**
* `model::by_id("fx991cncw")` / `--model fx991cncw` and `--list-models`.
* ROM auto-detection via the `"71CASG"` signature at 0x8FDE0.
* `Machine::with_model` accepts and size-validates/pads a 1 MiB image; no panic
  for 20-bit code addresses (reads above the implemented window return `0xFF`).
* The CW key matrix is available (`keys::CWCW_KEYS`) and
  `keys::pc_key_for(model.id, label)` resolves its labels for headless input.
* The CLI prints an explicit "execution is not supported" note when this model
  is selected.

**Does not work (deliberate, documented gap):**
* Firmware execution. `Model::execution_supported()` returns `false`; the
  CLI/GUI still start so ROM loading and debugging can be inspected, but the
  CPU will fetch `0xFF` from unmapped pages and will not boot the calculator.
* LCD rendering: `Machine::render()` still uses the ES PLUS 96×32 / single-VRAM
  layout. The 192×63 dual-framebuffer format is not decoded.
* The ClassWiz II BCD coprocessor, battery/standby SFRs, and the second VRAM
  are not emulated.
* Desktop GUI hit-testing still uses the ES PLUS 43×28 key grid and
  `keys::KEYS`, not `model.keys`; use headless mode with CW key labels.

## 5. Files changed in this work

`src/model.rs`, `src/keys.rs`, `src/machine.rs`, `src/main.rs`, `src/gui.rs`
(compile fix: `run_gui` now takes the selected `Model`), and this document.

## 6. Verification

* `cargo build --release` — OK.
* `cargo test` — 22 passed, 0 failed, 1 ignored (pre-existing long ROM test).
* CLI smoke test with the real ROM kept at `/tmp/cw_rom.bin` (never in the
  repo): `--list-models` lists `fx991cncw`; `--model fx991cncw --headless 5`
  loads the 1 MiB image, prints the unsupported note, and starts at PC
  `0x636C` / CSR 0 as expected from the ROM header.
