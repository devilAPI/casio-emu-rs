#!/usr/bin/env bash
# Download the public ROM dumps referenced by docs/MODELS.md into roms/.
#
# These images are copyrighted by CASIO COMPUTER CO., LTD.  They are fetched
# for local emulation only and are gitignored (*.bin).  Never commit or
# redistribute them.  Only use ROMs you are licensed to use.
set -euo pipefail

cd "$(dirname "$0")/.."
mkdir -p roms

base="https://raw.githubusercontent.com/user202729/fxesplus/master"

fetch() {
    local name="$1" url="$2" size="$3"
    local out="roms/${name}.rom.bin"
    if [[ -s "$out" ]]; then
        echo "skip  $out ($(stat -c%s "$out") bytes)"
        return
    fi
    echo "fetch $out"
    curl -fL --retry 3 --connect-timeout 20 -o "$out" "$url"
    local actual
    actual=$(stat -c%s "$out")
    if [[ "$actual" != "$size" ]]; then
        echo "error: $out is $actual bytes, expected $size" >&2
        return 1
    fi
    echo "      $actual bytes OK"
}

fetch fx570es-plus    "https://raw.githubusercontent.com/ZaoHan415/fxesplus/master/dumps/rom.bin" 131072
fetch fx82esplusa     "$base/82espa/rom.bin"                   131072
fetch fx82esp-emu     "$base/82esp/emu_rom.bin"                131072
fetch fx570vnx        "$base/580vnx/rom.bin"                   262144
fetch fx570vnx-emu    "$base/580vnx_emu/rom.bin"               262144
fetch fx991cnx        "$base/991cnx/rom.bin"                   262144
fetch fx991cnx-emu    "$base/991cnx_emu/rom.bin"               262144
fetch fx991cncw       "https://raw.githubusercontent.com/Physics365/991CN-X-CW-Decompilation_Files/main/%E7%9C%9F%E6%9C%BAROM/991CN%20CW/rom.bin" 1048576

# The fx-991ES PLUS C ROM is not published by these repos; copy a local dump
# if one is available.
if [[ -f fx991es.rom.bin && ! -s roms/fx991es-plus-c.rom.bin ]]; then
    cp fx991es.rom.bin roms/fx991es-plus-c.rom.bin
    echo "copy  roms/fx991es-plus-c.rom.bin"
fi

echo
echo "ROMs in roms/:"
ls -l roms/*.rom.bin
