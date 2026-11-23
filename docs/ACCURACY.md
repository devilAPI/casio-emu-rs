# Accuracy

The emulator is validated differentially against the GPL-3.0 reference core
[`Fraserbc/u8_emu`](https://github.com/Fraserbc/u8_emu), which implements the
same nX-U8/100 ISA. The same licensed firmware image is loaded into both
cores.

## Measured results

| Test | Scope | Result |
|---|---|---|
| Register/memory trace | 10,000,000 instructions | **10,000 / 10,000 hash checkpoints identical** |
| Key matrix sweep | 49 keys, each run to 6,000,000 instructions | **49 / 49 keys identical** |
| Boot checkpoint | 200,000 instructions | `PC=0xdaa6`, `CSR=1`, `PSW=0x00` |
| Key checkpoint | `1` pressed at instruction 2,000,000, run to 6,000,000 | `PC=0x538a`, `PSW=0`, 32 LCD pixels |

The trace hash covers:

- `PC`, `CSR`, `LCSR`, `ECSR[3]`
- `LR`, `ELR[3]`
- `PSW`, `EPSW[3]`
- `SP`, `EA`, `DSR`
- all 16 general-purpose registers
- a checksum of the full mutable memory image (RAM + SFR + keypad)

The key sweep compares the final `PC`, `CSR`, `PSW` and the complete 96×32 LCD
bitmap for every key in the fx-991ES PLUS C matrix.

A measured match rate of 100% on these tests means the requested 99.9%
threshold is met with margin **for the tested corpus**. It is not a formal
proof for every possible undocumented SFR/timer/sound edge case; see the
limitations below.

## Reproduce

```bash
git clone https://github.com/Fraserbc/u8_emu ../u8_emu

cd casio-emu-rs
U8EMU_DIR=../u8_emu \
FX991_ROM=/path/to/fx991es.rom.bin \
tools/accuracy/run.sh
```

The script builds two small C harnesses against the reference core, builds
the Rust release binary, compares the 10M-instruction trace, and runs the
49-key sweep.

## Unit tests

```bash
cargo test
```

The default suite covers ALU flags, zero-width shifts, the `TB` bit-access
flag behavior, and the VRAM packing layout. ROM-dependent tests are skipped
unless a ROM is present; run them with:

```bash
FX991_ROM=/path/to/fx991es.rom.bin cargo test --release -- --ignored
```

## Limitations

- The SFR window is backed by RAM. The official emulator implements some
  timer, interrupt and sound behavior there. Normal input, menus, display
  and the tested firmware paths match.
- The firmware image and the official device artwork are proprietary and are
  not part of this repository.
- Accuracy is measured, not mathematically proven. If you find a mismatch,
  add the failing state to the differential harness before changing the core.
