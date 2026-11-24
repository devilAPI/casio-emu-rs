# ES PLUS / ES PLUS 2 / fx-5800P family (new 128 KiB archive)

This note records the descriptors added to `src/model.rs` for the 128 KiB
`HW_ES_PLUS` / `HW_FX_5800P` ROMs in the new `roms/models/models/` archive.
It covers only models **not already registered** — `fx991es-plus-c`,
`fx570es-plus`, `fx82esplusa` and `fx82esp-emu` are unchanged, and the
`fx-570ES PLUS A` / `fx-82ES PLUS A` dumps (`fx570esplus`, `fx82espa`,
`fx82esplusa`) are already covered by `fx570es-plus` / `fx82esplusa`.

`model.lua` was used as the authority for `model_name`, `hardware_id`,
`real_hardware` and the key map wherever present.  Folders without a
`model.lua` were decoded from their `config.bin` header (csr_mask at 0x72,
chip id at 0x74, real_hardware at 0x76) and named from the folder.

All images are exactly **131072 bytes (128 KiB = two 64 KiB code pages)**.
Self-test signatures live at `0x1FFF4`; the descriptor stores the six-byte
prefix used by `model::detect()`.  Signatures shown below are the full
seven-byte string in the ROM.

## Added models

| id | name | ROM size | signature @ 0x1FFF4 | keyboard interface | source folder |
|---|---|---:|---|---|---|
| `fx115esplus2-emu` | Casio fx-115ES PLUS 2 (emulator ROM) | 128 KiB | `CY-874A` | emulator mailbox `0x8E00` | `fx115esplus2_emu` |
| `fx570es-emu` | Casio fx-570ES (emulator ROM) | 128 KiB | *(none — explicit `--model` required)* | emulator mailbox `0x8E00` | `fx570es_emu` |
| `fx570esplus2-emu` | Casio fx-570ES PLUS 2 (emulator ROM) | 128 KiB | `CY-854A` | emulator mailbox `0x8E00` | `fx570esplus2_emu` |
| `fx570vnplus` | Casio fx-570VN PLUS | 128 KiB | `LY710XA` | hardware KI/KO SFRs | `fx570vnplus` |
| `fx570vnplus2` | Casio fx-570VN PLUS 2 | 128 KiB | `CY-860A` | hardware KI/KO SFRs | `fx570vnplus2` |
| `fx570vnplus2-emu` | Casio fx-570VN PLUS 2 (emulator ROM) | 128 KiB | `CY-860A` *(shared)* | emulator mailbox `0x8E00` | `fx570vnplus2_emu` |
| `fx570vnplus-emu` | Casio fx-570VN PLUS (emulator ROM) | 128 KiB | `LY710XA` *(shared)* | emulator mailbox `0x8E00` | `fx570vnplus_emu` |
| `fx5800p` | Casio fx-5800P | 128 KiB + 512 KiB flash | *(none — explicit `--model` required)* | hardware KI/KO SFRs | `fx5800p` |
| `fx82auplusii-emu` | Casio fx-82AU PLUS II (emulator ROM) | 128 KiB | `LY711XA` | emulator mailbox `0x8E00` | `fx82auplusii_emu` |
| `fx82auplusii2` | Casio fx-82AU PLUS II (2nd edition) | 128 KiB | `CY-863A` | hardware KI/KO SFRs | standalone `/home/devlapi/Downloads/rom.bin` |

Board parameters shared by every descriptor above: `BoardKind::EsPlus`,
RAM `0x8000..0x8DFF`, VRAM at `0xF800` (96×32, 16-byte rows), 49 matrix keys
+ ON from `keys::KEYS`, `key_mailbox = 0x8E00` on emulator images.
`csr_mask = 0x01` for all ES PLUS / ES PLUS 2 models; the fx-5800P keeps the
`csr_mask = 0x000f` declared in its `model.lua`.

### Per-folder parameters (from `model.lua` / `config.bin`)

