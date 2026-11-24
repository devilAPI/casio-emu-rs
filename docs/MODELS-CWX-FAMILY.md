# ClassWiz X family (HW_CLASSWIZ, hardware id 4)

All dumps in the `models/models` archive that use the 256 KiB (0x40000)
ClassWiz board.  Every ROM stores its six/seven-byte self-test string at
`0x3FFEE` (`SIGNATURE_OFFSET_CLASSWIZ`).  All of them use the same ClassWiz
board definition (`BoardKind::ClassWiz`, `csr_mask = 0x0F`, 8 KiB RAM at
`0x0D000`, 192x64 VRAM, hardware KI/KO SFR keyboard with `keys::VNX_KEYS`), so
adding a descriptor does not change the already-working timer/IRQ/standby/
screen-mode emulation.

`model.lua` is authoritative for model names, `hardware_id` (4 for all of
them), `csr_mask` (0x000F), `pd_value` (0x00), `real_hardware` and the button
map (identical to the fx-580VN X reference keypad).  Dump identity below is
Deduplicated by full SHA-256; duplicate folders sharing a hash are listed
together in the source column.

## Models

| id | Model name (`model.lua`) | ROM size | Signature @ 0x3FFEE | Source folder | Keyboard interface | `real_hardware` | SHA-256 (prefix) | Status |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `fx580vnx` | fx-580VN X | 262144 (0x40000) | `CY-298A` | `fx580vnx` | HardwareSfr (`VNX_KEYS`) | 1 | `7a164c650d6e6bc8` | already registered |
| `fx570vnx` | fx-570VN X (same ROM as fx-580VN X) | 262144 (0x40000) | `CY-298` | (published fx-580VN X image) | HardwareSfr (`VNX_KEYS`) | 1 | `7a164c650d6e6bc8` | already registered |
| `fx991ex` | fx-991EX | 262144 (0x40000) | `CY-235F` | `fx991ex` | HardwareSfr (`VNX_KEYS`) | 1 | `fb6848532c04a4f2` | already registered; archive ROM byte-identical |
| `fx991cnx` | fx-991CN X | 262144 (0x40000) | `CY-239` (ROM: `CY-239C`) | `fx991cnx` | HardwareSfr (`VNX_KEYS`) | 1 | `8b6b6430f96858a8` | already registered; archive ROM byte-identical |
| `cy23d` | CY-23D | 262144 (0x40000) | `CY-23D` | `cy23d` | HardwareSfr (`VNX_KEYS`) | 1 | `229e3825d977b302` | new |
| `fx82cnx-a` | fx-82CN X (rev A) | 262144 (0x40000) | `CY-236A` | `fx82cnx` | HardwareSfr (`VNX_KEYS`) | 1 | `89cdfbe7b6142bbc` | new |
| `fx82cnx-b` | fx-82CN X (rev B) | 262144 (0x40000) | `CY-236B` | `fx-82 CN X` | HardwareSfr (`VNX_KEYS`) | 0 | `bbf3c5f8b6239f9d` | new |
| `fx83gtx` | fx-83GT X | 262144 (0x40000) | `CY-213A` | `fx-83 GT X` | HardwareSfr (`VNX_KEYS`) | 0 | `be0312bf36863771` | new |
| `fxjp700` | fx-JP700 | 262144 (0x40000) | `CY-242A` | `fxjp700`, `fx-JP700` (duplicate) | HardwareSfr (`VNX_KEYS`) | 1 | `2d6bf7867d2bfe66` | new |
| `fxjp900` | fx-JP900 | 262144 (0x40000) | `CY-243F` | `fxjp900`, `fx-JP900` (duplicate) | HardwareSfr (`VNX_KEYS`) | 1 | `f8990442c26e49a1` | new |
| `fx530az` | fx-530AZ | 262144 (0x40000) | `CY-240A` | `fx530az` | HardwareSfr (`VNX_KEYS`) | 1 | `35ee817afc7472d7` | new |
| `fx82arx` | fx-82AR X | 262144 (0x40000) | `CY-256A` | `fx82arx` | HardwareSfr (`VNX_KEYS`) | 1 | `f921562f52fafa6c` | new |
| `fx82cex` | fx-82CE X | 262144 (0x40000) | `CY-291E` | `fx82cex` | HardwareSfr (`VNX_KEYS`) | 1 | `b39443de80d957a6` | new |
| `fx85gtx` | fx-85GT X | 262144 (0x40000) | `CY-214A` | `fx85gtx` | HardwareSfr (`VNX_KEYS`) | 1 | `c1bf1bdb2689f4db` | new |
| `fx92p` | fx-92+ Spéciale Collège | 262144 (0x40000) | `CY-295A` | `fx92p` | HardwareSfr (`VNX_KEYS`) | 1 | `095a9b26eb91caf8` | new |
| `fx92sc` | fx-92 Spéciale Collège | 262144 (0x40000) | `CY-246A` | `fx92sc` | HardwareSfr (`VNX_KEYS`) | 1 | `d9cabc97e4803b21` | new |
| `fx97sgx` | fx-97SG X | 262144 (0x40000) | `CY-297E` | `fx97sgx` | HardwareSfr (`VNX_KEYS`) | 1 | `c25e97394c713895` | new |
| `fx991cex` | fx-991CE X | 262144 (0x40000) | `CY-294F` | `fx991cex` | HardwareSfr (`VNX_KEYS`) | 1 | `d277fcf41c310086` | new |
| `fx991dex` | fx-991DEX (rev A) | 262144 (0x40000) | `CY-251A` | `fx991dex` | HardwareSfr (`VNX_KEYS`) | 1 | `0d32e34941841a3f` | new; distinct from `fx991dex-verf` (`CY-251F`) |
| `fx580vnx-cwii` | fx-580VN X (font CWII) | 262144 (0x40000) | `CY-298A` (collision; descriptor uses empty signature) | `fx-580VN X (font CWII)` (no `model.lua`) | HardwareSfr (`VNX_KEYS`) | n/a | `f3259b162d68cc4d` | new; explicit `--model fx580vnx-cwii` only |

