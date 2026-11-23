# casio-emu-rs — Casio Calculator Emulator

[![CI](https://github.com/devilAPI/casio-emu-rs/actions/workflows/ci.yml/badge.svg)](https://github.com/devilAPI/casio-emu-rs/actions/workflows/ci.yml)
[![Dev release](https://github.com/devilAPI/casio-emu-rs/actions/workflows/dev-release.yml/badge.svg)](https://github.com/devilAPI/casio-emu-rs/actions/workflows/dev-release.yml)
[![License: GPL-3.0-or-later](https://img.shields.io/badge/license-GPL--3.0--or--later-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)](https://www.rust-lang.org/)
[![WebAssembly](https://img.shields.io/badge/WebAssembly-yes-purple.svg)](#web--webassembly)

A cross-platform emulator for Casio nX-U8/100 calculators, written in Rust.
It runs original firmware on a Rust nX-U8/100 core, renders the real 96×32
(ES PLUS) and 192×64 (ClassWiz) LCDs, and works as:

- a native desktop app on **Linux, macOS and Windows** (`minifb`)
- a **WebAssembly** app in the browser (canvas + wasm-bindgen)
- a headless CLI for tests and automated accuracy checks

![casio-emu-rs fallback UI with 123 on the LCD](docs/screenshot.png)

> The screenshot shows the built-in fallback UI. If you extract the optional
> device image from your own copy of the official emulator, both the desktop
> and web frontends draw the real calculator skin as the background.

---

## Highlights

| | |
|---|---|
| **Accurate core** | 10,000,000-instruction differential trace and 49/49 key-matrix sweep match the reference core exactly |
| **Multi-model** | 15 registered models across ES PLUS, ClassWiz and ClassWiz II boards; switch at runtime with F6/F7 |
| **Real firmware** | Runs original Casio ROM dumps unmodified |
| **Real LCD** | 96×32 and 192×64 framebuffers with the correct VRAM packing |
| **Full key matrix** | 49 keys, shift/alpha, arrows, mode, DEL/AC, all functions |
| **Scalable windows** | Desktop window resizes smoothly; web canvas is responsive |
| **Device skin** | Optional official `3000.bmp` / `3001.bmp` artwork |
| **WebAssembly** | Same Rust core in the browser, no server-side emulation |
| **Headless mode** | Run N instructions, press keys, dump the LCD as PBM |
| **Reproducible accuracy** | C reference harness and Python sweep included |

## Quick start

### 1. Get your own firmware

The ROM is copyright CASIO and is **not included**. Dump it from a copy you
are licensed to use:

```bash
# Linux + Wine
python3 tools/extract_rom.py --out fx991es.rom.bin
```

On Windows, use [`fxesdev/RomDumper`](https://github.com/fxesdev/RomDumper).
See [`docs/ROM.md`](docs/ROM.md) for the full explanation, including why the
ROM must be dumped from the running emulator.

Obtain the official emulator from CASIO (https://edu.casio.com/) or your
licensed installer. This project intentionally does not link to third-party
firmware mirrors.

### 2. Build and run

```bash
cargo build --release
./target/release/casio-emu-rs --rom fx991es.rom.bin
```

The desktop binary searches `--rom`, `fx991es.rom.bin`, and
`roms/fx991es.rom.bin`.

### Supported models

```bash
./target/release/casio-emu-rs --list-models
```

The model is auto-detected from the ROM's self-test signature; use `--model ID`
to override it.  With `--model ID` and no `--rom`, the app loads
`roms/ID.rom.bin` automatically.  Run `tools/fetch_roms.sh` to download all
public ROM dumps into `roms/` (they are gitignored and never committed).

**In the desktop app, press F8 (or click the `MODEL: … [Change]` button in the
top bar) to open a full list of every model with its name and board tag.**
Click a row, or use Up/Down then Enter, to switch — the matching ROM in `roms/`
is loaded automatically. F6/F7 cycle models directly, Esc closes the list
(press it again to quit). Each keypad button shows its label and lights up
when pressed; the active model name is shown above the keys. The web app has a
styled model dropdown next to the ROM loader.

| id | model | board | status |
|---|---|---|---|
| `fx991es-plus-c` | fx-991ES PLUS C | ES PLUS | fully emulated and validated |
| `fx570es-plus` | fx-570ES PLUS (real HW ROM from `ZaoHan415/fxesplus`) | ES PLUS | boots; keys working (keyboard/timer IRQs) |
| `fx82esplusa` | fx-82ES PLUS A | ES PLUS | boots; keys working |
| `fx82esp-emu` | fx-82ES PLUS (emulator ROM) | ES PLUS | descriptor |
| `fx580vnx` | fx-580VN X (self-test CY-298A) | ClassWiz | boots; LCD and keys working |
| `fx570vnx` | fx-570VN X (CY-298) | ClassWiz | boots; LCD and keys working |
| `fx991ex` | fx-991EX (CY-235F) | ClassWiz | boots; LCD and keys working |
| `fx82ex` | fx-82EX VerE (CY-230E) | ClassWiz | boots; LCD and keys working |
| `fx991dex-verf` | fx-991DE X VerF (CY-251F) | ClassWiz | boots; LCD and keys working |
| `fx991dex-vere` | fx-991DE X VerE | ClassWiz | loads; boot stuck in RAM probe |
| `fx991cnx` | fx-991CN X (CY-239) | ClassWiz | boots; LCD and keys working |
| `fx991cnx-emu` | fx-991CN X (emulator ROM) | ClassWiz | descriptor (mailbox out of range) |
| `fx991cw` | fx-991CW (EY-005A, 512 KiB) | ClassWiz II | boots; LCD and keys working |
| `fx9910cw` | fx-9910CW 2nd ed. (FY-505A, 512 KiB) | ClassWiz II | loads; boot stuck (unmapped segment 6) |
| `fx991cncw` | fx-991CN CW (1 MiB) | ClassWiz II | boots; LCD and keys working |
| `fx880btg` | fx-880BTG (EY-023B, 1 MiB) | ClassWiz II | boots; LCD and keys working |

The three board fixes and their coexistence are documented in
[`FIX-ESPLUS.md`](docs/FIX-ESPLUS.md), [`FIX-CW2.md`](docs/FIX-CW2.md),
[`FIX-CLASSWIZ.md`](docs/FIX-CLASSWIZ.md) and
[`MERGE-NOTES.md`](docs/MERGE-NOTES.md).

See [`docs/MODELS.md`](docs/MODELS.md) for memory maps, ROM hashes, and notes on
additional Casio models.

### 3. Optional: device image

```bash
python3 tools/extract_skin.py --dll /path/to/fxESPLUS_P16.dll
```

This writes `assets/skin.png` and copies it into `web/www/` for the web build.

### 4. Web build

```bash
./web/build.sh
python3 -m http.server -d web/www 8000
# open http://localhost:8000/
```

Load your ROM with the file picker. See [`docs/WEB.md`](docs/WEB.md).

Hosted WebAssembly builds:

- **Release** (latest `v*` tag):
  **[devilapi.github.io/casio-emu-rs](https://devilapi.github.io/casio-emu-rs/)**
- **Dev** (`main` branch):
  **[devilapi.github.io/casio-emu-rs/dev/](https://devilapi.github.io/casio-emu-rs/dev/)**

Both are built and deployed together by
[`.github/workflows/pages.yml`](.github/workflows/pages.yml). Pushing a `v*`
tag updates the release build at `/`; pushing to `main` updates `/dev`. Each
page has a link to the other build.

---

## Accuracy

The CPU and machine are validated against the GPL-3.0 reference core
[`Fraserbc/u8_emu`](https://github.com/Fraserbc/u8_emu). Both cores run the
same firmware image; the harness compares every architectural register and a
checksum of the mutable memory.

| Test | Scope | Result |
|---|---:|---|
| Register/memory trace | 10,000,000 instructions | **10,000 / 10,000 checkpoints identical** |
| Key matrix sweep | 49 keys × 6,000,000 instructions | **49 / 49 identical** |
| Boot checkpoint | 200,000 instructions | exact |
| LCD after `1` | 6,000,000 instructions | exact bitmap |

The measured match rate is **100% on the tested corpus**, so the requested
**99.9%** threshold is met with margin. Accuracy is measured, not
mathematically proven; the SFR/timer/sound window is the main remaining
unknown. See [`docs/ACCURACY.md`](docs/ACCURACY.md) for methodology and
limitations.

Reproduce it:

```bash
git clone https://github.com/Fraserbc/u8_emu ../u8_emu
U8EMU_DIR=../u8_emu FX991_ROM=/path/to/fx991es.rom.bin tools/accuracy/run.sh
```

---

## Desktop controls

| Key | Calculator |
|---|---|
| `0`–`9`, `.` | digits |
| `+` `-` `*` `/` | arithmetic |
| `Enter` / `=` | equals |
| `Backspace` | DEL |
| `Delete` / `Esc` | AC (Esc also closes the window) |
| arrows | cursor |
| `S` `C` `T` | sin / cos / tan |
| `L` `N` | log / ln |
| `R` `P` | √ / x² |
| `X` `Y` `Z` | × / ×10ˣ / x⁻¹ |
| `A` `B` `H` | Abs / a⁄b / hyp |
| `F1`–`F4` | MODE / SHIFT / ALPHA / ON |
| `F5` | √ |

The on-screen keys are clickable. The **ON** button resets the CPU.

### Scalable windows

The desktop window opens at the active model's natural size and is freely
resizable. The device image, LCD, keypad and hitboxes are all scaled together
with nearest-neighbour sampling, and each LCD dot and key cell uses the same
scale on both axes, so nothing is stretched or clipped and the UI stays crisp.
On high-density displays, pass `--zoom 2` (or `3`/`4`) to open at a whole-number
multiple without any resampling.

The web canvas derives its width from both the viewport width and a height
budget, so the 289:582 device artwork keeps its aspect ratio on narrow phones
instead of being squashed, and it renders at the display's device-pixel ratio.

### Headless mode

```bash
# Boot and press "1" at instruction 2,000,000, then print the LCD.
./target/release/casio-emu-rs --rom fx991es.rom.bin \
  --headless 6000000 --press 2000000:1

# Write the LCD as a PBM image.
./target/release/casio-emu-rs --rom fx991es.rom.bin \
  --headless 6000000 --press 2000000:1 --dump lcd.pbm

# Print one line per instruction (reference-diff format).
./target/release/casio-emu-rs --rom fx991es.rom.bin --trace 1000

# Print a rolling register+memory hash every 1000 instructions.
./target/release/casio-emu-rs --rom fx991es.rom.bin --trace-hash 10000000

# Start from a raw RAM snapshot instead of zeroed RAM (board-size dependent).
./target/release/casio-emu-rs --model fx580vnx --ram roms/fx580vnx/ram.dmp \
  --headless 200000
```

---

## Project layout

```text
src/
  cpu.rs        nX-U8/100 core: decode, operands, ALU, control flow
  table.rs      generated instruction decode table
  model.rs      per-calculator descriptors and ROM auto-detection
  machine.rs    ROM/RAM/SFR/VRAM, keyboard interfaces, LCD render
  keys.rs       49-key fx-991ES PLUS C matrix + desktop mapping
  gui.rs        minifb frontend, skin loading, resizable canvas
  main.rs       CLI, headless/trace runners
web/
  src/lib.rs    wasm-bindgen bindings
  www/          HTML/JS/CSS frontend
tools/
  extract_rom.py    Linux/Wine firmware dumper
  fetch_roms.sh     download all public ROM dumps into roms/
  extract_skin.py   device artwork extractor
  accuracy/         differential harness + 49-key sweep
docs/
  ACCURACY.md   methodology and measured results
  MODELS.md     supported models, memory maps, ROM inventory and legal notes
  ROM.md        firmware/artwork extraction and legal notes
  WEB.md        web build and deployment
```

## Development

```bash
cargo fmt --all
cargo clippy --workspace --all-targets
cargo test
cargo build --release
```

ROM-dependent tests are skipped by default; run them with a ROM:

```bash
FX991_ROM=/path/to/fx991es.rom.bin cargo test --release -- --ignored
```

## CI and releases

- **CI** (`.github/workflows/ci.yml`) runs on every push and pull request:
  formatting, clippy, tests, release builds on Linux/Windows/macOS, and a
  WebAssembly build.
- **Pages** (`.github/workflows/pages.yml`) builds the WebAssembly app from the
  latest `v*` tag (served at `/`) and from `main` (served at `/dev`), then
  deploys both to GitHub Pages.
- **Dev release** (`.github/workflows/dev-release.yml`) runs on every push to
  `main`, builds all artifacts and updates the `dev` prerelease.
- **Release** (`.github/workflows/release.yml`) runs on `v*` tags and creates
  a full GitHub Release with desktop archives and the web bundle.

Artifacts never contain the ROM or the CASIO artwork.

## Legal and credits

The Rust source is licensed **GPL-3.0-or-later**. The CASIO firmware and the
official device artwork remain the property of CASIO COMPUTER CO., LTD. and
are **not** covered by this license. CASIO and fx-991ES are trademarks of
CASIO COMPUTER CO., LTD. This project is not affiliated with or endorsed by
CASIO.

The CPU semantics and decode table are a Rust port of
[`Fraserbc/u8_emu`](https://github.com/Fraserbc/u8_emu) (GPL-3.0). Thanks also
to the nX-U8/100 community, `LifeEmu/SimU8`, `fxesdev`, `user202729`, `Xyzstk`
and everyone who documented the Casio platform.
