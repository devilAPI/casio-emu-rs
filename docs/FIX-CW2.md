# ClassWiz II (CW ii, hardware id 5) board support

Status: **board implemented; three of four target ROMs boot to a usable LCD and
respond to keys. `fx9910cw` remains a documented gap (see §5).**

All work was done in an isolated copy (`/tmp/fix-cw2`); the main repository and
the read-only ROM dumps were not modified and no ROM was copied into the repo.

## 1. Hardware parameters (from CasioEmuX `stable`, GPL-3.0)

Reference files: `models/fx991cncw/model.lua`,
`emulator/src/Chipset/MMU.cpp`, `Chipset/Chipset.cpp`, `InterruptSource.cpp`,
`CPU.cpp`, and `Peripheral/{ROMWindow,BatteryBackedRAM,Screen,Keyboard,Timer,
Miscellaneous,StandbyControl,BCDCalc}.cpp`. Cross-checked against
`docs/MODELS-991CNCW.md`, `docs/MODELS-880BTG.md` and the ROM headers.

| Parameter | Value |
|---|---|
| CPU core | nX-U8/100 (unchanged); reference selects CM_NX_U16 for CW ii |
| CSR mask | `0x0F`; DSR mask `0x1F` |
| ROM | 512 KiB (`fx991cw`, `fx9910cw`) or 1 MiB (`fx991cncw`, `fx880btg`) |
| RAM | 24 KiB at data `0x9000..0xEFFF` |
| Data ROM windows | seg 0: `0x0000..0x8FFF`; segs 1–7, 9–15: direct; seg 8: `0x80000..0x8FFDF` (+ second VRAM at `0x89000` on emulator builds) |
| Code segments | seg 8 → ROM bank 0; segs 9–15 alias 1–7 (`csr & 7`); seg 6 and seg 7 ≥ `0x2000` unmapped; seg 0 ≥ `0xFE00` unmapped |
| SFR page | `0xF000..0xFFFF`, same KI/KO addresses (`KI=0xF040`, mask `0xF044/45`, drive `0xF046/47`) |
| Interrupt mask/pending | `0xF010..0xF013` / `0xF014..0xF017`, 21 maskable sources; bit *i+1* enables/flags source *i*; vector = `(5+i)*2` |
| Keyboard IRQ | source 0 (EXI0), vector `0x000A` |
| Timer 0 IRQ | source 4 (TM0), vector `0x0012`; registers `0xF020..0xF025` (LSCLK/HSCLK selectable) |
| LCD | 192×64: status row + 63 dot rows, 32-byte stride, 24 displayed bytes |
| Framebuffers | primary at `0xF800`, second at `0x89000` (emulator) / bank-switched by `0xF037` bit 2 (real HW); blended on display |
| BCD coprocessor | `0xF400/402/404/405/410/414/415` and `0xF480/4A0/4C0/4E0` (4×12-byte registers) |
| Boot vectors | SP `0xF000`; PC `0x636C` (fx991cncw), `0x6166` (fx991cw), `0x680A` (fx9910cw), `0x6F04` (fx880btg); CSR 0 |
| Self-test | `71CASG` @ `0x8FDE0` (1 MiB), `EY-005A`/`FY-505A` @ `0x5FFEE` (512 KiB), `EY-023B` @ `0x71FEE` (fx880btg) |

## 2. Changes

### `src/model.rs`
* `BoardKind::ClassWizII` no longer aliases ClassWiz: added
  `CLASSWIZ2_RAM_BASE/SIZE` (`0x9000`/`0x6000`), `CLASSWIZ2_ROM_MIRROR_BASE`
  (`0x80000`), `CLASSWIZ2_VRAM2_BASE` (`0x89000`), `CLASSWIZ2_VRAM_SIZE`
  (`0x800`).
* `Model::execution_supported()` now returns `true` for every model.

### `src/machine.rs`
* `Machine::ram_base/ram_size/rom_mirror_base` dispatch on `ClassWizII`.
* `Machine::cw2_code_addr()` implements the CW ii code-segment decode above;
  `Bus::read_code` uses it (CSR masked with `csr_mask`).
* `cw2_data_rom_addr()` implements the data ROM-window decode; `read_data_byte`
  and `write_data_byte` route segment 0 ROM, RAM, SFR page and VRAM2. ROM
  windows stay read-only.
* New `CwiiState` holds both 4 KiB framebuffers, screen control, interrupt
  mask/pending/active, timer 0, standby control (HALT/STOP), DSR SFR, keyboard
  EXICON and BCD registers. The SFR accessors implement the
  mask/clear-on-write behaviour of the reference (`0xF010..0xF017`,
  `0xF020..0xF025`, `0xF030..0xF032`, `0xF037`, `0xF400..0xF4EB`).
* `render()` blends both CW ii framebuffers (pixel OR, equivalent to showing
  everything drawn) with the same 32-byte/24-byte geometry as ClassWiz.
