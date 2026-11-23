#!/usr/bin/env bash
# Extract both the firmware and the device skin from a local, licensed copy
# of the official CASIO emulator.  Neither file is redistributed.
set -euo pipefail
cd "$(dirname "$0")/.."

EXE=""
DLL=""
ROM_OUT="fx991es.rom.bin"

while [ $# -gt 0 ]; do
  case "$1" in
    --exe) EXE="$2"; shift 2 ;;
    --dll) DLL="$2"; shift 2 ;;
    --rom-out) ROM_OUT="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [ -n "$EXE" ]; then
  python3 tools/extract_rom.py --exe "$EXE" --out "$ROM_OUT"
else
  python3 tools/extract_rom.py --out "$ROM_OUT"
fi

if [ -n "$DLL" ]; then
  python3 tools/extract_skin.py --dll "$DLL"
else
  python3 tools/extract_skin.py
fi

echo
echo "done: $ROM_OUT and assets/skin.png"
