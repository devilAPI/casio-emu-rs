# fx-82ES PLUS A / fx-82ES PLUS (emulator) model notes

This document records the per-model configuration used by `src/model.rs` for
the two low-end ES PLUS machines and how it was derived.  ROM images are
**Casio copyrighted**: they are never stored in this repository.  The hashes
and URLs below are provided so a user can identify a dump they already own.
For testing, the images were fetched only into `/tmp` and verified by hash.

## 1. Supported models

| CLI id | Hardware | ROM origin | Signature @ 0x1FFF4 | Keyboard |
|---|---|---|---|---|
| `fx82esplusa` | fx-82ES PLUS A | real calculator dump | `LY713XA` | hardware KI/KO SFRs |
| `fx82esp-emu` | fx-82ES PLUS | Casio `fx-ES PLUS Emulator` dump | `GY450XE` | emulator mailbox |

Both images are exactly **128 KiB (0x20000)**, i.e. two 64 KiB code pages,
and use the same nX-U8/100 core as the already-supported fx-991ES PLUS C.
The model descriptor stores the six-byte signature prefix (`LY713X`,
`GY450X`), so `model::detect()` recognises the full string at 0x1FFF4.

An explicit `--model <id>` always overrides detection.  `--list-models`
prints all four configured models.

## 2. ROM sources and hashes

### fx-82ES PLUS A (real hardware)

* URL: <https://raw.githubusercontent.com/user202729/fxesplus/master/82espa/rom.bin>
* Git blob SHA-1: `881238105f36f77741c754b023d8b85b081f92f4`
* SHA-256: `ab58e84c8ea265b81fb9eda9a590ee0e3c35f62e3f07b35c7431fd0ad03bf384`
* Size: 131072 bytes; bytes at 0x1FFF4..0x1FFFA = `4C 59 37 31 33 58` (`LY713X`),
  followed by `41` (`A`) and `20`.

### fx-82ES PLUS (emulator ROM)

* URL: <https://raw.githubusercontent.com/user202729/fxesplus/master/82esp/emu_rom.bin>
* Git blob SHA-1: `edcda212a1b0f32f3b2776bf12519e3b5e90c756`
* SHA-256: `a1458a72fbdd9452b31cd791605e087c3a52191a4b9955d1a34c2c71aaeae35d`
* Size: 131072 bytes; bytes at 0x1FFF4..0x1FFFA = `47 59 34 35 30 58` (`GY450X`),
  followed by `45` (`E`) and `20`.

> Note: the `user202729/CasioEmu` model `fx82es_emu/readme.txt` lists a
> different SHA-256 (`05d0babf…f8f3c`) for the ROM it expects.  That is a
> separate dump of the older **fx-82ES** emulator, not the fx-82ES **PLUS**
> `emu_rom.bin` referenced above.  The board parameters
> (`hardware_id = 3`, `real_hardware = 0`, `pd_value = 0x08`, `csr_mask = 1`)
> are identical for both, so the same descriptor applies.

## 3. Board parameters

| Parameter | Both models | Source |
|---|---|---|
| ROM size | `0x20000` | 128 KiB image; CasioEmu maps segments 0, 1 and 8 |
| Code segment mask | `csr_mask = 0x01` | `model.lua`: `csr_mask = 0x0001` |
| RAM | `0x8000..0x8DFF` (0xE00 bytes) | `BatteryBackedRAM.cpp`, `HW_ES_PLUS` |
| VRAM | `0xF800`, 32 rows × 16 bytes = 0x200 | `Screen.cpp`, `N_ROW = 31` + status row, `ROW_SIZE = 16`, `ROW_SIZE_DISP = 12` |
| LCD | 96 × 32 pixels (incl. status row) | 12 display bytes/row × 8 pixels |
| KO mask / drive | `0xF044/0xF045`, `0xF046/0xF047` (10-bit, 0x03FF) | `Keyboard.cpp` |
| KI input | `0xF040`, active low | `Keyboard.cpp` |
| Emulator mailbox | `0x8E00` ready, `0x8E01` KI, `0x8E02` KO | `Keyboard.cpp`, `HW_ES_PLUS` → offset 0 |
| Pd (model ID) | `0xF050`, value `0x08` on `fx82esp-emu` | `Keyboard.cpp`, `model.lua` `pd_value` |
| Extra emulator RAM | 0x100 bytes at `0x9800` | `BatteryBackedRAM.cpp`, `!real_hardware` (not modelled, see §6) |

