#!/usr/bin/env bash
# Differential accuracy check against the u8_emu reference core.
#
# Requirements:
#   - a licensed fx991es.rom.bin (FX991_ROM)
#   - https://github.com/Fraserbc/u8_emu cloned next to this repo, or set U8EMU_DIR
set -euo pipefail
cd "$(dirname "$0")/../.."

U8EMU_DIR="${U8EMU_DIR:-../u8_emu}"
ROM="${FX991_ROM:-./fx991es.rom.bin}"
STEPS="${STEPS:-10000000}"

if [ ! -d "$U8EMU_DIR/src/core" ]; then
  echo "u8_emu not found at $U8EMU_DIR" >&2
  echo "clone it: git clone https://github.com/Fraserbc/u8_emu '$U8EMU_DIR'" >&2
  exit 1
fi
if [ ! -f "$ROM" ]; then
  echo "ROM not found: $ROM (set FX991_ROM)" >&2
  exit 1
fi

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
ln -s "$(realpath "$ROM")" "$TMP/rom.bin"

echo "building reference harness..."
gcc -O2 -std=c99 -I"$U8EMU_DIR" -o "$TMP/hash" tools/accuracy/hash.c \
  "$U8EMU_DIR/src/core/core.c" "$U8EMU_DIR/src/core/instr.c" \
  "$U8EMU_DIR/src/core/instr_impl.c" "$U8EMU_DIR/src/core/regs.c" \
  "$U8EMU_DIR/src/core/mem.c"
gcc -O2 -std=c99 -I"$U8EMU_DIR" -o "$TMP/keyseq_one" tools/accuracy/keyseq_one.c \
  "$U8EMU_DIR/src/core/core.c" "$U8EMU_DIR/src/core/instr.c" \
  "$U8EMU_DIR/src/core/instr_impl.c" "$U8EMU_DIR/src/core/regs.c" \
  "$U8EMU_DIR/src/core/mem.c"

echo "building Rust release..."
cargo build --release -p fx991es-rs >/dev/null

echo "10M-instruction register/memory trace..."
(cd "$TMP" && ./hash "$STEPS" | head -n -1) > "$TMP/c.hash"
./target/release/fx991es-rs --rom "$ROM" --trace-hash "$STEPS" > "$TMP/r.hash"
if ! diff -q "$TMP/c.hash" "$TMP/r.hash" >/dev/null; then
  echo "TRACE MISMATCH" >&2
  diff "$TMP/c.hash" "$TMP/r.hash" | head -20 >&2
  exit 1
fi
echo "trace: $(wc -l < "$TMP/c.hash") checkpoints match"

echo "key matrix sweep..."
C_BIN="$TMP/keyseq_one" C_CWD="$TMP" ROM="$ROM" STEPS=6000000 \
  python3 tools/accuracy/key_sweep.py

echo "PASS"
