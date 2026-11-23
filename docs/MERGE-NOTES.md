# Merge notes: ES PLUS + ClassWiz II + ClassWiz

This copy combines three independent fixes, all of which remain active:

| Source | What it contributes |
|---|---|
| main repo baseline | ES PLUS real-hardware keyboard/timer/IRQ/STOP model (`RunMode`), `Machine::load_ram()` + 4 RAM tests, all 16 model descriptors (incl. `fx880btg`), and the validated C-model accuracy path. |
| [`FIX-CW2.md`](FIX-CW2.md) | CW ii board: `CwiiState`, 24 KiB RAM at `0x9000`, 512 KiB/1 MiB ROM windows, dual framebuffers (`0xF800` + `0x89000`), CW ii SFR/timer/IRQ controller, `Cpu::raise()` with delayed MIE, 7 ROM-free tests. |
| [`FIX-CLASSWIZ.md`](FIX-CLASSWIZ.md) | ClassWiz (HW id 4) board: full 64 KiB code segment 0, timer/IRQ/standby SFRs, screen-mode-aware rendering, `Cpu::accept_maskable_interrupt()`, `Model::rom_filename()`, `--release STEP`, 6 ROM-free tests + 2 CPU tests. |

## How the three coexist

* **Boards are selected by `model.board`, not by keyboard kind.** `Machine::tick()`
  has three disjoint paths: ClassWiz II (`cw2_tick()` → `cpu.raise()`), ClassWiz
  (`cw_timer_tick()` → `cpu.accept_maskable_interrupt()`), and the original ES
  PLUS path (`tick_peripherals()` → `cpu.raise_maskable()` + `RunMode`).
  Emulator-mailbox models keep the untouched legacy path.
* **All three CPU interrupt helpers are kept.** `raise_maskable()` (ES PLUS,
  no EPSW save), `raise()` (CW ii, saves EPSW at a chosen exception level) and
  `accept_maskable_interrupt()` (ClassWiz, EL-checked, returns `bool`).
  The CW ii-only `psw_backup[3]` delayed-MIE snapshot is updated in
  `Cpu::step()` and read only by `master_interrupt_enable()`; ES PLUS and
  ClassWiz never consult it, so their instruction/accuracy behaviour is
  unchanged.
* **State is separated.** ES PLUS keeps `run_mode: RunMode`, `stpacp_last`,
  `stop_acceptor_enabled`, `timer_raise` and `has_input`. ClassWiz uses the
  `cw_timer_*`, `cw_standby`, `cw_stop_acceptor` and `cw_key_irq_raised` fields.
  CW ii owns `cw: CwiiState` (including its own `halted`/`stopped` flags) and
  `cw_key_irq_pending`. No board can observe another board's state.
* **SFR names do not clash.** The ES PLUS IRQ/timer constants keep their
  original names (`INT_MASK`, `INT_PENDING`, `IRQ_KEYBOARD = 5`,
  `IRQ_TIMER = 9`, `TIMER_*`, `PD_PORT`, `KO_BITS_MASK`, …). The ClassWiz
  registers at the same addresses use `CW_`-prefixed constants, because
  ClassWiz encodes source `n` in bit `n+1` while ES PLUS uses bit `n-4`.
  CW ii uses its own 32-bit masks in `CwiiState`.
* **Memory maps are model-gated.** `ram_base()`, `ram_size()` and
  `rom_mirror_base()` dispatch on all three boards. Code segment 0 now reads
  the whole 64 KiB ROM page on ClassWiz (root cause of the blank display),
  while CW ii code fetches go through `cw2_code_addr()` and ES PLUS keeps its
  two 64 KiB pages. `read_data_byte()`/`write_data_byte()` route CW ii VRAM2,
  SFRs, RAM and ROM windows before the older boards' branches.
* **Compatibility features survive the merge.** `--ram` still loads a raw RAM
  snapshot through `Machine::load_ram()` in the CLI and GUI, `Model::rom_filename()`
  only changes the auto-discovery filename (fx-82EX uses `fx82ex-vere.rom.bin`),
  and `--release STEP` is additive. All 16 descriptors remain, CW ii execution
  gating is gone (`execution_supported()` always returns `true`), and the ES
  PLUS C 6M-instruction checkpoint is unchanged.

## Validation summary

* `cargo build --release --workspace`, `cargo test` (51 passed, 1 ignored),
  `cargo clippy --workspace --all-targets` (no warnings),
  `cargo fmt --all`, and
  `FX991_ROM=… cargo test --release -- --ignored` (ES PLUS C checkpoint) all pass.
* ES PLUS `fx570es-plus` with `--press 2000000:1`: 32 lit pixels, digit `1` drawn.
* ClassWiz `fx570vnx/fx580vnx/fx991ex/fx82ex/fx991dex-verf/fx991cnx`: 41 lit
  pixels at boot, 71 after key `1` (108 pixels change).
* CW ii `fx991cw/fx991cncw/fx880btg`: 128 lit splash pixels at boot, 3515–3535
  after key `1` (3545–3591 pixels change).

Known limitations are the ones documented in the two fix notes: `fx9910cw`
and `fx991dex-vere` still do not render (different firmware revisions), and
the CW ii BCD coprocessor is register-only.
