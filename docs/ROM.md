# Firmware and device image

This project does **not** ship the CASIO firmware or the official device
artwork. Both are copyright CASIO. The emulator runs a ROM image dumped from a
copy you are licensed to use, and the device background is optional.

## Why there is no `rom.bin` in the repository

The official Windows emulator is an InstallShield bootstrapper containing an
MSI/CAB with:

- `fx-991ES PLUS C Emulator.exe` — MFC UI
- `SimU8.dll` — emulator front-end
- `SimU8engine.dll` — `CSimU8core` nX-U8/100 CPU simulator
- `fxESPLUS_P16.dll` — device artwork

The firmware is decrypted/assembled at runtime into two heap buffers inside
`CSimU8core`. It is not a plain file in the installer and it is not a static
resource that can simply be copied out of the DLL. The ROM is therefore
extracted from the running emulator.

> Obtain the official emulator from CASIO (https://edu.casio.com/) or your
> licensed installer. This project intentionally does not link to third-party
> firmware mirrors.

## Extract the ROM

### Linux + Wine

```bash
export WINEPREFIX="$HOME/.wine"
python3 tools/extract_rom.py \
  --exe "$WINEPREFIX/drive_c/Program Files (x86)/CASIO/fx-ES PLUS Emulator Ver. 4/fx-991ES PLUS C Emulator/fx-991ES PLUS C Emulator.exe" \
  --out fx991es.rom.bin
```

The script launches the emulator, waits for the code buffers to be populated,
reads the two code segments with `process_vm_readv(2)`, truncates the image to
the used 64 KiB pages, and writes `fx991es.rom.bin`.

If `ptrace` is restricted (`/proc/sys/kernel/yama/ptrace_scope = 1`) and the
Wine process is not your child, run the script as the parent (it launches the
emulator itself) or temporarily set the scope to `0`.

### Windows

Use [`fxesdev/RomDumper`](https://github.com/fxesdev/RomDumper):

```text
dump.exe oa <PID of "fx-991ES PLUS C Emulator.exe">
```

Concatenate the segment-0 and segment-N dumps and truncate to 128 KiB.

### Place the ROM

The desktop binary looks for, in order:

1. `--rom PATH`
2. `fx991es.rom.bin` in the current directory
3. `roms/fx991es.rom.bin`

The web frontend never bundles a ROM; use the **Load ROM dump** file picker,
or serve a private copy for local demos with `?rom=fx991es.rom.bin`.

## Extract the device image

The official skin is stored as bitmap resource `3000.bmp` (289×582) and
`3001.bmp` (405×816) in `fxESPLUS_P16.dll`:

```bash
python3 tools/extract_skin.py \
  --dll "/path/to/fxESPLUS_P16.dll" \
  --variant 3000
```

This writes `assets/skin.bmp` and `assets/skin.png`, and copies them to
`web/www/` if that directory exists. The web build uses `skin.png`; the
desktop frontend uses its built-in reflowing UI instead.

`tools/extract_all.sh` runs both extractors:

```bash
tools/extract_all.sh --exe "/path/to/fx-991ES PLUS C Emulator.exe" \
                     --dll "/path/to/fxESPLUS_P16.dll"
```

## Legal

Only dump and use files from a copy you are legally licensed to use. Do not
commit the ROM or the extracted artwork to a public repository and do not
redistribute them. The GPL-3.0 license of this project applies to the Rust
source code, not to CASIO's firmware or artwork.
