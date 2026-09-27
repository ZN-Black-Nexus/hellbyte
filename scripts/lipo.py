#!/usr/bin/env python3
"""Merge thin Mach-O executables into one universal ("fat") binary.
usage: lipo.py OUT IN1 IN2 ...   (standard library only)"""
import struct, sys
out, ins = sys.argv[1], sys.argv[2:]
slices = []
for p in ins:
    d = open(p, "rb").read()
    magic, cputype, subtype = struct.unpack("<III", d[:12])
    assert magic == 0xFEEDFACF, f"{p} is not a 64-bit Mach-O"
    align = 14 if cputype == 0x0100000C else 12  # arm64 pages are 16 KiB
    slices.append((cputype, subtype, align, d))
off = 4096
hdr = struct.pack(">II", 0xCAFEBABE, len(slices))
body = b""
entries = []
for cputype, subtype, align, d in slices:
    a = 1 << align
    off = (off + a - 1) // a * a
    entries.append((cputype, subtype, off, len(d), align, d))
    off += len(d)
for cputype, subtype, o, size, align, _ in entries:
    hdr += struct.pack(">IIIII", cputype, subtype & 0xFFFFFFFF, o, size, align)
blob = bytearray(hdr.ljust(entries[0][2], b"\0"))
for _, _, o, size, _, d in entries:
    blob += b"\0" * (o - len(blob)) + d
open(out, "wb").write(blob)
print(f"{out}: {len(entries)} architectures, {len(blob)} bytes")
