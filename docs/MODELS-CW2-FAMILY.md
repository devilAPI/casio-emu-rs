# ClassWiz II (HW_CLASSWIZ_II) archive family

Ten ClassWiz II models from `roms/models/models/<folder>/rom.bin` were added
to the model registry.  Each is hardware id 5 (`BoardKind::ClassWizII`), uses
the CW ii board already implemented in `src/machine.rs` (24 KiB RAM at
`0x9000`, dual 192x64 framebuffers, battery detector, timer/IRQ) and reuses
`keys::VNX_KEYS`.  **No board/machine code was changed** — these are
descriptor-only additions.

`model.lua` was used as the authority for `model_name`, `hardware_id`,
`real_hardware`, `csr_mask` and `rom_path` wherever present (all eight folders
with a `model.lua` report `hardware_id = 5`, `real_hardware = 1`,
`csr_mask = 0x000F` and the same 5x4 + 6x2 key-code map).  Three folders have
no `model.lua` (`fx-810 DE CW`, `fx991cw_emu`, `graphlight_emu`); for those the
folder name/product name and the `EY-xxx` version record in the ROM are used.

## Models

| id | model name | ROM size | signature / offset | source folder | keyboard interface |
|---|---|---:|---|---|---|
| `fx810decw` | Casio fx-810 DE CW | 512 KiB | `EY-047A` @ `0x071FEE` | `fx-810 DE CW` | hardware KI `0xF040` / KO `0xF046,0xF047` (SFR) |
| `fx8200au` | Casio fx-8200AU | 512 KiB | `EY-044A` @ `0x071FEE` (also `0x05FFEE`) | `fx8200AU` | hardware KI/KO SFR |
| `fx82decw` | Casio fx-82DE CW | 512 KiB | `EY-012A` @ `0x071FEE` (also `0x05FFEE`) | `fx82decw` | hardware KI/KO SFR |
| `fx880btg-alt` | Casio fx-880BTG (archive dump) | 1 MiB | `EY-023B` @ `0x071FEE` (also `0x05FFEE`) | `fx880btg` | hardware KI/KO SFR |
| `fx92bs` | Casio fx-92B Secondaire | 1 MiB | `EY-007A` @ `0x071FEE` (also `0x05FFEE`) | `fx92bs` | hardware KI/KO SFR |
| `fx991cncw-alt` | Casio fx-991CN CW (archive dump) | 1 MiB | `EY-027A` @ `0x05FFEE` (also `0x071FEE`) | `fx991cncw` | hardware KI/KO SFR |
| `fx991cw-emu` | Casio fx-991CW (emulator ROM) | 1 MiB | `EY-005A` @ `0x071FEE` | `fx991cw_emu` | hardware KI/KO SFR (emulator mailbox not modelled) |
| `fx999cncw` | Casio fx-999CN CW | 1 MiB | `EY-028A` @ `0x071FEE` (also `0x05FFEE`) | `fx999cncw` | hardware KI/KO SFR |
| `fxjp700cw` | Casio fx-JP700 CW | 512 KiB | `EY-030A` @ `0x071FEE` (also `0x05FFEE`) | `fxjp700cw` | hardware KI/KO SFR |
| `graphlight-emu` | Casio Graph Light (emulator ROM) | 512 KiB | `EY-090A` @ `0x071FEE` | `graphlight_emu` | hardware KI/KO SFR (emulator mailbox not modelled) |

All descriptors use `lcd_width = 192`, `lcd_height = 64`, `vram_base = 0xF800`,
`csr_mask = 0x0F`, `key_mailbox = 0`, `keys = keys::VNX_KEYS` and
`keyboard = KeyboardKind::HardwareSfr`.  CPU execution remains enabled via
`execution_supported() == true`.

## Deduplication and signature collisions

ROMs were deduplicated by SHA-256 of the dumped `rom.bin` files:

- `fxjp700cw/romjp7cw.bin` is byte-identical to `fxjp700cw/rom.bin`
  (`ef12c0ff…e4cce9cd05`); only one `fxjp700cw.rom.bin` was copied.
- `fx991cncw/rom.bin` (`55ec7507…480aaf`) is **not** byte-identical to the
  already registered `fx991cncw.rom.bin` (`4ab47916…3471f`).  It is therefore
  a separate descriptor, `fx991cncw-alt`.  Both images contain `71CASG` at
  `0x8FDE0`, but only the archive dump has `EY-027A` at `0x5FFEE`; using that
  offset lets `detect()` distinguish them without changing the existing
  descriptor.
- `fx880btg/rom.bin` (`2fa677a4…670ac7c7`) is **not** byte-identical to the
  already registered `fx880btg.rom.bin` (`39dd1984…c9eb887`).  Both carry the
  identical `EY-023B` version/checksum record at `0x071FEE`, so signature
  detection cannot separate them.  `detect()` deliberately resolves to the
  first-registered `fx880btg`; load the archive dump explicitly with
  `--model fx880btg-alt` (file `roms/fx880btg-alt.rom.bin`).
- `fx991cw_emu/rom.bin` (`ded1d608…31c1`) is a 1 MiB `EY-005A` image, distinct
  from the 512 KiB `fx991cw` dump (`4da20976…69352bb`, `EY-005` at `0x5FFEE`).
  `fx991cw-emu` is registered after `fx991cw`, so the existing model keeps
  auto-detection.

No previously registered model (`fx991cw`, `fx9910cw`, `fx991cncw`,
`fx880btg`) was modified.  The version record is stored twice in most CW ii
images (`0x05FFEE` and `0x071FEE`, plus `0x0F1FEE` in 1 MiB images); the
descriptor records one canonical offset and tests check that offset.

## ROM files

ROM dumps were copied (not committed; `roms/` is gitignored) into the main
repository as `roms/<id>.rom.bin` for each id in the table above.  ROM images
remain copyrighted by CASIO COMPUTER CO., LTD; use only dumps you are licensed
to use.

## Verification

- ROM-free unit tests: signature detection at the documented offsets, ROM-size
  validation (truncated images are not detected), descriptor parameters,
  uniqueness of ids, and explicit-model fallback for the two collisions.
- `cargo build --release --workspace` — pass.
- `cargo test` — 62 passed, 0 failed (1 pre-existing ignored long ROM test).
- `cargo clippy` — pass (two pre-existing style warnings in `src/gui.rs`,
  unrelated to these descriptors).
- `cargo fmt --all` — applied; clean.
- `FX991_ROM=…/fx991es.rom.bin cargo test --release -- --ignored` — pass
  (C-model checkpoints unchanged).

The desktop model dropdown was made height-aware (`src/gui.rs`): row height
shrinks on short windows so all registry entries remain visible/clickable.
This is presentation-only; no keyboard, LCD or machine behaviour changed.
