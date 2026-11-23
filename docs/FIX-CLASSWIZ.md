# ClassWiz (hardware id 4) LCD/keyboard fix

This document records the root cause, the model-gated changes and the measured
validation for the ClassWiz boards (`fx570vnx`, `fx580vnx`, `fx991ex`, `fx82ex`,
`fx991dex-verf`, `fx991dex-vere`, `fx991cnx`).  The fx-991ES PLUS C reference
board is deliberately untouched.

All measurements below were taken with public ROM dumps placed in `roms/`
(git-ignored, never copied into the repository) and the headless CLI.  The
exact reproduction command is:

```bash
cargo build --release
./target/release/casio-emu-rs \
  --rom roms/fx570vnx.rom.bin \
  --headless 6000000 --press 2000000:1 --dump /tmp/lcd.pbm
```

## 1. Findings

### 1.1 Primary root cause: code segment 0 was truncated at 0xCFFF

`Machine::read_code_byte()` reused the **data** map for code fetches, so
CSR=0 offsets `0xD000..0xFFFF` returned `0xFF`.  In the reference core
(`user202729/CasioEmu`, `MMU::ReadCode`) code segment 0 bypasses the segment
dispatch entirely and reads the whole 64 KiB ROM page:

```cpp
if (!segment_index)
    return (rom_data[segment_offset + 1] << 8) | rom_data[segment_offset];
```

Only *data* segment 0 is split into ROM `0x0000..0xCFFF`, RAM `0xD000..0xEFFF`
and SFRs `0xF000..0xFFFF`.  The fx-580VN X firmware makes a far call to code
address `0xD736` at instruction ~58,608.  With the truncated map that read
`0xFFFF`, which this CPU decodes as `BRK`; the BRK vector (`0x0004`) points at
`0xA1D0`, a `B .` self-loop.  The emulator then spun in that loop for the
remaining ~5.94 million instructions, with `pc=a1d0`, `VRAM all zero` and a
blank panel.  Fixing the code map alone made the CASIO splash screen appear
(1,590 lit pixels), proving the CPU was otherwise healthy.

### 1.2 Secondary root cause: missing peripherals + standby state

Once boot proceeded, the firmware:

* writes `0x22` to the interrupt mask at `0xF010` (enabling maskable sources 0
  and 4 = keyboard and timer),
* configures the timer at `0xF020..0xF025`,
* drives KO lines via `0xF046` and samples KI at `0xF040`,
* sets screen mode 5 (normal) in the low three bits of `0xF031`, and
* performs the standby handshake `0x50 -> 0xA0` on `0xF008`, then writes bit 1
  of `0xF009` to enter STOP.

With no interrupt delivery the CPU never woke from STOP, and with no
timer/standby model `pc` kept drifting through standby code while VRAM was
cleared.  The panel therefore stayed blank even though the CPU was running.

## 2. Changes

All new behaviour is gated by `model.board == BoardKind::ClassWiz`; the ES PLUS
path returns before any of it runs.

| File | Change |
|---|---|
| `src/machine.rs` | Code segment 0 now maps the complete 64 KiB ROM page (fix above). Added ClassWiz SFR model: 16-bit interrupt mask (`0xF010`) and pending (`0xF014`), timer interval/counter/prescaler/control (`0xF020..0xF025`), screen mode (`0xF031`) and the `0xF008`/`0xF009` standby handshake. `tick()` advances the timer, freezes the CPU in HALT/STOP until an interrupt is pending, then delivers the lowest-numbered unmasked interrupt. `press()` raises the keyboard edge interrupt (source 0). `render()` honours the screen mode: only mode 5 shows the dot matrix; modes 4/6 clear it and all other modes are blank. |
| `src/cpu.rs` | Added `Cpu::accept_maskable_interrupt()`: saves ELR/ECSR/EPSW at exception level 1, clears MIE, enters EL 1 and jumps to the 16-bit vector at `index*2` in segment 0. Rejects interrupts while globally disabled or at EL>1, matching the reference chipset. |
| `src/model.rs` | New SFR/IRQ constants and `Model::rom_filename()` (the fx-82EX dump is published as `fx82ex-vere.rom.bin`). |
| `src/main.rs`, `src/gui.rs` | Use `rom_filename()` when auto-loading a model ROM. Added `--release STEP` to the headless runner so a key down/up pair can be scripted. |
| tests | 6 new ROM-free unit tests (code-window, timer IRQ, keyboard IRQ, screen-mode gating, standby wake-up, ES PLUS isolation) plus 2 CPU interrupt tests. |

