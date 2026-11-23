#!/usr/bin/env python3
"""
Extract the fx-991ES PLUS C firmware image from a running copy of the
official CASIO emulator.

The official emulator keeps the decrypted code image in two heap buffers
inside SimU8.dll.  This script launches the emulator under Wine (or attaches
to a process you already started), reads those buffers with
process_vm_readv(2), and writes a flat ROM image.

The ROM is copyrighted by CASIO.  Only use this with a copy you are legally
licensed to use, and do not redistribute the resulting file.

Usage:
    ./extract_rom.py                         # launch the installed app under Wine
    ./extract_rom.py --exe /path/to/app.exe  # launch a different copy
    ./extract_rom.py --pid 12345             # attach to an existing child process
    ./extract_rom.py --out my.rom.bin

Requirements: Linux, Wine, Python 3.8+, and a working CASIO emulator install.
"""

import argparse
import ctypes
import os
import re
import struct
import subprocess
import sys
import time
from pathlib import Path

# SimU8.dll + 0x16CE20 is the CSimU8core object.  The memory-region
# descriptors are at +0x2C (code page 0), +0x48 (RAM), +0x64 (code page N).
CORE_OBJECT_OFFSET = 0x16CE20
SEG0_OFFSET = 0x2C
RAM_OFFSET = 0x48
SEG1_OFFSET = 0x64

DEFAULT_EXE = (
    "drive_c/Program Files (x86)/CASIO/fx-ES PLUS Emulator Ver. 4/"
    "fx-991ES PLUS C Emulator/fx-991ES PLUS C Emulator.exe"
)


class IOVec(ctypes.Structure):
    _fields_ = [("iov_base", ctypes.c_void_p), ("iov_len", ctypes.c_size_t)]


_libc = ctypes.CDLL(None, use_errno=True)
_process_vm_readv = _libc.process_vm_readv
_process_vm_readv.argtypes = [
    ctypes.c_int,
    ctypes.POINTER(IOVec),
    ctypes.c_ulong,
    ctypes.POINTER(IOVec),
    ctypes.c_ulong,
    ctypes.c_ulong,
]
_process_vm_readv.restype = ctypes.c_ssize_t


def read_mem(pid: int, addr: int, size: int) -> bytes:
    buf = ctypes.create_string_buffer(size)
    local = IOVec(ctypes.cast(buf, ctypes.c_void_p), size)
    remote = IOVec(ctypes.c_void_p(addr), size)
    n = _process_vm_readv(pid, ctypes.byref(local), 1, ctypes.byref(remote), 1, 0)
    if n < 0:
        err = ctypes.get_errno()
        raise OSError(err, os.strerror(err))
    return buf.raw[:n]


def maps(pid: int):
    out = []
    with open(f"/proc/{pid}/maps") as f:
        for line in f:
            m = re.match(
                r"([0-9a-f]+)-([0-9a-f]+)\s+(\S+)\s+(\S+)\s+(\S+)\s+(\S+)\s*(.*)",
                line,
            )
            if m:
                out.append(
                    (
                        int(m.group(1), 16),
                        int(m.group(2), 16),
                        m.group(3),
                        m.group(7),
                    )
                )
    return out


def find_simu8_base(pid: int):
    for start, _end, perms, path in maps(pid):
        if path.endswith("/SimU8.dll") and "r" in perms:
            return start
    return None


def find_app_pid(exe_name: str, parent: int):
    """Find the emulator process.  Wine usually execs in the launcher PID, but
    some versions fork, so walk the descendant tree as a fallback."""
    candidates = []
    for entry in os.listdir("/proc"):
        if not entry.isdigit():
            continue
        pid = int(entry)
        try:
            cmd = (
                open(f"/proc/{pid}/cmdline", "rb")
                .read()
                .replace(b"\0", b" ")
                .decode(errors="replace")
            )
        except OSError:
            continue
        if exe_name not in cmd:
            continue
        p = pid
        for _ in range(8):
            if p == parent:
                candidates.append(pid)
                break
            try:
                p = int(open(f"/proc/{p}/stat").read().split()[3])
            except OSError:
                break
    return candidates[0] if candidates else None


