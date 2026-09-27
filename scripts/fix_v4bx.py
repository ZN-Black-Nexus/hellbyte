#!/usr/bin/env python3
"""Make an ARMv4T binary run on ARMv4 cores (StrongARM, Faraday FA526).

ARMv4 has no BX instruction. Like GNU ld's --fix-v4bx, rewrite every
`bx rN` (any condition) to the equivalent `mov pc, rN`. Only words inside
ARM code ranges are touched: the ELF must still have its `$a`/`$d` mapping
symbols (build unstripped, strip afterwards). Standard library only.

usage: fix_v4bx.py IN.elf OUT.elf
"""
import os, struct, sys

data = bytearray(open(sys.argv[1], "rb").read())
assert data[:4] == b"\x7fELF" and data[4] == 1 and data[5] == 1, "need a 32-bit little-endian ELF"
e_shoff, = struct.unpack_from("<I", data, 0x20)
e_shentsize, e_shnum, e_shstrndx = struct.unpack_from("<HHH", data, 0x2E)
secs = []
for i in range(e_shnum):
    o = e_shoff + i * e_shentsize
    name, typ, flags, addr, off, size, link, info, align, entsize = struct.unpack_from("<IIIIIIIIII", data, o)
    secs.append(dict(name=name, type=typ, flags=flags, addr=addr, off=off, size=size, link=link, entsize=entsize))
symtab = next((s for s in secs if s["type"] == 2), None)
assert symtab, "no symbol table: build the ARMv4 variant unstripped"
strtab = secs[symtab["link"]]
def cstr(off):
    end = data.index(b"\0", off)
    return data[off:end].decode()
# Collect mapping symbols per section: (address, kind)
marks = {}
for i in range(symtab["size"] // 16):
    st_name, st_value, st_size, st_info, st_other, st_shndx = struct.unpack_from("<IIIBBH", data, symtab["off"] + i * 16)
    if st_shndx == 0 or st_shndx >= len(secs):
        continue
    n = cstr(strtab["off"] + st_name)
    if n in ("$a", "$d", "$t") or n.startswith(("$a.", "$d.", "$t.")):
        marks.setdefault(st_shndx, []).append((st_value, n[1]))
patched = 0
for idx, s in enumerate(secs):
    if not (s["flags"] & 0x4) or s["type"] != 1:  # SHF_EXECINSTR, PROGBITS
        continue
    m = sorted(marks.get(idx, [(s["addr"], "a")]))
    for k, (start, kind) in enumerate(m):
        end = m[k + 1][0] if k + 1 < len(m) else s["addr"] + s["size"]
        if kind != "a":
            continue
        for a in range(start & ~3, end, 4):
            if a < s["addr"] or a + 4 > s["addr"] + s["size"]:
                continue
            o = s["off"] + (a - s["addr"])
            w, = struct.unpack_from("<I", data, o)
            if (w & 0x0FFFFFF0) == 0x012FFF10 and (w >> 28) != 0xF:  # bx{cond} rN
                struct.pack_into("<I", data, o, (w & 0xF000000F) | 0x01A0F000)  # mov{cond} pc, rN
                patched += 1
open(sys.argv[2], "wb").write(data)
os.chmod(sys.argv[2], os.stat(sys.argv[1]).st_mode & 0o7777)  # keep it executable
print(f"fix_v4bx: rewrote {patched} bx instruction(s)")