## Detection collisions and resolution

`Model::detect()` scans `model::ALL` in order and uses `starts_with` on each
model's signature, so a shorter signature can shadow a longer one that begins
with it.  The new descriptors are ordered and shaped as follows:

* **`CY-298A` (fx-580VN X font CWII vs fx-580VN X).**  These are two different
  ROM images with the same self-test string.  `FX580VNX` appears earlier in
  `ALL` and wins auto-detection.  `FX580VNX_CWII` therefore carries an empty
  signature and is selectable only with `--model fx580vnx-cwii`.
* **`CY-236A` vs `CY-236B` (fx-82CN X revisions A/B).**  Both signatures are
  seven bytes and differ in the last character, so each descriptor matches
  only its own dump (`FX82CNX_A` and `FX82CNX_B`).
* **`CY-251A` (new fx-991DEX) vs `CY-251F` (`fx991dex-verf`).**  Also distinct
  at the seventh byte; auto-detection picks the correct descriptor for both.
* **`CY-239C` / `CY-235F`.**  Already resolved by the existing `FX991CNX`
  (`CY-239`) and `FX991EX` (`CY-235F`) descriptors.  The archive ROMs are
  byte-identical to the dumps already installed in `roms/`, so no new
  descriptor was added.
* **`CY-298` (`fx570vnx`).**  A generic six-byte prefix that also matches
  `CY-298A` images; the existing ordering puts `FX580VNX` before `FX570VNX`,
  so `CY-298A` dumps are attributed to the fx-580VN X.  Unchanged by this work.

## ROM files

All dumps are installed in the gitignored top-level `roms/` folder as
`<id>.rom.bin`, e.g. `roms/cy23d.rom.bin`, `roms/fx82cnx-a.rom.bin`,
`roms/fx580vnx-cwii.rom.bin`.  The already-registered models keep their
existing files (`fx580vnx.rom.bin`, `fx991ex.rom.bin`, `fx991cnx.rom.bin`),
which are byte-identical to the corresponding archive dumps.  ROM files are
never added to git and are never placed under `src/`.

## Keyboard

Every model in this family uses the same button map.  In `model.lua` codes are
`KO << 4 | KI`; this emulator stores active-high bit masks
(`ki = 1 << (code & 0xF)`, `ko = 1 << (code >> 4)`), giving `keys::VNX_KEYS`.
Keys are delivered through the hardware SFRs `KI_PORT` (0x0F040),
`KO_DRIVE_LO/HI` (0x0F046/0x0F047) and `KO_MASK_LO/HI` (0x0F044/0x0F045).
`real_hardware = 0` on two dumps only affects how the original CasioEmu
injects keys; the Windows-emulator ClassWiz mailbox lives at 0x48E00, outside
this emulator's 16-bit mailbox space, so this build always uses the SFR path.
