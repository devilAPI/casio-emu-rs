# Device artwork (optional)

The official calculator skin is copyright CASIO and is **not** included in
this repository.

To use it, extract the bitmap resource from your own copy of the official
emulator:

```bash
python3 tools/extract_skin.py --dll /path/to/fxESPLUS_P16.dll
```

This writes `assets/skin.png` and `assets/skin.bmp`, and copies them into
`web/www/` for the browser build. The desktop frontend loads `assets/skin.png`
automatically and falls back to its built-in drawn keypad if it is absent.

`skin.png` and `skin.bmp` are intentionally ignored by git.
