#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."

rustup target add wasm32-unknown-unknown >/dev/null 2>&1 || true
cargo build -p casio-emu-web --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir web/www/pkg \
  target/wasm32-unknown-unknown/release/casio_emu_web.wasm

# The device image is copyrighted CASIO artwork.  It is intentionally not
# committed; copy your own extracted skin into place for local testing.
if [ -f assets/skin.png ]; then
  cp assets/skin.png web/www/skin.png
fi

echo "built web/www"
echo "run: python3 -m http.server -d web/www 8000"