`fx82esplusa` has `KeyboardKind::HardwareSfr`: `Machine` samples the held
key through `0xF040`, gated by the driven KO bits `0xF046/F047` and the KO
mask `0xF044/F045` (active-low, with ghosting semantics approximated by the
simple AND gate in `machine.rs`).

`fx82esp-emu` has `KeyboardKind::EmulatorMailbox`: `Machine::tick()` watches
the ready flag at `0x8E00` and publishes the queued `(KI, KO)` pair at
`0x8E01/0x8E02`, exactly like the fx-991ES PLUS C descriptor.

## 4. Physical key matrix: identical on all ES PLUS models

The reference Lua button maps (`models/fx82esplusa/model.lua`,
`models/fx82es_emu/model.lua`, `models/fx570esplus/model.lua`,
`models/fx991esplus_emu/model.lua` in `user202729/CasioEmu`, mirrored by
`Xyzstk/CasioEmuX`) use the same 50 button codes.  `Keyboard.cpp` decodes
them as `KO bit = 1 << (code >> 4)`, `KI bit = 1 << (code & 0xF)`;
`code == 0xFF` is the ON key and is not part of the scanned matrix.  In other
words **49 matrix positions + ON**, with KI bits 0..7 and KO bits 0..6:

| KO bit (KI → label) | 0 (`0x01`) | 1 (`0x02`) | 2 (`0x04`) | 3 (`0x08`) | 4 (`0x10`) | 5 (`0x20`) | 6 (`0x40`) |
|---|---|---|---|---|---|---|---|
| KI 0 (`0x01`) | 1 | 2 | 3 | + | − | — | = |
| KI 1 (`0x02`) | 4 | 5 | 6 | × | ÷ | — | Ans |
| KI 2 (`0x04`) | 7 | 8 | 9 | DEL | AC | — | ×10^x |
| KI 3 (`0x08`) | RCL | ENG | ( | ) | S↔D | M+ | . |
| KI 4 (`0x10`) | (−) | °′″ | hyp | sin | cos | tan | 0 |
| KI 5 (`0x20`) | a b/c | √ | x² | xⁿ | log | ln | — |
| KI 6 (`0x40`) | Abs | x³ | ◀ | ▼ | x⁻¹ | logₐ | — |
| KI 7 (`0x80`) | SHIFT | ALPHA | ▲ | ▶ | MODE | — | — |

Equivalent key codes from the Lua tables (low nibble = KI index, high
nibble = KO index): numeric block `0x00/0x10/0x20/0x30/0x40`, `0x01/…/0x41`,
`0x02/…/0x42`, `0x60/0x61/0x62/0x63/0x64`; function block `0x03..0x55`;
arrows `0x26`, `0x36`, `0x27`, `0x37`; SHIFT `0x07`, ALPHA `0x17`, MODE
`0x47`; ON `0xFF`.  These are byte-for-byte the same as `src/keys.rs`, so
both new descriptors reuse `keys::KEYS` and all PC-key mappings keep working.
KO bits 7, 8 and 9 exist in the silicon for model-ID (Pd) resistors and are
masked with `ko_mask`; no calculator key uses them.

### What actually differs on the fx-82ES PLUS

*No electrical difference.*  The matrix, KI/KO codes, ON reset behaviour and
hitboxes are shared.  The differences are firmware/legend level:

