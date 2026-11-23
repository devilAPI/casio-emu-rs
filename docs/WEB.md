# Web / WebAssembly build

The browser frontend uses the same Rust core compiled to WebAssembly. The
canvas keeps the 289:582 aspect ratio at any viewport size (width is derived
from the height budget instead of letting `max-width` squash it), and its
backing store is sized to the display's device-pixel ratio so the LCD and
clickable keys stay sharp. The LCD overlay is redrawn in design coordinates.

## Build

Requirements:

- Rust with the `wasm32-unknown-unknown` target
- `wasm-bindgen` CLI **0.2.130** (matching the crate version)
- `python3` for the local static server

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.130 --locked

cd casio-emu-rs
./web/build.sh
```

`web/build.sh` builds the crate, runs `wasm-bindgen`, and copies
`assets/skin.png` into `web/www/` if it exists.

## Run locally

```bash
python3 -m http.server -d web/www 8000
# open http://localhost:8000/
```

Load a ROM dump with the file picker. For a private local demo you can place
your own ROM at `web/www/fx991es.rom.bin` and open:

```text
http://localhost:8000/?rom=fx991es.rom.bin&boot=6000000
```

The `boot` parameter runs that many instructions immediately after loading,
which is useful for screenshots.

## Deploy

The generated `web/www` directory is a static site. Publish it with GitHub
Pages, Netlify, Cloudflare Pages, or any static host. The repository's
release workflow attaches a `casio-emu-web.zip` artifact containing:

- `index.html`
- `main.js`
- `style.css`
- `pkg/` (wasm-bindgen output)

It intentionally does **not** contain a ROM or the CASIO device artwork.

## Notes

- The web UI supports keyboard input and clicking the drawn device keys.
- The browser must serve the page over HTTP(S); WASM cannot be loaded from a
  `file://` URL in most browsers.
- The page falls back to a plain LCD if `skin.png` is missing.