| source folder | model_name | hardware_id | real_hardware | notes |
|---|---|---:|---:|---|
| `fx115esplus2_emu` | *(folder-derived)* fx-115ES PLUS 2 | HW_ES_PLUS (3) | 0 | config.bin: `csr_mask=1` |
| `fx570es_emu` | *(folder-derived)* fx-570ES | HW_ES_PLUS (3) | 0 | see the hash caveat below |
| `fx570esplus2_emu` | *(folder-derived)* fx-570ES PLUS 2 | HW_ES_PLUS (3) | 0 | config.bin: `csr_mask=1` |
| `fx570vnplus` | *(folder-derived)* fx-570VN PLUS | HW_ES_PLUS (3) | 1 | |
| `fx570vnplus2` | *(folder-derived)* fx-570VN PLUS 2 | HW_ES_PLUS (3) | 1 | |
| `fx570vnplus2_emu` | *(folder-derived)* fx-570VN PLUS 2 | HW_ES_PLUS (3) | 0 | |
| `fx570vnplus_emu` | *(folder-derived)* fx-570VN PLUS | HW_ES_PLUS (3) | 0 | |
| `fx5800p` | `fx-5800p` | HW_FX_5800P (345) | 1 | `csr_mask=0x000f`, `flash_path="flash.bin"` |
| `fx82auplusii_emu` | `CASIO fx-82AU PLUS II (Emulator ROM)` | HW_ES_PLUS (3) | 0 | `pd_value=0x00` |
| standalone dump | fx-82AU PLUS II 2nd edition (per submitter) | HW_ES_PLUS (3) | 1 | signature bytes `…01 01` match the other real-hardware dumps |

## Signature collisions and detection order

`model::detect()` returns the **first** model in `model::ALL` whose signature
prefix matches at the model's signature offset.  The registry order therefore
encodes the following policy:

* `LY710XA` is shared by `fx570vnplus` (real hardware) and
  `fx570vnplus-emu`.  **`fx570vnplus` wins** auto-detection; the emulator dump
  must be loaded with `--model fx570vnplus-emu` (or by selecting that id in
  the GUI), otherwise it would be driven through the KI/KO SFR path.
* `CY-860A` is shared by `fx570vnplus2` (real hardware) and
  `fx570vnplus2-emu`.  **`fx570vnplus2` wins** auto-detection; use
  `--model fx570vnplus2-emu` for the emulator image.
* `fx570es-emu` and `fx5800p` have zeros at `0x1FFF4`, so they are never
  auto-detected.  Both need an explicit `--model` (their ROMs are still
  placed in `roms/` so `--model <id>` finds them automatically).

The other new signatures (`CY-874A`, `CY-854A`, `LY711XA`, `CY-863A`) are
unique in the registry and auto-detect normally.  No existing signature was
changed, so `fx-991ES PLUS C` accuracy is unaffected.

## fx-5800P: rom.bin vs flash.bin

The archive folder contains two images:

* `rom.bin` — 131072 bytes, the executable code image named by
  `rom_path = "rom.bin"` in `model.lua`.  **This is the file the descriptor
  loads** (copied as `roms/fx5800p.rom.bin`).  It has no self-test string at
  `0x1FFF4`; the `Gy344000` marker at `0x1E8B2` is a version string, not a
  signature.
* `flash.bin` — 524288 bytes, named by `flash_path = "flash.bin"`.  It is
  data/program flash (its tail contains `GY344 Ver1`), mapped outside the
  ES PLUS code window in the original emulator.  It is copied as
  `roms/fx5800p.flash.bin` for completeness but the current ES PLUS board
  does not map it, so firmware that pages code from flash will not run
  correctly.  This is a known limitation, not a reason to load flash.bin as
  the ROM.

