# Casio fx-880BTG — model notes

Status: **recognised model, firmware execution NOT supported** (same CW ii
board gap as [`MODELS-991CNCW.md`](MODELS-991CNCW.md)).

## 1. Which device is this?

There are two unrelated products sold under the name "fx-880BTG"; they must
not be confused:

1. **The genuine Casio fx-880BTG** is a Vietnamese/Bitex-exclusive **ClassWiz
   II (CW) generation** calculator.  Casio's own manuals list it next to the
   ClassWiz models, its self-test version is `EY-023` revision B with checksum
   `8113`, and the successor fx-880BTG Plus changes the software ID to
   `FY-523`.
   - <https://fxesdev.github.io/calcupedia_backup/docs/diagnostic.html>
   - <https://tipla.net/news>
   - <https://support.casio.com/global/vi/calc/manual/fx-880BTG_vi>
   - <https://classpad.net/intl/billing> (Casio groups it with `fx-…CW`,
     `fx-580VN X`, `fx-CG100`)
2. **A counterfeit "fake Casio fx-880BTG"** also circulates.  It is an
   **ESP32-S3**-based device with an ST75256 192x63 LCD driven over SPI, and is
   the subject of an Arduino library that explicitly calls it "the fake Casio
   fx-880BTG".  That is a different machine entirely (Xtensa, not nX-U8) and is
   *not* what an nX-U8 ROM dump would come from.
   - <https://github.com/tkhoi0821-svg/Casio-classwiz-880BTG-lib>
   - <https://registry.platformio.org/libraries/tkhoi0821-svg/CasioST75256>

## 2. CPU

For the genuine calculator the CPU is the **nX-U8 core** (commonly nX-U8/100),
not SH/Xtensa.  `fxesdev/CasioEmuX` — an emulator for nX-U8/100 and
nX-U16/100 Casio calculators — ships a `fx880btg_emu` model descriptor with:

| field | value |
|---|---|
| `model_name` | `fx-880BTG` |
| `hardware_id` | `5` (= HW_CLASSWIZ_II) |
| `real_hardware` | `0` (dump from Casio's Windows emulator) |
| `csr_mask` | `0x000F` |
| `pd_value` | `0x00` |

<https://github.com/fxesdev/CasioEmuX/blob/2a32ed1/models/fx880btg_emu/model.lua>

That is the same CPU and the same board generation as the fx-991CN CW already
in this repository (`hardware_id = 5`).  The existing decoder in `src/cpu.rs`
is therefore the right instruction decoder, but `src/machine.rs` implements
only the ES PLUS board: CW ii has 24 KiB RAM at `0x9000`, two 192x64
framebuffers, a BCD coprocessor and a different ROM-window map.  Running the
firmware would fetch `0xFF` from unmapped pages, so `execution_supported()`
returns `false`.

## 3. Parameters used by the descriptor

| field | value | source / note |
|---|---|---|
| `id` | `fx880btg` | CLI: `--model fx880btg` |
| `rom_size` | `0x100000` (1 MiB, 16 x 64 KiB pages) | reported dump size; CW ii reference board |
| `signature` | `EY-023` | self-test version; revision B, `SUM8113` |
| `signature_offset` | `0x071FEE` | inferred (see §4) |
| `board` | `ClassWizII` | `hardware_id = 5` |
| `keyboard` | `HardwareSfr` | real HW: KI `0xF040`, KO `0xF046/47`, mask `0xF044/45`; emulator dump uses mailbox `0x88E00` (out of u16 range) |
| `keys` | `keys::KEYS` | ~48 key codes from `model.lua`; generic table reused |
| `lcd_width` / `lcd_height` | `192` / `64` | 63 dot rows + status row |
| `vram_base` | `0xF800` | primary framebuffer, 0x800 bytes; second at `0x89000` |
| `key_mailbox` | `0` | unused (emulator mailbox `0x88E00` does not fit) |
| `csr_mask` | `0x0F` | 4 implemented CSR bits, 16 code pages |

Board details not representable in the `Model` struct (from the CW ii
reference emulator): RAM `0x9000..0xEFFF` (24 KiB); code windows
`0x00000–0x08FFF → ROM 0x00000`, `0x10000–0x7FFFF` mapped directly,
`0x80000–0x88DFF → ROM bank 0`; keyboard IRQ 5; second VRAM `0x89000`
blended 30/70; BCD coprocessor at `0xF400/402/410/414/415/480/4A0/4C0/4E0`.

Boot vector of the 1 MiB fx-991CN CW reference image: initial SP `0xF000`,
initial PC `0x636C` in CSR 0.  **Not verified for the fx-880BTG image.**

## 4. Signature-offset derivation (important caveat)

The ROM dump named in the task (`/home/devlapi/Downloads/fx-880BTG.bin`,
1,048,576 bytes, SHA-256
`39dd1984fef9338e8e482eeabd10408f510806604c5e88e0b178d126ae3e3c53`) **was
not present on that path or anywhere on the machine**, so no binary/strings
analysis of it was possible.  The offset `0x71FEE` is inferred from the 1 MiB
CW ii dumps already in `roms/`:

| ROM | version at `0x71FEE` | version at `0xF1FEE` | checksum bytes at +8 |
|---|---|---|---|
| `fx991cncw.rom.bin` (1 MiB) | `EY-027A` | `EY-027A` | `f6 b5` = `0xB5F6` |
| `fx991cw.rom.bin` (512 KiB) | `EY-005A` | (n/a) | `5b e0` = `0xE05B` |
| `fx570vnx.rom.bin` (256 KiB) | `CY-298A` at `0x3FFEE` | (n/a) | `26 bb` = `0xBB26` |

Those checksums match Calcupedia's `EY-027 A → B5F6`, `EY-005 A → E05B` and
`CY-298 A → BB26`, confirming the layout `version[7] + checksum u16 LE`.
For the fx-880BTG Calcupedia gives `EY-023 B → 8113`, so a matching image
should contain `45 59 2D 30 32 33 42 20 13 81` at `0x71FEE`.  If auto-detection
does not fire, use `--model fx880btg`; the record is still valid for ROM
loading/inspection.

## 5. What works / what does not

**Works:** `--list-models`, `--model fx880btg`, signature detection for an image
matching §4, 1 MiB ROM size validation, explicit "execution is not supported"
notice.

**Does not work:** firmware execution, LCD rendering, CW ii BCD coprocessor,
second framebuffer, keyboard IRQ, exact emulator mailbox.  The descriptor is
for recognition and ROM inspection only.

**Naming ambiguity:** a different, counterfeit ESP32 device is also sold as
"fx-880BTG".  If the supplied dump turns out to be that device's firmware,
none of the above applies and this descriptor should be removed or replaced.

## 6. Verification

* `cargo build --release` — OK.
* `cargo test` — 26 passed, 0 failed, 1 ignored (pre-existing long ROM test).
* CLI with a synthetic 1 MiB image carrying `EY-023B` + checksum at `0x71FEE`:
  auto-detection identifies the model and the unsupported-board notice is
  printed before execution starts.

`model::detect()` now also rejects images shorter than a model's declared
`rom_size`, so a truncated/fake image cannot be auto-detected as the 1 MiB
fx-880BTG.
