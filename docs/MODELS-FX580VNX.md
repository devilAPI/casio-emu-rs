# fx-580VN X archive notes (`fx580vnx.rar`)

The user-supplied `fx580vnx.rar` (RAR5) contains four files:

| File | Size | SHA-256 |
|---|---:|---|
| `rom.bin` | 262,144 (`0x40000`) | `7a164c650d6e6bc89e220caf8ac8349724adc2021e59649d428adae6b509e267` |
| `ram.dmp` | 8,192 (`0x2000`) | `3c75c22d22abc1665efd35c71b93f957fe8a169f3118eb4b8b84d8d2e7da11f3` |
| `config.bin` | 2,722 (`0xAAA`) | `701695b15f6a9b80dc81743a97dbef86c79d57b927d86761e24281a1715261b6` |
| `interface.png` | 89,429 | `47617ffbd5dedad1b0ef2ed53574c1c48be6e9bacb000e2cd5315ef1270daac1` |

**The ROM is already registered.** `rom.bin` is byte-identical to the
`roms/fx580vnx.rom.bin` / `roms/fx570vnx.rom.bin` image described in
[`MODELS-570VNX.md`](MODELS-570VNX.md): 256 KiB ClassWiz image, self-test
`CY-298A` at `0x3FFEE`, SHA-256 as above. Model id `fx580vnx` was added to
`src/model.rs` ahead of `fx570vnx` (whose shorter `CY-298` signature is a
prefix of `CY-298A`). No new model descriptor or ROM import is needed.

The other three files are **not** required to run the emulator. None of them
is stored in this repository (CASIO firmware/artwork is copyrighted); the
notes below are documentation of third-party files supplied by the user.

## `ram.dmp` — raw ClassWiz battery-backed RAM

This is a headerless, raw 8 KiB image of the data-space window mapped at
`0x0D000..0x0EFFF` on DSR 0. File offset `i` is the byte at `0xD000 + i`.
This matches `BatteryBackedRAM::Initialise()` in
`user202729/CasioEmu` (`stable` branch): for `HW_CLASSWIZ` the buffer is
`0x2000` bytes at base `0xD000`, and its `ram=` option loads/saves the file
with a plain `read`/`write` of the whole buffer — no header or checksum.
(ES PLUS uses `0xE00` bytes at `0x8000`, optionally plus `0x100` bytes at
`0x9800` for emulator ROMs.)

Only 1,458 of 8,192 bytes (17.8%) are non-zero, consistent with a mostly
fresh device state. Concrete landmarks (file offsets = RAM address − `0xD000`):

| Offset | Bytes | Likely meaning |
|---|---|---|
| `0x000C..0x0013` | `bf 00 ca 00 7e 76 00 01` | small settings header; `CA 00 7E` recurs at `0x0DD0` and `0x13D9` (duplicated settings blocks) |
| `0x005C..0x005D` | `1a d3` | little-endian pointer `0xD31A` into this RAM window |
| `0x0144..0x0145` | `80 d1` | pointer `0xD180` |
| `0x0318..0x0319` | `7a 20` | single 16-bit value |
| `0x0BD0..0x0BDE` | `0f 0e … 01 00` | descending 16-byte lookup table (`0x0F` down to `0x00`) |
| `0x0BE2..0x0BE4` | `0e 07 bf` | trailing settings byte near the table |
| `0x19D4..0x1DEB` | dense data | 247 16-bit little-endian pointers into `0xD000..0xEFFF` in total; this region looks like a variable/name/pointer table (calculator variables A, B, …, matrix/list data) |
| `0x1E29..0x1F37` | `0f 80 19 c0 30 c0 …` | 16-bit-packed glyph/bitmap-like patterns followed by byte sequences with many `0xEF` opcodes — likely firmware helper code/data and stack copied into RAM |
| `0x1F68..0x1F6B` | `43 61 6c 63` | ASCII `Calc` |
| `0x1F80..0x1F97` | repeated `00 07 00 07 00 07 …` | small structured table at the top of RAM |

Interpretation of individual variables is **not** verified; the above are
observable facts and plausible readings, not a decoded Casio RAM format.
Notably, **display contrast is not in this dump**: on ClassWiz it lives in
the SFR page at `0xF030..0xF032`, outside the RAM window.

## `config.bin` — configuration of the source PC emulator

