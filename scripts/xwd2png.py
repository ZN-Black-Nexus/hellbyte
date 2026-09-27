#!/usr/bin/env python3
"""Convert an XWD screen dump (e.g. Xvfb -fbdir output) to PNG. Stdlib only."""
import struct, sys, zlib
d = open(sys.argv[1], "rb").read()
f = struct.unpack(">25I", d[:100])
hsize, w, h, byteorder, bpp, bpl, ncolors = f[0], f[4], f[5], f[7], f[11], f[12], f[19]
off = hsize + ncolors * 12
rows = []
for y in range(h):
    row = bytearray(b"\0")
    line = d[off + y * bpl: off + (y + 1) * bpl]
    for x in range(w):
        p = line[x * 4: x * 4 + 4]
        v = int.from_bytes(p, "little" if byteorder == 0 else "big")
        row += bytes(((v >> 16) & 255, (v >> 8) & 255, v & 255))
    rows.append(bytes(row))
raw = b"".join(rows)
def chunk(t, b): return struct.pack(">I", len(b)) + t + b + struct.pack(">I", zlib.crc32(t + b) & 0xffffffff)
open(sys.argv[2], "wb").write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 6)) + chunk(b"IEND", b""))
print(w, h, bpp)
