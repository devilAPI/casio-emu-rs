# Firmware

The CASIO firmware image is **not** included in this repository. It is
copyrighted by CASIO and cannot be redistributed.

Place a dump from your own licensed copy here as `fx991es.rom.bin`, or pass an
explicit path with `--rom`.  The fx-570ES PLUS ROM uses the same filename by
default; `--model fx570es-plus` selects its real-hardware keyboard interface
(also auto-detected from the ROM signature).

See [`docs/MODELS.md`](../docs/MODELS.md) for supported model ids and hashes.

## Bundled downloads

Run `tools/fetch_roms.sh` to populate this directory with the public ROM
images.  All files are gitignored (`*.bin`) and are never committed:

```text
fx991es-plus-c.rom.bin   fx570es-plus.rom.bin    fx82esplusa.rom.bin
fx82esp-emu.rom.bin      fx570vnx.rom.bin        fx570vnx-emu.rom.bin
fx580vnx.rom.bin         fx991cnx.rom.bin        fx991cnx-emu.rom.bin
fx991cncw.rom.bin        fx991cw.rom.bin         fx9910cw.rom.bin
fx991ex.rom.bin          fx82ex-vere.rom.bin     fx991dex-verf.rom.bin
fx991dex-vere.rom.bin    fx880btg.rom.bin
```

`fx580vnx.rar` is the source archive for `fx580vnx.rom.bin`; its companion
files are extracted in `roms/fx580vnx/` (`ram.dmp`, `config.bin`,
`interface.png`).  The archive's ROM is byte-identical to the one already
registered as the `fx580vnx` model.

The whole `roms/` directory is gitignored (`.gitignore` contains `/roms/`).

The desktop app loads the matching file automatically when you switch models
with **F6 / F7** (or the on-screen `<` / `>` selector), so all models can be
tried in one session.

Generate the dump with `tools/extract_rom.py` (Linux + Wine) or
`fxesdev/RomDumper` (Windows). See `docs/ROM.md` for details.