This is not a calculator file. The ASCII header identifies it as the config
of the tool that produced the dump:

- `0x000A`: `nx-U16/U8 Emulator Configuration file v51`
- `0x0035`: Chinese title `模拟键盘布局v51` (UTF-8)
- `0x0050`: Vietnamese title `Tệp cấu hình bố trí bàn phím v51` (UTF-8,
  partially mangled in `strings` output)

From `0x0073` onward it is a little-endian binary table of **key-binding
records**: a `u32` name length, four zero bytes, the ASCII name, then `u32`
on-screen button geometry (x/y/w/h) and a host virtual-key code. Examples:

- name `Backspace` at `0x00F3` (length 9 stored at `0x00EB`), rectangle
  x=404, y=41, 29×29 at `0x0100..0x010F`;
- `Space` at `0x0118` (length 5 at `0x0110`), x=444, y=41, 29×29;
- `Return` at `0x02CD` (length 6), x=304, y=33, 29×20;
- arrow keys (`Left`/`Right`/`Upx`/`Down`) around `0x05D7..0x063F` with
  host VK codes `0x25`/`0x27`/`0x26`/`0x28`;
- 22 `rsd_*` records at `0x064A..0xA43` (`rsd_a7`, `rsd_angle`,
  `rsd_cmplx`, `rsd_dg`, `rsd_down`, `rsd_e`, `rsd_fix`, `rsd_gv`,
  `rsd_interface`, `rsd_left`, `rsd_m`, `rsd_mathV`, `rsd_pause`,
  `rsd_pixel`, `rsd_ro`, `rsd_right`, `rsd_s`, `rsd_sci`, `rsd_stoI`,
  `rsd_sun`, `rsd_up`, `rsd_wdown`) — these map host keys to the 20 status-bar
  icons visible in `interface.png` (S, A, M, STO, MATH, D, R, G, FIX, SCI, E,
  CMPLX, ANGLE, WDOWN, arrows, PAUSE, SUN). They are PC-emulator UI toggles,
  not calculator state.

Tail (length-prefixed strings): `interface.png` at `0x0A66` (13 chars),
model label `fx-580VN X` at `0x0A7B` (10 chars), `rom.bin` at `0x0A8D`
(7 chars). Fields are byte-packed with a few alignment bytes; the exact
C++ struct layout was not reverse engineered and is not needed by this
project. There is no ROM or RAM payload in `config.bin`.

## `interface.png` — device artwork

307×615 RGBA PNG: a Casio CLASSWIZ device image with the colour keypad and
an empty LCD; it also shows the status icons referenced by the `rsd_*`
bindings above. It is the source emulator's skin, not the ES PLUS skin that
this project extracts via `tools/extract_skin.py`, and it is **not bundled**
here. It is not used by the desktop app (which draws a built-in reflowing
UI); its LCD and key hitboxes remain those compiled into `src/keys.rs` /
`src/model.rs`.

## Loading the RAM snapshot

`Machine::load_ram(&[u8])` copies a raw snapshot into the board's RAM window
(clearing it first, clipping oversized data to `ram_size()`). The CLI option
is:

```bash
./target/release/casio-emu-rs --model fx580vnx \
  --rom roms/fx580vnx.rom.bin \
  --ram /path/to/fx580vnx/ram.dmp \
  --headless 200000
```

Default behaviour is unchanged when `--ram` is absent. The option works in
GUI, headless, trace and trace-hash modes; in the GUI it applies to the
initial model only (F6/F7 model switches start with zeroed RAM). ROM-free
unit tests cover ES PLUS and ClassWiz windows and clipping.

**Observed effect on boot:** none that advances execution. With the real ROM,
both zeroed RAM and this snapshot stall at `pc=a1d0 csr=00`, repeatedly
executing the word `ceff` (a sleep/wait) after ~200k instructions; only the
PSW differs (`0x8E` vs `0x2A`) and the LCD remains blank even after 5 M
instructions. The blockers are still the unimplemented timers, interrupts
and standby/screen-mode SFRs (`0xF008`, `0xF020..0xF025`, `0xF030..0xF032`),
as noted in [`MODELS-570VNX.md`](MODELS-570VNX.md). Loading a *real* snapshot
may restore variables/settings for a machine that can already boot, but it
does not substitute for those peripherals.