The fx-5800P keypad in `model.lua` uses a different key-code layout
(e.g. codes `0x70`, `0x67`, `0x66` drive KO7/KO6 lines that are model-ID
resistors on the ES PLUS) and a different status-row geometry from the
49-key ES PLUS matrix.  The descriptor reuses `keys::KEYS` as a best-effort
fallback; full 5800P keyboard/DSP support would require board work outside
this task.

## Key map

The `model.lua` button maps for `fx570esplus`, `fx82espa`,
`fx82auplusii_emu` and `fx5800p` were checked.  All ES PLUS / ES PLUS 2
images use the same 49 matrix positions + ON as `src/keys.rs`:
`KO bit = 1 << (code >> 4)`, `KI bit = 1 << (code & 0xF)`, with `0xFF` being
ON.  No new key table was needed; every new descriptor points at
`keys::KEYS`, so PC key bindings and GUI hitboxes keep working (except the
documented fx-5800P layout difference).

## ROM inventory (hashes of the source images)

| source | SHA-256 |
|---|---|
| `fx115esplus2_emu/rom.bin` | `e3ca5d2141dcad0a6c65845de3c327515f8bd246f6aac8291578286e63527b9a` |
| `fx570es_emu/rom.bin` | `05d0babf1b3d2e560ecf3471df7d1717557028144d6cf71e64e6427111af8f3c` |
| `fx570esplus2_emu/rom.bin` | `4f09c90235dda8474a2f1232b9cc4b7fef8c271706382730ddfab83976829bea` |
| `fx570vnplus/rom.bin` | `e0731352099b311f4262eac1f086cb6d87546fe9e07e642dcb18b5087f8021ff` |
| `fx570vnplus2/rom.bin` | `1980c8b05328b645a33f817fc06f1ddfdffe37bac29150be05888e0bba29c8b4` |
| `fx570vnplus2_emu/rom.bin` | `35f5553ba6067b432d1545c34a0aa83a9a5c7be11901bb570dbc971830b2f1c7` |
| `fx570vnplus_emu/rom.bin` | `35ed75a055edef40af2210ed0bed59b34608b4870d4a78d38fb7adab48c3e836` |
| `fx5800p/rom.bin` | `9a190cb85d1c3c1f096dfabf0cd8393580beb59ae603e56871f76b721ce7de75` |
| `fx5800p/flash.bin` | `c3bd74079d60fd070a8fe0351d000b34f15a23e5bc78153b0c159e639ac712d4` |
| `fx82auplusii_emu/rom.bin` | `c60c317bdd8de8a6f0a0806fb08c1aa1150377ef401ad622faddb7535422c580` |
| standalone `/home/devlapi/Downloads/rom.bin` | `479957cc3d60a686f8b91267f11c595eaf483971905e6422b124c1a53ba8a262` |

`fx82espa/rom.bin` and `fx82esplusa/rom.bin` are byte-identical
(SHA-256 `ab58e84c…bf384`), which is why the existing `fx82esplusa`
descriptor covers both folders.

## Tests and validation

ROM-free unit tests in `src/model.rs` cover: auto-detection of all new
unique signatures, the two ambiguous signatures (real-hardware descriptor
wins), signatureless models not being auto-detected, all ROM sizes/boards/
LCD geometry, keyboard kinds, the shared key matrix, id lookup and ROM file
names, and id uniqueness.  The fx-991ES PLUS C accuracy test
(`--ignored`, requires the user's own ROM) was re-run and still passes.

Caveats: no new ROM was run to a boot checkpoint or compared against a
reference emulator; the mailbox/SFR keyboard split is taken from
`real_hardware` in the metadata.  The `fx570es_emu` dump's SHA-256 matches
the `fx82es_emu` ROM mentioned in `MODELS-82ESPLUS.md` (a different, older
fx-82ES emulator image), so the folder name may be a mislabel — the
descriptor follows the folder name as instructed, but the ROM contents are
not an fx-570ES image.  The fx-5800P's separate 512 KiB flash and distinct
key layout are not emulated (see above).