Timer details: one prescaler count per instruction, `divider = 2^(F024 & 7)`;
writing `F022` clears the counter; the firmware's repeated writes to `F025`
do **not** clear it (matching the reference), which is what makes the periodic
interrupt reachable.  Interrupt sources use the reference bit layout
(`bit n+1` of mask/pending) and vectors 5 (keyboard, `0x000A`) and 9 (timer,
`0x0012`).

## 3. Measured before / after

`fx570vnx.rom.bin`, 6,000,000 headless instructions, `--press 2000000:1`:

| State | PC at 6M | CPU cycles | Lit pixels (192x64) | Display |
|---|---:|---:|---:|---|
| Before any fix | `0xA1D0` | 6,000,006 | **0** | blank (BRK trap loop) |
| Code-map fix only | `0xAC36` | 6,000,030 | 1,590 | CASIO splash logo |
| Full fix, no key | `0x9F7A` | ~1,137,719 | 41 | input line + blinking cursor |
| Full fix, key `1` held | `0x8C98` | ~2,486,766 | **71** | input line shows `1` (`11` with auto-repeat) |

The key press changes the LCD in **6 of the 7 target models**:

| model | no key | key `1` at 2M | changed? |
|---|---:|---:|:---:|
| fx570vnx | 41 | 71 | yes |
| fx580vnx | 41 | 71 | yes |
| fx991ex | 41 | 71 | yes |
| fx82ex | 41 | 71 | yes |
| fx991dex-verf | 41 | 71 | yes |
| fx991dex-vere | 0 | 0 | **no (see §5)** |
| fx991cnx | 41 | 71 | yes |

`cmp` of the two PBM dumps for the six working models reports a differing LCD
bitmap (the glyph is drawn at the top-left of the dot matrix).

### Regression checks (all run in the isolated copy)

```
cargo build --release                          OK
cargo test                                     33 passed; 0 failed; 1 ignored
FX991_ROM=<path to fx991es.rom.bin> \
  cargo test --release -- --ignored            1 passed  (ES PLUS C accuracy)
```

The 6M-instruction ES PLUS C checkpoint (`pc=0x538a`, 32 lit pixels after key
`1`) and the 200k boot checkpoint are unchanged.

## 4. Why the display looks like a cursor, not the splash

With the standby model enabled the firmware spends most of the 6M *wall-clock*
steps halted (PSW `0x80` at the stop point); only ~2.5M instructions actually
execute.  At that point boot has finished, the splash has been cleared and the
calculator is showing the empty input line with a blinking cursor.  This is
the expected real-hardware state, and a key press inserts a digit.

## 5. Remaining gaps

* **fx-991DE X VerE** (`roms/fx991dex-vere.rom.bin`) still boots into a RAM
  probe loop at code addresses `0xA876..0xA898`.  It reads through DSR `0x44`
  (which is unmapped on the ClassWiz board) and never sees writable memory, so
  it retries forever from `0xA84E`.  The dump has no self-test signature and a
  different reset vector (`0xA844`) from the other ClassWiz ROMs (`0xA1CE`),
  so it is likely a different/hardware-specific firmware revision.  It is
  detected and loaded but its LCD stays blank.
* The keyboard IRQ is a key-down edge only; ghosting and KO model-ID resistor
  decoding are still simplified (one held key, `KI = ~bit` gated by
  `ko_drive & ~ko_mask`).
* The status row is icon *flags* in real hardware.  `render()` still expands
  those 32 bytes as raw pixels because no device skin is bundled, so the top
  row is not pixel-accurate; the 63 dot rows are.
* Timer tick scaling (1 prescaler count per instruction) was chosen to match
  the firmware's observed ~20-instruction short delay with `F024=1`; it is
  not yet cycle-accurate against real hardware.
* ClassWiz II (hardware id 5) remains unsupported.
