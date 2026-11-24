# Supported calculator models

One nX-U8/100 core can run firmware for a large family of Casio calculators.
What changes between models is the board around the CPU — ROM size, memory
map, LCD geometry and how key presses reach the firmware.  `src/model.rs`
describes those differences as a `Model` descriptor.

```text
--list-models              list all recognised ids
--model ID                 override auto-detection; loads roms/ID.rom.bin
                           if --rom is not given
--rom path/to/rom.bin      ROM dump for that model
--ram path/to/ram.bin      optional raw RAM snapshot (see MODELS-FX580VNX.md)
```

`tools/fetch_roms.sh` downloads all public ROM dumps into `roms/`.  In the
desktop app, F6/F7 (or the on-screen `<` / `>` selector) cycles through every
model and loads the corresponding ROM from `roms/`; the web app has a model
dropdown.  ROM files are gitignored and never committed.

The model is normally detected from a self-test signature stored in the ROM:
`LY727X…` / `GY454X…` / `LY713X…` / `GY450X…` at `0x1FFF4` (ES PLUS),
`CY-298A` / `CY-298` / `CY-235F` / `CY-230E` / `CY-251F` / `CY-239` at
`0x3FFEE` (ClassWiz), `EY-005` / `FY-505` at `0x5FFEE` and `71CASG` at
`0x8FDE0` (ClassWiz II).  Models without a readable signature require an
explicit `--model ID`.

## Board generations

| board | code | RAM | LCD / VRAM | keyboard |
|---|---|---|---|---|
| ES PLUS | 2 × 64 KiB pages, `csr_mask=0x01` | 3.5 KiB at `0x8000` | 96×32, `0xF800`, 16-byte rows | mailbox `0x8E00` (emulator ROMs) or KI `0xF040` / KO `0xF046` (real hardware) |
| ClassWiz | 4 × 64 KiB pages + mirror at CSR 5, `csr_mask=0x0F` | 8 KiB at `0xD000` | 192×64, `0xF800`, 32-byte rows | KI `0xF040` / KO `0xF046` |
| ClassWiz II | 512 KiB or 1 MiB ROM, `csr_mask=0x0F` | 24 KiB at `0x9000` | dual 192×64 framebuffers | KI/KO + BCD coprocessor |

## Model registry

| id | model | ROM | board | execution |
|---|---|---:|---|---|
| `fx991es-plus-c` | fx-991ES PLUS C | 128 KiB | ES PLUS | **validated** |
| `fx570es-plus` | fx-570ES PLUS (real HW) | 128 KiB | ES PLUS | boots, keys working |
| `fx82esplusa` | fx-82ES PLUS A (real HW) | 128 KiB | ES PLUS | boots, keys working |
| `fx82esp-emu` | fx-82ES PLUS (emulator ROM) | 128 KiB | ES PLUS | descriptor |
| `fx580vnx` | fx-580VN X (CY-298A) | 256 KiB | ClassWiz | boots; LCD and keys working |
| `fx570vnx` | fx-570VN X (CY-298) | 256 KiB | ClassWiz | boots; LCD and keys working |
| `fx991ex` | fx-991EX (CY-235F) | 256 KiB | ClassWiz | boots; LCD and keys working |
| `fx82ex` | fx-82EX VerE (CY-230E) | 256 KiB | ClassWiz | boots; LCD and keys working |
| `fx991dex-verf` | fx-991DE X VerF (CY-251F) | 256 KiB | ClassWiz | boots; LCD and keys working |
| `fx991dex-vere` | fx-991DE X VerE | 256 KiB | ClassWiz | loads; boot stuck in RAM probe |
| `fx991cnx` | fx-991CN X (CY-239) | 256 KiB | ClassWiz | boots; LCD and keys working |
| `fx991cnx-emu` | fx-991CN X (emulator ROM) | 256 KiB | ClassWiz | descriptor (mailbox out of range) |
| `fx991cw` | fx-991CW (EY-005A) | 512 KiB | ClassWiz II | boots; LCD and keys working |
| `fx9910cw` | fx-9910CW 2nd ed. (FY-505A) | 512 KiB | ClassWiz II | loads; dump fails model/integrity checks |
| `fx991cncw` | fx-991CN CW | 1 MiB | ClassWiz II | boots; LCD and keys working |
| `fx880btg` | fx-880BTG (EY-023B) | 1 MiB | ClassWiz II | boots; LCD and keys working |

Only `fx991es-plus-c` is fully validated: 10 M instructions and a 49-key
sweep match the `Fraserbc/u8_emu` reference core exactly
([`ACCURACY.md`](ACCURACY.md)).