* The fx-82ES PLUS A has no MATRIX, VECTOR, STAT, EQN or BASE-N modes; its
  primary key legends (digits, arithmetic, sin/cos/tan, log/ln, √, x², xⁿ,
  a b/c, RCL, ENG, M+, etc.) are the same, but the yellow SHIFT and red ALPHA
  legends behind those keys differ from the fx-991ES PLUS C.  For example the
  fx-991 keymap table (`user202729/fxesplus/keymap/table`) exposes `STAT`,
  `CMPLX`, `MATRIX`, `VECTOR`, `CONST`, `CONV` and `RCL-A…RCL-M` entries that
  are absent on the low-end model.
* MODE menu contents differ (COMP/CMPLX/… subset).  The descriptor does not
  model menus; the firmware decides what each key does from its own lookup
  table.
* The status-row sprite set is the same hardware capability (`rsd_stat`,
  `rsd_mat`, `rsd_vct`, … are declared even in the fx-82ES PLUS A Lua model),
  but the low-end firmware never enables the MATRIX/VECTOR indicators.
* The GUI skin and ink colour differ between models; no 82ES skin is shipped
  here (see §6).

## 5. Implementation and tests

* `src/model.rs`: `FX82ES_PLUS_A` and `FX82ESP_EMU` added to `ALL`;
  `detect()` recognises both signatures; `by_id()` is case-insensitive.
* `src/machine.rs`: `Machine::with_model()` already handles the keyboard
  split; the new models inherit the shared RAM/SFR/VRAM map.
* `src/gui.rs`: the GUI now runs the selected `Model` (window title, keypad
  hitboxes and drawing), fixing the parameterisation gap in the CLI path.
* `src/main.rs`: `--model` / `--list-models` documented in `--help`.
* ROM-free unit tests cover signature detection, distinct signatures, model
  lookup, the 49+ON matrix identity, key-label coverage, keyboard kind,
  ROM/RAM/SFR/VRAM mapping, VRAM rendering and short-ROM padding.

Build/test results: `cargo build --release` and `cargo test` pass on Linux
(25 passed, 1 intentionally ignored ROM-dependent checkpoint), and
`--list-models` lists all four models.

## 6. Validation gaps / caveats

1. **No ROM-backed validation.**  The copyrighted images were fetched to
   `/tmp` only for hash/signature inspection.  Boot checkpoints, key-sequence
   accuracy and rendering were not compared against a reference emulator for
   these two ROMs; the existing `rom_boot_checkpoint` test still targets the
   fx-991ES PLUS C image only.
2. **Extra emulator RAM at 0x9800.**  CasioEmu maps another 0x100 bytes there
   for `!real_hardware` models.  `Machine` currently maps 0x8E00..0x8E2F as
   the keypad/mailbox and leaves 0x9800 unmapped (reads return 0).  This may
   matter for firmware that uses that scratch area.
3. **Keyboard ghosting/Pd are simplified.**  `Machine` implements the
   active-low, KO-gated KI read but not CasioEmu's multi-key ghost propagation,
   the input filter at 0xF042, the extra KO lines 7–9 or the 0xF050 Pd value.
4. **Secondary key legends and the ROM keycode table are not shipped.**  The
   descriptor uses the shared primary-label table; exact SHIFT/ALPHA legends
   for the fx-82ES PLUS A were not transcribed from a physical device.
5. **No model-specific skin.**  The bundled `assets/skin.png` is for the
   fx-991ES PLUS C; the fallback keypad uses the shared 43×28 grid and will
   show fx-991-style labels for both new models.
6. **Interrupt/timer/contrast SFR details** (`0xF010`, `0xF014`, `0xF030`..
   `0xF032`, `0xF050`) are unchanged from the existing core and have not been
   characterised against these ROMs.
7. **Emulator ROM provenance.**  The `fx82esp-emu` signature `GY450XE` is
   distinct from CasioEmu's `fx82es_emu` readme hash; if a user supplies the
   older fx-82ES emulator ROM, detection will not match it (use
   `--model fx82esp-emu` explicitly; board parameters are the same).
