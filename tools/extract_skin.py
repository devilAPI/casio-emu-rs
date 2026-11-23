#!/usr/bin/env python3
"""
Extract the calculator device image from fxESPLUS_P16.dll.

The official emulator stores the device artwork as Windows BITMAP resources
(3000.bmp is the 289x582 skin, 3001.bmp is the 405x816 high-DPI variant).
This script uses 7-Zip to unpack those resources and, if Pillow is available,
converts them to PNG for the Rust desktop and web frontends.

The image is copyright CASIO and is not part of this repository.  Only use it
with a copy you are licensed to use.

Usage:
    python3 tools/extract_skin.py --dll /path/to/fxESPLUS_P16.dll
    python3 tools/extract_skin.py --dll ... --variant 3001
"""

import argparse
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

DEFAULT_DLLS = [
    "drive_c/Program Files (x86)/CASIO/fx-ES PLUS Emulator Ver. 4/"
    "fx-991ES PLUS C Emulator/fxESPLUS_P16.dll",
]


def find_default() -> Path | None:
    prefix = Path(os.environ.get("WINEPREFIX", Path.home() / ".wine"))
    for rel in DEFAULT_DLLS:
        p = prefix / rel
        if p.exists():
            return p
    return None


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--dll", default=None, help="fxESPLUS_P16.dll (or extracted copy)")
    ap.add_argument("--variant", default="3000", choices=["3000", "3001"],
                    help="3000 = 289x582 skin, 3001 = 405x816 high-DPI skin")
    ap.add_argument("--out-dir", default="assets", help="output directory")
    ap.add_argument("--no-web", action="store_true", help="do not copy into web/www")
    args = ap.parse_args()

    dll = Path(args.dll) if args.dll else find_default()
    if dll is None or not dll.exists():
        print("fxESPLUS_P16.dll not found; pass --dll", file=sys.stderr)
        return 1

    seven = shutil.which("7z") or shutil.which("7za") or shutil.which("7zr")
    if not seven:
        print("7-Zip not found (install 7z)", file=sys.stderr)
        return 1

    out_dir = Path(args.out_dir)
    out_dir.mkdir(parents=True, exist_ok=True)

    with tempfile.TemporaryDirectory() as td:
        subprocess.run(
            [seven, "x", "-y", str(dll), f"-o{td}"],
            check=True,
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        bmp = next(Path(td).rglob(f"{args.variant}.bmp"), None)
        if bmp is None:
            print(f"bitmap {args.variant}.bmp not found in {dll}", file=sys.stderr)
            return 1

        bmp_out = out_dir / "skin.bmp"
        shutil.copyfile(bmp, bmp_out)
        print(f"wrote {bmp_out}")

        png_out = out_dir / "skin.png"
        try:
            from PIL import Image

            Image.open(bmp).convert("RGB").save(png_out)
            print(f"wrote {png_out}")
        except Exception as exc:
            print(f"Pillow not available, kept BMP only ({exc})", file=sys.stderr)

        if not args.no_web and Path("web/www").is_dir():
            for name in ("skin.bmp", "skin.png"):
                src = out_dir / name
                if src.exists():
                    shutil.copyfile(src, Path("web/www") / name)
                    print(f"copied {src} -> web/www/{name}")

    print()
    print("The device image is copyright CASIO. Do not redistribute it.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