The real-hardware ES PLUS image in [`ZaoHan415/fxesplus`](https://github.com/ZaoHan415/fxesplus)
is an **fx-570ES PLUS** (ROM SHA-256
`1a1b575f130b3fa4c79a598881181f5a137edb45e398cd9b78ae5baf9b071071`).
It never touches the emulator mailbox; the machine tracks the physical KI/KO
registers: KI `0xF040` is active low, KO drive `0xF046/47`, KO mask
`0xF044/45` (KO7–KO9 are model-ID/Pd resistors).

ROM images remain copyrighted by CASIO COMPUTER CO., LTD.  This repository
never includes them.  Use only dumps from a calculator or emulator you are
licensed to use.

## ROM inventory

| model | URL (ROMs are **not** bundled) | size | hash |
|---|---|---:|---|
| fx-570ES PLUS | `ZaoHan415/fxesplus` → `dumps/rom.bin` | 128 KiB | SHA-256 `1a1b575f…71071` |
| fx-82ES PLUS A | `user202729/fxesplus` → `82espa/rom.bin` | 128 KiB | blob `88123810…92f4` |
| fx-82ES PLUS (emu) | `user202729/fxesplus` → `82esp/emu_rom.bin` | 128 KiB | blob `edcda212…c756` |
| fx-570VN X | `user202729/fxesplus` → `580vnx/rom.bin` | 256 KiB | blob `3105c5e4…4699` |
| fx-570VN X (emu) | `user202729/fxesplus` → `580vnx_emu/rom.bin` | 256 KiB | blob `f956589e…818a` |
| fx-991CN X | `user202729/fxesplus` → `991cnx/rom.bin` | 256 KiB | blob `850fcc63…2184` |
| fx-991CN X (emu) | `user202729/fxesplus` → `991cnx_emu/rom.bin` | 256 KiB | blob `f1c0caf0…7a85` |
| fx-991CN CW | `Physics365/991CN-X-CW-Decompilation_Files` → `991CN CW/rom.bin` | 1 MiB | blob `6bb188ae…3594` |

Local dumps placed in `roms/` by the user (also gitignored): `fx580vnx` (CY-298A),
`fx991ex` (CY-235F), `fx82ex-vere` (CY-230E), `fx991dex-verf` (CY-251F),
`fx991dex-vere`, `fx991cw` (EY-005A), `fx9910cw` (FY-505A) and `fx880btg`.
The signature identifies the firmware; several models share one ROM image.

## Detailed notes

- [`MODELS-82ESPLUS.md`](MODELS-82ESPLUS.md) — fx-82ES PLUS A and emulator ROM.
- [`MODELS-570VNX.md`](MODELS-570VNX.md) — ClassWiz board, memory map and keys.
- [`MODELS-991CNCW.md`](MODELS-991CNCW.md) — ClassWiz II chip identification.
- [`MODELS-880BTG.md`](MODELS-880BTG.md) — fx-880BTG identification and parameters.
- [`MODELS-FX580VNX.md`](MODELS-FX580VNX.md) — fx-580VN X archive analysis and `--ram` option.
- [`MODELS-ESPLUS-FAMILY.md`](MODELS-ESPLUS-FAMILY.md) — 128 KiB ES PLUS / 2nd-edition models.
- [`MODELS-CWX-FAMILY.md`](MODELS-CWX-FAMILY.md) — 256 KiB ClassWiz X models.
- [`MODELS-CW2-FAMILY.md`](MODELS-CW2-FAMILY.md) — 512 KiB / 1 MiB ClassWiz II models.
- [`FIX-ESPLUS.md`](FIX-ESPLUS.md) — real-hardware keyboard/timer interrupt fix and validation.
- [`FIX-CW2.md`](FIX-CW2.md) — CW ii board (memory map, dual VRAM, IRQ/timer).
- [`FIX-CLASSWIZ.md`](FIX-CLASSWIZ.md) — ClassWiz code window, peripherals and LCD.
- [`MERGE-NOTES.md`](MERGE-NOTES.md) — how all three fixes coexist.

## Known gaps

`fx991dex-vere` loads but does not reach a usable LCD (different
firmware revisions; see the fix notes).  CW ii's BCD coprocessor is
register-only (`RunCommand` and the macro engine are not implemented), status
icons are not decoded, and timer/LSCLK timing is approximate.  The ES PLUS
C reference board remains the only C-model-validated configuration.

Research credits: `user202729/CasioEmu` (GPL-3.0), `Xyzstk/CasioEmuX`,
`qiufuyu123/CasioEmuNeo`, `Physics365` decompilations, the casiocalc.org
fx-ES PLUS hacking thread and the fx-es(ms) Tieba archive.  Notes from the
"Casio Calculator Hacking" Discord are in
[`DISCORD-RESEARCH.md`](DISCORD-RESEARCH.md).
