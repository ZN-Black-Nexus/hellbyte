#!/usr/bin/env python3
"""Run hellbyte in a pseudo-terminal, feed it keys, and render what a terminal
would show to a PNG (half-block pixels and text cells). Standard library only.

usage: termshot.py OUT.png COLSxROWS SECONDS "keys" -- ./hellbyte --term [args]
keys: a string of characters; '|' waits 0.25 s, '~' sends Enter,
      '^' sends Up, 'v' Down, '<' Left, '>' Right, 'E' Escape.
"""
import os, pty, select, struct, subprocess, sys, termios, fcntl, time, zlib

def xterm_rgb(i):
    base = [(0,0,0),(170,0,0),(0,170,0),(170,85,0),(0,0,170),(170,0,170),(0,170,170),(170,170,170),
            (85,85,85),(255,85,85),(85,255,85),(255,255,85),(85,85,255),(255,85,255),(85,255,255),(255,255,255)]
    if i < 16: return base[i]
    if i < 232:
        v = i - 16; l = lambda x: 0 if x == 0 else 55 + 40 * x
        return (l(v // 36), l((v // 6) % 6), l(v % 6))
    g = 8 + 10 * (i - 232); return (g, g, g)

def png(path, w, h, px):
    raw = b"".join(b"\0" + bytes(px[y*w*3:(y+1)*w*3]) for y in range(h))
    def chunk(t, d): return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    open(path, "wb").write(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
                           + chunk(b"IDAT", zlib.compress(raw, 6)) + chunk(b"IEND", b""))

def main():
    out, size, secs, keys = sys.argv[1], sys.argv[2], float(sys.argv[3]), sys.argv[4]
    cmd = sys.argv[sys.argv.index("--") + 1:]
    cols, rows = map(int, size.lower().split("x"))
    master, slave = pty.openpty()
    fcntl.ioctl(slave, termios.TIOCSWINSZ, struct.pack("HHHH", rows, cols, 0, 0))
    env = dict(os.environ, TERM="xterm-256color")
    env.pop("DISPLAY", None)
    p = subprocess.Popen(cmd, stdin=slave, stdout=slave, stderr=slave, env=env, start_new_session=True)
    os.close(slave)
    data = bytearray()
    t0 = time.time()
    kq = list(keys)
    next_key = t0 + 0.6
    while time.time() - t0 < secs:
        r, _, _ = select.select([master], [], [], 0.02)
        if r:
            try: data += os.read(master, 65536)
            except OSError: break
        if kq and time.time() >= next_key:
            k = kq.pop(0)
            m = {"~": b"\r", "^": b"\x1b[A", "v": b"\x1b[B", "<": b"\x1b[D", ">": b"\x1b[C", "E": b"\x1b"}
            if k != "|": os.write(master, m.get(k, k.encode()))
            next_key = time.time() + 0.25
    os.write(master, b"\x03")
    try: p.wait(timeout=2)
    except subprocess.TimeoutExpired: p.kill()
    # --- emulate
    top = [[(0,0,0)] * cols for _ in range(rows)]
    bot = [[(0,0,0)] * cols for _ in range(rows)]
    txt = [[None] * cols for _ in range(rows)]
    fg, bg = (200,200,200), (0,0,0)
    cr, cc = 0, 0
    s = data.decode("utf-8", "replace")
    i = 0
    while i < len(s):
        ch = s[i]
        if ch == "\x1b" and i + 1 < len(s) and s[i+1] == "[":
            j = i + 2
            while j < len(s) and not ("\x40" <= s[j] <= "\x7e"): j += 1
            if j >= len(s): break
            body, fin = s[i+2:j], s[j]
            i = j + 1
            if body.startswith(("?", ">", "<")): continue
            ps = [int(x) if x.isdigit() else 0 for x in body.split(";")] if body else []
            if fin == "H":
                cr = (ps[0] if ps else 1) - 1; cc = (ps[1] if len(ps) > 1 else 1) - 1
            elif fin == "J":
                for r_ in range(rows):
                    top[r_] = [(0,0,0)] * cols; bot[r_] = [(0,0,0)] * cols; txt[r_] = [None] * cols
            elif fin == "K":
                for c_ in range(cc, cols): top[cr][c_] = bot[cr][c_] = bg; txt[cr][c_] = None
            elif fin == "m":
                k = 0
                if not ps: ps = [0]
                while k < len(ps):
                    v = ps[k]
                    if v == 0: fg, bg = (200,200,200), (0,0,0)
                    elif v in (38, 48) and k + 2 < len(ps) and ps[k+1] == 5:
                        c = xterm_rgb(ps[k+2]); fg, bg = (c, bg) if v == 38 else (fg, c); k += 2
                    elif v in (38, 48) and k + 4 < len(ps) and ps[k+1] == 2:
                        c = tuple(ps[k+2:k+5]); fg, bg = (c, bg) if v == 38 else (fg, c); k += 4
                    elif 30 <= v <= 37: fg = xterm_rgb(v - 30)
                    elif 90 <= v <= 97: fg = xterm_rgb(v - 90 + 8)
                    elif 40 <= v <= 47: bg = xterm_rgb(v - 40)
                    elif 100 <= v <= 107: bg = xterm_rgb(v - 100 + 8)
                    k += 1
            continue
        if ch == "\x1b": i += 2; continue
        if ch in "\r\n": i += 1; continue
        if 0 <= cr < rows and 0 <= cc < cols:
            if ch == "▀": top[cr][cc], bot[cr][cc], txt[cr][cc] = fg, bg, None
            elif ch == " ": top[cr][cc] = bot[cr][cc] = bg; txt[cr][cc] = None
            else: top[cr][cc], bot[cr][cc], txt[cr][cc] = bg, bg, fg
        cc += 1; i += 1
    S = 4  # each cell -> 4x8 pixels
    W, H = cols * S, rows * 2 * S
    px = bytearray(W * H * 3)
    for r_ in range(rows):
        for c_ in range(cols):
            for half, col in ((0, top[r_][c_]), (1, bot[r_][c_])):
                for y in range(S):
                    for x in range(S):
                        o = (((r_ * 2 + half) * S + y) * W + c_ * S + x) * 3
                        px[o:o+3] = bytes(col)
            if txt[r_][c_]:  # text: a light mark so text positions are visible
                for y in range(2, 2 * S - 2):
                    o = ((r_ * 2 * S + y) * W + c_ * S + 1) * 3
                    px[o:o+6] = bytes(txt[r_][c_]) * 2
    png(out, W, H, px)
    print(f"{len(data)} bytes of terminal output; exit={p.returncode}")

main()