* `Cpu::raise()`-based maskable interrupt delivery: lowest pending enabled
  source wins, EL must be 0/1, MIE uses the 3-instruction-delayed PSW snapshot;
  ELR/ECSR/EPSW are saved at EL1 and MIE is cleared, PC is vectored through
  `index*2` in CSR 0. The keyboard press sets the EXI0 pending bit once.
* Timer 0 runs from HSCLK (F024 bit 3) or the 256-cycle LSCLK, with the
  `2^n` prescaler; SBYCON HALT/STOP halt the core until an interrupt is pending.
  This is required because the firmware disables IRQs and STOPs between frames.
* Checksum includes the CW ii framebuffers.

### `src/cpu.rs`
* Added `Cpu::raise()` (the nX-U8 exception entry described above) and the
  delayed `psw_backup[3]` used by the CW ii interrupt controller. ES PLUS and
  ClassWiz never call these paths; all existing instruction semantics are
  unchanged.

### Tests
New ROM-free tests in `src/machine.rs`: ROM padding/size, data-window map,
code-segment decode (including mirrors and unmapped segments), dual-VRAM
rendering and bank switching, SFR/mask/timer behaviour, keyboard KI/KO gating
and the DSR SFR. `model.rs`' CW ii test now asserts execution support.

## 3. Validation

Commands run in `/tmp/fix-cw2` with the read-only ROMs:

```
cargo build --release                 # OK, no warnings
cargo test                            # 32 passed, 0 failed, 1 ignored
FX991_ROM=/home/devlapi/Downloads/fx991es-rs-linux-x86_64/fx991es.rom.bin \
  cargo test --release -- --ignored   # 1 passed (C-model accuracy, no regression)

./target/release/casio-emu-rs --rom roms/<model>.rom.bin --model <model> \
  --headless 6000000 --press 2000000:1 --dump /tmp/lcd.pbm
```

| ROM | Final PC/CSR | Lit pixels with key | Lit pixels without key | Pixels changed |
|---|---|---|---|---|
| `fx991cw` (512 KiB) | `6062`/`00` | 3535 | 128 | 3545 |
| `fx9910cw` (512 KiB) | `680C`/`00` | 0 | 0 | 0 — **not booting** |
| `fx991cncw` (1 MiB) | `612C`/`00` | 3515 | 128 | 3591 |
| `fx880btg` (1 MiB) | `6E00`/`00` | 3535 | 128 | 3545 |

For the task's exact command (`fx991cncw`) `/tmp/lcd.pbm` contains 3515 lit
pixels and differs from the no-key run in 3591 pixels. The rendered image is
the calculator's Chinese main menu (計算 / 統計 / 函数表格 / 方程 / 不等式 /
复数); without a key press the board idles on the `CASIO — Boost your
Curiosity` splash. Pressing `1` at 2 M instructions is observed by the firmware
as KI `0xFE` on KO `0x01` and wakes it from STOP.

## 4. Why keys work at 2 M but not earlier/later versions of the code path

The CW ii firmware enters STOP between frames with MIE cleared. We mirror the
reference `Chipset::Tick`: peripherals keep ticking while the core is stopped
and any pending interrupt resumes it; the actual exception is only taken when
the (delayed) MIE and the interrupt mask permit it. A key press therefore both
sets the pending EXI0 bit and, on real-hardware ROMs, is picked up by the
firmware's KO scan (`KI=0xF040`), which dismisses the splash.

## 5. Remaining gaps

* **`fx9910cw` (2nd edition, `FY-505A`) does not boot.** Tracing its 512 KiB
  image shows a deliberate software reset around instruction 23 that pops
  `PC=0x67F6`, `CSR=0xF6` from the reset-vector words at `0x000C/0x000E`.
  With `csr_mask=0x0F` this becomes code segment 6, which the reference
  `MMU::ReadCode` explicitly leaves unmapped (`0xFFFF`), so the CPU falls into
  an EL2 branch-to-self at `0x680C` (`0xCEFF`) and the LCD stays blank. Mapping
  segment 6 lets it run further but still produces no display. This ROM is
  not among the `models/` shipped with CasioEmuX, so its exact flash/segment
  mapping is unknown; it needs a dedicated model descriptor or a different
  `csr_mask`/remap rule. All other CW ii behaviour in this build is shared
  with the three working models, so `execution_supported()` stays `true` and
  this limitation is documented rather than hidden.
* **BCD coprocessor** (`src/bcd.rs`) implements the command unit
  (`RunCommand`: add/sub/shift/copy/clear on the four 12-byte registers, flags,
  leading-zero counters) and the 32-word macro sequencer used for multiply,
  divide and shifts, ported from CasioEmuX `BCDCalc.cpp`. A command queued
  behind a running macro stalls the CPU until it is accepted. Before this was
  added every calculation on CW ii ROMs returned 0.
