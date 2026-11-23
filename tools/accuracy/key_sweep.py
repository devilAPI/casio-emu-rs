#!/usr/bin/env python3
"""Compare every key in the fx-991ES PLUS C matrix with the reference core.

Environment:
    C_BIN    path to the compiled keyseq_one reference harness
    R_BIN    path to the Rust fx991es-rs binary
    ROM      path to fx991es.rom.bin
    C_CWD    directory containing rom.bin for the C harness
"""
import os
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
C_BIN = os.environ.get("C_BIN", str(ROOT.parent / "u8_emu" / "keyseq_one"))
R_BIN = os.environ.get("R_BIN", str(ROOT / "target" / "release" / "fx991es-rs"))
ROM = os.environ.get("ROM", str(ROOT / "fx991es.rom.bin"))
C_CWD = os.environ.get("C_CWD", str(ROOT.parent / "u8_emu"))
STEPS = int(os.environ.get("STEPS", "6000000"))


def norm(text: str) -> str:
    lines = text.strip().splitlines()
    if lines and lines[0].startswith("pc="):
        parts = lines[0].split()
        lines[0] = " ".join(p for p in parts if not p.startswith("cycles="))
    return "\n".join(lines)


src = (ROOT / "src" / "keys.rs").read_text()
keys = []
for m in re.finditer(
    r'CalcKey \{\s*label: "([^"]+)",\s*ki: (0x[0-9a-fA-F]+),\s*ko: (0x[0-9a-fA-F]+),',
    src,
    re.S,
):
    label, ki, ko = m.group(1), int(m.group(2), 16), int(m.group(3), 16)
    if ki == 0 and ko == 0:
        continue
    keys.append((label, ki, ko))

bad = []
for i, (label, ki, ko) in enumerate(keys, 1):
    c = subprocess.run(
        [C_BIN, str(STEPS), hex(ki), hex(ko)],
        cwd=C_CWD,
        capture_output=True,
        text=True,
        timeout=120,
    ).stdout
    r = subprocess.run(
        [R_BIN, "--rom", ROM, "--headless", str(STEPS), "--press", f"2000000:{label}"],
        capture_output=True,
        text=True,
        timeout=120,
    ).stdout
    if norm(c) != norm(r) or not c.strip():
        bad.append(label)
        print(f"MISMATCH {label} ki={ki:#x} ko={ko:#x}")
    else:
        print(f"{i}/{len(keys)} {label:6} ok")

print(f"matched {len(keys) - len(bad)} of {len(keys)} keys")
if bad:
    print("bad:", ", ".join(bad))
sys.exit(1 if bad else 0)