def region(blob: bytes, offset: int):
    # struct memory_region:
    # +0x00 unk[4], +0x04 buf_addr, +0x08 buf_size,
    # +0x0C unk[8], +0x14 lower, +0x18 upper, +0x1C unk[4]
    buf_addr, buf_size = struct.unpack_from("<II", blob, offset + 4)
    return buf_addr, buf_size


def extract(pid: int, out: Path, keep: bool = False, launcher=None):
    base = None
    seg0 = seg1 = None
    obj = None
    deadline = time.time() + 30
    while time.time() < deadline:
        base = find_simu8_base(pid)
        if base is not None:
            try:
                obj = read_mem(pid, base + CORE_OBJECT_OFFSET, 0x100)
                a0, s0 = region(obj, SEG0_OFFSET)
                a1, s1 = region(obj, SEG1_OFFSET)
                if 0 < s0 <= 0x100000 and 0 < s1 <= 0x800000:
                    head = read_mem(pid, a0, 16)
                    if any(head):
                        seg0 = (a0, s0)
                        seg1 = (a1, s1)
                        break
            except OSError:
                pass
        time.sleep(0.5)

    if base is None or obj is None or seg0 is None or seg1 is None:
        print("could not locate the emulator's ROM buffers", file=sys.stderr)
        print("(is the calculator window open and fully booted?)", file=sys.stderr)
        return 1

    print(f"SimU8.dll base : 0x{base:08x}")
    print(f"ROM segment 0  : 0x{seg0[0]:08x} ({seg0[1]:#x} bytes)")
    print(f"ROM segment N  : 0x{seg1[0]:08x} ({seg1[1]:#x} bytes)")

    rom = read_mem(pid, seg0[0], seg0[1])
    rom += read_mem(pid, seg1[0], seg1[1])

    # Truncate to the last partially used 64 KiB page, with a 128 KiB floor.
    last = max((i for i, b in enumerate(rom) if b), default=0)
    size = ((last // 0x10000) + 1) * 0x10000
    size = max(size, 0x20000)
    rom = rom[:size]

    out.write_bytes(rom)
    print(f"wrote {len(rom)} bytes to {out}")

    if not keep and launcher is not None:
        try:
            launcher.terminate()
        except Exception:
            pass
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--exe", default=None, help="path to the emulator executable")
    ap.add_argument("--pid", type=int, default=None, help="attach to an existing PID")
    ap.add_argument("--out", default="fx991es.rom.bin", help="output ROM file")
    ap.add_argument("--keep", action="store_true", help="leave the emulator running")
    ap.add_argument("--timeout", type=float, default=20.0, help="boot wait in seconds")
    args = ap.parse_args()

    out = Path(args.out)
    launcher = None

    if args.pid is not None:
        return extract(args.pid, out, keep=True)

    exe = args.exe
    if exe is None:
        prefix = os.environ.get("WINEPREFIX", str(Path.home() / ".wine"))
        exe = str(Path(prefix) / DEFAULT_EXE)
    exe_path = Path(exe)
    if not exe_path.exists():
        print(f"emulator not found: {exe_path}", file=sys.stderr)
        print("pass --exe or install the official emulator first", file=sys.stderr)
        return 1

    env = os.environ.copy()
    env.setdefault("WINEDEBUG", "-all")
    # The /usr/bin/wine wrapper can re-parent the Windows process; use the
    # real loader when available so the emulator stays our child and
    # ptrace/process_vm_readv permissions are inherited.
    wine_bin = "/usr/lib/wine/wine" if os.path.exists("/usr/lib/wine/wine") else "wine"
    print(f"launching {exe_path} with {wine_bin}")
    launcher = subprocess.Popen(
        [wine_bin, str(exe_path)],
        cwd=str(exe_path.parent),
        env=env,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        start_new_session=True,
    )

    pid = None
    deadline = time.time() + args.timeout
    while time.time() < deadline:
        pid = find_app_pid(exe_path.name, os.getpid())
        if pid is not None:
            break
        time.sleep(0.5)
    if pid is None:
        # Wine may exec in the launcher PID.
        pid = launcher.pid

    print(f"emulator PID {pid}")
    return extract(pid, out, keep=args.keep, launcher=launcher)


if __name__ == "__main__":
    raise SystemExit(main())