* **fx-810 DE CW** boots to its home menu but does not react to keys: the
  firmware leaves KO undriven and the input filter (`0xF042`) at 0 before STOP,
  so the one-shot keyboard interrupt this emulator raises is discarded when the
  wake-up code clears `0xF014/15`. Raising it level-sensitively livelocks in the
  ISR, so the root cause is still open.
* **Timing is approximate.** One `tick()` is one CPU instruction and one HSCLK
  tick; FCON's `ClockDiv`, the real 4 MHz clock ratio, instruction cycle counts
  and the watchdog/RTC peripherals are not modelled. The battery-level
  detector (`0xF0D0..0xF0D2`, `PowerSupply.cpp`) **is** modelled: BLDFlag is
  initialised healthy, writes to `0xF0D1` start a short test that always
  succeeds, and the solar indicator at `0xF310` reports a panel on fx-991CW /
  fx-991CN CW only. Without this the firmware displays a dead-battery warning
  and never leaves the splash screen. LSCLK is
  approximated as one tick per 256 instructions. SPLASH/screen transitions are
  therefore instruction-count-accurate only for this build.
* **Second framebuffer**: `0x89000` is routed to VRAM2 for all CW ii models.
  CasioEmuX only maps it on `real_hardware == 0` images; real hardware uses the
  `0xF037` bank switch (which we also implement). Blending is a pixel OR rather
  than the 30/70 intensity mix.
* **Status icons** are not decoded from the two status-row bytes; the menu/dot
  matrix rows are rendered. GUI hit-testing still uses the older key grid;
  headless key labels use `keys::pc_key`.
* Emulator-derived CW ii images (mailbox at `0x88E00`, extra 256 bytes at
  `0x89800`) are not specially supported; our 0x89000 window always maps VRAM2.

## 5b. fx-9910CW — dump is unusable (definitive)

`roms/fx9910cw.rom.bin` (FY-505A, 512 KiB, SHA-256
`e238c34f6023fcac167cab0482bf5eb2223ac4ba54bff958ee5733cccb9eb887`) cannot be
booted by any emulation change: **two of its flash banks contain no data at
all**.

Traced boot path with the unmodified dump:

| step | PC | event |
|---|---|---|
| 0 | `0x680A` | reset vector |
| 1–17 | `0x680E…0x685C` | table-copy loop reads an 8-byte config entry at data `4:0B8A0` (ROM `0x4B8A0`) — all eight bytes are `00`, end marker `FF FF` at `0x4B8A8`; result `R6=0`, `R7=0` |
| 18 | `0xB5E2` | `CMP R6,#2` fails (`R6=0`) → `0xB576` |
| 26 | `0xB576` | `CMP R7,#1` fails (`R7=0`) → `0xB5A4` → `B 0:822A` → BRK → EL2 vector `0x680C` → `BC AL,self` |

With a temporary diagnostic ROM that supplied `R6=2, R7=1`, boot proceeds to
`0xB57E`: `BL 1:08F0A` — a far call into code segment 1, i.e. ROM `0x18F0A`.
Bank 1 (`0x10000–0x1FFFF`) is **65,536 consecutive zero bytes**, so the CPU
executes NOPs forever; after 5 M steps it is still marching through the empty
bank with 0 lit pixels.

Per-bank nonzero byte counts prove the dump is incomplete:

| bank | 0 | 1 | 2 | 3 | 4 | 5 | 6 | 7 |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| fx9910cw | 55,336 | **0** | **0** | 60,004 | 54,124 | 15 | 0 | 8,189 |

Working CW ii dumps (fx991cw, fx810decw, fx880btg, fxjp700cw, fx82decw) have
~60 K nonzero bytes in banks 1 and 2. `fx810decw` has the identical boot
structure and contains valid code at `0x18F0A`; this dump does not. It also
lacks the `71CASGaYqgys` security ID + password at `0xFDE0` that every other
CW ii ROM has, and its stored checksum field is `0xF8D4` (not `0x8113`, which
belongs to fx-880BTG).

**Conclusion:** this is a failed flash read, not a model the emulator can
support as-is. No semantic emulation change was kept: a diagnostic experiment
masking DSR to `0x1F` (as CasioEmuX does) left all CW ii models' LCD output
unchanged and was reverted to avoid regressions. A complete, verified 512 KiB
FY-505A dump — with real data in banks 1/2, valid code at `0x18F0A`, the
security ID at `0xFDE0`, and a matching checksum — is required. Re-dump the
calculator with DTU8 and verify bank-1/2 nonzero counts first.

## 6. References

* Xyzstk/CasioEmuX (GPL-3.0), fetched from `stable`: `models/fx991cncw/model.lua`,
  `emulator/src/Chipset/MMU.cpp`, `Chipset.cpp`, `CPU.cpp`, `InterruptSource.cpp`,
  `Peripheral/*.cpp`, `Data/HardwareId.hpp`.
* Physics365 `991CN-X-CW-Decompilation` (nX-U8 confirmation, ROM header).
* Local notes: `docs/MODELS-991CNCW.md`, `docs/MODELS-880BTG.md`.
