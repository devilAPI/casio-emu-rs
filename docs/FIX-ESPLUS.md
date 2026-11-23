# Fix: key input on real-hardware ES PLUS ROMs

## Scope

`fx570es-plus` and `fx82esplusa` (128 KiB real-hardware ROMs) boot and draw
the LCD, but key presses did nothing.  The fx-991ES PLUS **C** mailbox model
is untouched and remains the accuracy reference.

## Investigation

Instrumentation was done outside the repository: a temporary Rust harness in
`/tmp/esinspect` wrapped the `Bus` trait to log every SFR access and every PC
change, and a temporary `ESPLUS_IRQ_DEBUG` print in `Machine::raise_irq`
recorded each IRQ (both removed again).  The ROM was copied to `/tmp` only;
no ROM bytes were added to the repo.

Observed during boot of `fx570es-plus` (SFR trace, instruction numbers):

| Step | SFR write | Meaning |
|---|---|---|
| 10,065–10,066 | `F044=0x80`, `F045=0xFF` | KO mask: KO7..9 reserved, KO0..6 enabled (10-bit register) |
| 10,071 | `F042=0x00` | input filter cleared |
| 38,583–38,745 | `F044/F045/F046/F047` writes + `F040` reads | model-ID (Pd) scan of KO0..9 via KI |
| ~1.9 M | `F010=0x0022` | IRQ mask enables **source 5 (keyboard)** and **source 9 (timer)** |
| 2,000,106 | `F020=0x129A`, `F022=0`, `F025=1` | timer interval 4762, counter cleared, timer started |
| 2,000,120 | `F014=0`, `F015=0` | pending IRQs acknowledged |
| 2,000,125 | `F008=0x50`, `F008=0xA0`, `F009=0x02` | STPACP sequence arms SBYCON and enters **STOP** |
| 2,000,134 | read `F014` | firmware polls the pending register |

With the original core the pending register stayed `0x0000` forever: the
machine had no timer counter, no IRQ source, no standby state and no CPU
interrupt delivery.  It therefore neither woke from STOP nor saw a key, and
just spun in the display-refresh loop `PC 0xB3A0..0xB400`
(`PC=0xB3C0`, `PSW=0xC4`, 6,000,323 executed instructions after 6 M ticks;
LCD had 9 lit pixels, none of them a digit).

Reference behaviour was cross-checked against `user202729/CasioEmu`:
`Peripheral/Keyboard.cpp` (IRQ source 5, `has_input` gated by `F042` at key
press, 10-bit KO mask, KO7..9 Pd sense lines), `Peripheral/Timer.cpp`
(interval/counter/control at `F020/F022/F025`, prescaler
`131072/10000` cycles, IRQ source 9), `Peripheral/StandbyControl.cpp`
(STOP/halt), `Chipset/Chipset.cpp` (mask/pending SFR bits `source-4`, one
interrupt accepted per tick, `Raise()` on MIE) and `Chipset/CPU.cpp::Raise`
(save ELR/ECSR at EL1, clear MIE, vector = `ReadCode(source*2)`).

## Fix

All new behaviour is gated on `Model::keyboard == KeyboardKind::HardwareSfr`;
the mailbox firmware path in `Machine::tick` is byte-for-byte unchanged.

* `src/model.rs` — SFR/IRQ constants (`F008/F009`, `F010/F014`, `F020/F022/
  F025`, `F042`, `F044..F047`, `F050`), 10-bit KO mask, 13 managed interrupt
  bits, CPU/timer clock rates (131072 Hz / 10000 Hz).
* `src/cpu.rs` — `Cpu::raise_maskable()`: save ELR/ECSR at level 1, clear MIE,
  enter EL1, vector through code segment 0 at `source*2` (matching
  `CPU::Raise`; EPSW is not saved, as in the reference).
* `src/machine.rs`:
  * IRQ mask/pending as 13-bit SFR fields; `raise_irq()` latches the pending
    bit, resumes a halted/stopped core and calls `raise_maskable()` when MIE
    is set.
  * Keyboard IRQ: `press()` latches `has_input` only if the input filter
    (`F042`) passes the KI bit at the moment of the press; `release()` clears
    it; `Tick()` retries source 5 while held.
  * Timer: interval with forced minimum 1, write-any-byte counter clear,
    run bit, prescaler and source 9 retry latch.
  * Standby: `F008 = 0x50,0xA0` then `F009.1` freezes the CPU until an
    interrupt is accepted (`F009.0` is HALT); peripherals keep ticking.
  * KO mask/drive are masked to 10 bits; KO7..9 never drive a key; `F050`
    (Pd) reads as 0 and ignores writes on real hardware.
* Tests — 7 new ROM-free unit tests in `src/machine.rs` cover keyboard IRQ
  filtering and vectoring, masked/MIE-disabled IRQs, timer countering and
  vectoring, timer SFR write semantics, KO/Pd behaviour, STOP resumption and
  the mailbox model's immunity to all of the above.

## Measured results

Command used (ROM in `/tmp` only, absolute paths):

```bash
cargo build --release
./target/release/casio-emu-rs --rom /tmp/fx570es-plus.rom.bin \
  --model fx570es-plus --headless 6000000 --press 2000000:1 --dump /tmp/lcd.pbm
```

| | before | after |
|---|---|---|
| final PC | `0xB3C0` (display-refresh loop) | `0x4680` (key-processing path) |
| PSW | `0xC4` | `0x00` |
| executed instructions (of 6 M ticks) | 6,000,323 | 2,166,084 (rest spent in STOP between IRQs) |
| `F010` IRQ mask | `0x0022` | `0x0022` |
| `F014` pending | `0x0000` | `0x0002` after key press |
| LCD lit pixels | 9 (status/icons only, no digit) | 32 (digit `1` at x=2..5, y=2..10, input cursor at x=5..6) |
| key response | none | `1` appears; `2` also renders correctly |

Debug trace after the fix: at headless tick 2,000,000 the key press latches
pending bit 1 (`0x0002`) and wakes the CPU from STOP at instruction ~62,484;
the firmware then draws the digit.  `fx82esplusa` behaves the same way:
after the fix `PC=0xA9E0`, `PSW=0x20`, the same 32-pixel `1` is displayed.

## Regression validation

* `cargo build --release` — OK.
* `cargo test` — 32 passed, 0 failed, 1 ignored.
* `FX991_ROM=/home/devlapi/Downloads/fx991es-rs-linux-x86_64/fx991es.rom.bin
  cargo test --release -- --ignored` — key checkpoint passed (`PC=0x538a`,
  32 LCD pixels).
* `FX991_ROM=<C ROM> cargo test --release` — boot checkpoint passed
  (`PC=0xDAA6`, `CSR=1`, `PSW=0`).
* Differential accuracy harness (`U8EMU_DIR=/tmp/u8_emu`, C ROM, 10 M
  instructions): **10,000/10,000 hash checkpoints identical**; key sweep
  **49/49 keys identical**; `PASS`.

## Remaining gaps

* Keyboard ghosting for multiple simultaneous keys is still approximated by
  the single held `(KI, KO)` pair; the reference propagates ghosts through
  connected KO columns.
* The Pd/KO7..9 sense lines are modelled only as "not connected" for real
  hardware (KI7 stays high); no model exposes a nonzero `F050` value here.
* The ClassWiz ROMs share the hardware-SFR gate and therefore gain IRQ/timer
  support, but their screen sprites/status bar and larger keypad are still
  not accurate; ClassWiz II remains execution-unsupported.
* Sound SFRs (`F008/F009` are standby here), contrast and other unknown SFRs
  are not characterised; the differential harness only covers the C ROM.
