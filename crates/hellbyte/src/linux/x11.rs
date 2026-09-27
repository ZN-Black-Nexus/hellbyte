//! X11 window backend speaking the X protocol directly over a socket
//! (no Xlib / libxcb). Also works under XWayland and over `ssh -X`.

use super::nr;
use super::*;
use crate::common::args::Args;
use crate::common::Buf;
use hellbyte_engine::keys::*;
use hellbyte_engine::{Engine, Host};

const EV_KEYPRESS: u32 = 1;
const EV_KEYRELEASE: u32 = 2;
const EV_BUTTONPRESS: u32 = 4;
const EV_BUTTONRELEASE: u32 = 8;
const EV_POINTERMOTION: u32 = 0x40;
const EV_EXPOSURE: u32 = 0x8000;
const EV_STRUCTURE: u32 = 0x20000;
const EV_FOCUS: u32 = 0x200000;

/// Connection state and a small request buffer.
struct X {
    fd: i32,
    rid_base: u32,
    rid_next: u32,
    root: u32,
    visual: u32,
    depth: u8,
    bpp: u8,
    max_req_bytes: usize,
    lsb_first: bool,
    rmask: u32,
    gmask: u32,
    bmask: u32,
    min_kc: u8,
    max_kc: u8,
    keymap: [u16; 256],
    buf: [u8; 1024],
    len: usize,
    inbuf: [u8; 8192],
    inlen: usize,
}

static mut XS: X = X {
    fd: -1,
    rid_base: 0,
    rid_next: 1,
    root: 0,
    visual: 0,
    depth: 24,
    bpp: 32,
    max_req_bytes: 262140,
    lsb_first: true,
    rmask: 0xff0000,
    gmask: 0xff00,
    bmask: 0xff,
    min_kc: 8,
    max_kc: 255,
    keymap: [0; 256],
    buf: [0; 1024],
    len: 0,
    inbuf: [0; 8192],
    inlen: 0,
};

// Row buffer for image upload: up to 1920 pixels * 4 bytes.
static mut ROW: [u8; 1920 * 4] = [0; 1920 * 4];

fn ne16(v: u16) -> [u8; 2] {
    v.to_ne_bytes()
}
fn ne32(v: u32) -> [u8; 4] {
    v.to_ne_bytes()
}
fn rd16(b: &[u8], o: usize) -> u16 {
    u16::from_ne_bytes([b[o], b[o + 1]])
}
fn rd32(b: &[u8], o: usize) -> u32 {
    u32::from_ne_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

impl X {
    fn id(&mut self) -> u32 {
        let v = self.rid_base | self.rid_next;
        self.rid_next += 1;
        v
    }
    fn put(&mut self, b: &[u8]) {
        for &c in b {
            if self.len == self.buf.len() {
                self.flush();
            }
            self.buf[self.len] = c;
            self.len += 1;
        }
    }
    fn p8(&mut self, v: u8) {
        self.put(&[v]);
    }
    fn p16(&mut self, v: u16) {
        self.put(&ne16(v));
    }
    fn p32(&mut self, v: u32) {
        self.put(&ne32(v));
    }
    fn pad(&mut self, n: usize) {
        for _ in 0..(4 - n % 4) % 4 {
            self.p8(0);
        }
    }
    fn flush(&mut self) {
        if self.len > 0 {
            write_all(self.fd, &self.buf[..self.len]);
            self.len = 0;
        }
    }

    /// Read exactly `n` bytes (blocking) into `out`.
    fn read_exact(&mut self, out: &mut [u8]) -> bool {
        let mut got = 0;
        while got < out.len() {
            // Serve from the input buffer first.
            if self.inlen > 0 {
                let k = self.inlen.min(out.len() - got);
                out[got..got + k].copy_from_slice(&self.inbuf[..k]);
                self.inbuf.copy_within(k..self.inlen, 0);
                self.inlen -= k;
                got += k;
                continue;
            }
            let n = read(self.fd, &mut out[got..]);
            if n <= 0 {
                if n == -nr::EINTR || n == -nr::EAGAIN {
                    continue;
                }
                return false;
            }
            got += n as usize;
        }
        true
    }

    /// Wait for the reply to the last request, queueing events that arrive first.
    fn reply(&mut self, out: &mut [u8], events: &mut EventQueue) -> Option<usize> {
        loop {
            let mut head = [0u8; 32];
            if !self.read_exact(&mut head) {
                return None;
            }
            match head[0] {
                0 => return None, // error
                1 => {
                    let extra = rd32(&head, 4) as usize * 4;
                    let n = (32 + extra).min(out.len());
                    out[..32].copy_from_slice(&head);
                    let mut rest = extra;
                    let mut pos = 32;
                    let mut tmp = [0u8; 256];
                    while rest > 0 {
                        let k = rest.min(256);
                        if !self.read_exact(&mut tmp[..k]) {
                            return None;
                        }
                        for (i, &b) in tmp[..k].iter().enumerate() {
                            if pos + i < out.len() {
                                out[pos + i] = b;
                            }
                        }
                        pos += k;
                        rest -= k;
                    }
                    return Some(n);
                }
                _ => events.push(&head),
            }
        }
    }

    fn intern(&mut self, name: &[u8], ev: &mut EventQueue) -> u32 {
        self.p8(16);
        self.p8(0);
        self.p16(((8 + name.len() + 3) / 4) as u16);
        self.p16(name.len() as u16);
        self.p16(0);
        self.put(name);
        self.pad(name.len());
        self.flush();
        let mut r = [0u8; 32];
        match self.reply(&mut r, ev) {
            Some(_) => rd32(&r, 8),
            None => 0,
        }
    }

    fn change_property(&mut self, win: u32, prop: u32, ty: u32, format: u8, data: &[u8]) {
        let n = data.len();
        self.p8(18);
        self.p8(0); // Replace
        self.p16((6 + (n + 3) / 4) as u16);
        self.p32(win);
        self.p32(prop);
        self.p32(ty);
        self.p8(format);
        self.put(&[0, 0, 0]);
        self.p32((n / (format as usize / 8)) as u32);
        self.put(data);
        self.pad(n);
    }

    fn load_keymap(&mut self, ev: &mut EventQueue) {
        let count = (self.max_kc as usize - self.min_kc as usize + 1).min(248);
        self.p8(101);
        self.p8(0);
        self.p16(2);
        self.p8(self.min_kc);
        self.p8(count as u8);
        self.p16(0);
        self.flush();
        // Reply can be large (count * per * 4); read header then stream.
        let mut head = [0u8; 32];
        loop {
            if !self.read_exact(&mut head) {
                return;
            }
            if head[0] == 1 {
                break;
            }
            if head[0] == 0 {
                return;
            }
            ev.push(&head);
        }
        let per = head[1] as usize;
        let total = rd32(&head, 4) as usize * 4;
        let mut sym = [0u8; 4];
        for i in 0..total / 4 {
            if !self.read_exact(&mut sym) {
                return;
            }
            let kc = self.min_kc as usize + i / per.max(1);
            if i % per.max(1) == 0 && kc < 256 {
                self.keymap[kc] = keysym_to_key(u32::from_ne_bytes(sym));
            }
        }
    }
}

/// Small queue for events that arrive while waiting for replies.
struct EventQueue {
    q: [[u8; 32]; 16],
    n: usize,
}

impl EventQueue {
    fn push(&mut self, e: &[u8; 32]) {
        if self.n < 16 {
            self.q[self.n] = *e;
            self.n += 1;
        }
    }
}

fn keysym_to_key(s: u32) -> u16 {
    match s {
        0xff51 | 0xff96 => KEY_LEFT,
        0xff52 | 0xff97 => KEY_UP,
        0xff53 | 0xff98 => KEY_RIGHT,
        0xff54 | 0xff99 => KEY_DOWN,
        0xff0d | 0xff8d => KEY_ENTER,
        0xff1b => KEY_ESCAPE,
        0xff09 => KEY_TAB,
        0xff08 => KEY_BACKSPACE,
        0xffe1 | 0xffe2 => KEY_SHIFT,
        0xffe3 | 0xffe4 => KEY_CTRL,
        0xffe9 | 0xffea | 0xffe7 | 0xffe8 | 0xfe03 => KEY_ALT,
        0xffbe..=0xffc9 => KEY_F1 + (s - 0xffbe) as u16,
        0xff13 => KEY_PAUSE,
        0xff50 => KEY_HOME,
        0xff57 => KEY_END,
        0xff55 => KEY_PGUP,
        0xff56 => KEY_PGDN,
        0xff63 => KEY_INSERT,
        0xffff => KEY_DELETE,
        0xffab => b'=' as u16,
        0xffad => b'-' as u16,
        0x20..=0x7e => (s as u8).to_ascii_lowercase() as u16,
        _ => 0,
    }
}

fn parse_display(d: &[u8]) -> Option<(Option<&[u8]>, u32)> {
    let colon = d.iter().rposition(|&c| c == b':')?;
    let host = &d[..colon];
    let rest = &d[colon + 1..];
    let num_end = rest.iter().position(|&c| c == b'.').unwrap_or(rest.len());
    let n = crate::common::parse_u32(&rest[..num_end])?;
    let host = if host.is_empty() || host == b"unix" { None } else { Some(host) };
    Some((host, n))
}

fn connect_display(host: Option<&[u8]>, n: u32) -> i32 {
    match host {
        None => {
            // Abstract socket first (what most Linux X servers listen on), then the file.
            for abstract_ns in [true, false] {
                let fd = socket(nr::AF_UNIX, nr::SOCK_STREAM);
                if fd < 0 {
                    return fd;
                }
                let mut addr = [0u8; 110];
                addr[..2].copy_from_slice(&(nr::AF_UNIX as u16).to_ne_bytes());
                let mut path: Buf<108> = Buf::new();
                path.push(b"/tmp/.X11-unix/X").num(n as u64);
                let off = if abstract_ns { 3 } else { 2 };
                addr[off..off + path.len].copy_from_slice(path.as_bytes());
                let len = off + path.len + if abstract_ns { 0 } else { 1 };
                if connect(fd, &addr[..len]) == 0 {
                    return fd;
                }
                close(fd);
            }
            -1
        }
        Some(h) => {
            // TCP (e.g. ssh -X gives localhost:10.0). Numeric IPv4 or localhost only.
            let ip: [u8; 4] = if h == b"localhost" {
                [127, 0, 0, 1]
            } else {
                let mut ip = [0u8; 4];
                let mut i = 0;
                for part in h.split(|&c| c == b'.') {
                    if i == 4 {
                        return -1;
                    }
                    ip[i] = crate::common::parse_u32(part).unwrap_or(0) as u8;
                    i += 1;
                }
                if i != 4 {
                    return -1;
                }
                ip
            };
            let fd = socket(nr::AF_INET, nr::SOCK_STREAM);
            if fd < 0 {
                return fd;
            }
            let mut addr = [0u8; 16];
            addr[..2].copy_from_slice(&(nr::AF_INET as u16).to_ne_bytes());
            addr[2..4].copy_from_slice(&((6000 + n) as u16).to_be_bytes());
            addr[4..8].copy_from_slice(&ip);
            if connect(fd, &addr) == 0 {
                fd
            } else {
                close(fd);
                -1
            }
        }
    }
}

/// Find a MIT-MAGIC-COOKIE-1 for display `n` in the Xauthority file.
fn find_cookie(env: &Env, n: u32, cookie: &mut [u8; 16]) -> bool {
    let mut path: Buf<280> = Buf::new();
    match env.var(b"XAUTHORITY") {
        Some(p) if !p.is_empty() => {
            path.push(p);
        }
        _ => {
            path.push(env.var(b"HOME").unwrap_or(b"")).push(b"/.Xauthority");
        }
    }
    let fd = open(path.cstr(), nr::O_RDONLY, 0);
    if fd < 0 {
        return false;
    }
    let mut data = [0u8; 4096];
    let len = read(fd, &mut data);
    close(fd);
    if len <= 0 {
        return false;
    }
    let data = &data[..len as usize];
    let mut num: Buf<12> = Buf::new();
    num.num(n as u64);
    let mut p = 0;
    let field = |p: &mut usize| -> Option<&[u8]> {
        if *p + 2 > data.len() {
            return None;
        }
        let l = u16::from_be_bytes([data[*p], data[*p + 1]]) as usize;
        *p += 2;
        if *p + l > data.len() {
            return None;
        }
        let f = &data[*p..*p + l];
        *p += l;
        Some(f)
    };
    let mut fallback = false;
    while p + 2 <= data.len() {
        p += 2; // family
        let (Some(_addr), Some(number), Some(name), Some(cdata)) = (field(&mut p), field(&mut p), field(&mut p), field(&mut p)) else {
            break;
        };
        if name == b"MIT-MAGIC-COOKIE-1" && cdata.len() == 16 {
            if number == num.as_bytes() {
                cookie.copy_from_slice(cdata);
                return true;
            }
            if !fallback {
                cookie.copy_from_slice(cdata);
                fallback = true;
            }
        }
    }
    fallback
}

fn setup(env: &Env, x: &mut X) -> Result<(), &'static [u8]> {
    let disp = env.var(b"DISPLAY").ok_or(b"DISPLAY is not set" as &[u8])?;
    let (host, n) = parse_display(disp).ok_or(b"cannot parse DISPLAY" as &[u8])?;
    x.fd = connect_display(host, n);
    if x.fd < 0 {
        return Err(b"cannot connect to the X server");
    }
    let mut cookie = [0u8; 16];
    let have = find_cookie(env, n, &mut cookie);
    x.p8(if cfg!(target_endian = "little") { b'l' } else { b'B' });
    x.p8(0);
    x.p16(11);
    x.p16(0);
    let name: &[u8] = if have { b"MIT-MAGIC-COOKIE-1" } else { b"" };
    x.p16(name.len() as u16);
    x.p16(if have { 16 } else { 0 });
    x.p16(0);
    x.put(name);
    x.pad(name.len());
    if have {
        x.put(&cookie);
    }
    x.flush();
    let mut head = [0u8; 8];
    if !x.read_exact(&mut head) {
        return Err(b"X server closed the connection");
    }
    let extra = rd16(&head, 6) as usize * 4;
    if head[0] != 1 {
        return Err(b"X server refused the connection (authorization?)");
    }
    // The setup data can be large on servers with many visuals; parse as it streams.
    static mut SETUP: [u8; 16384] = [0; 16384];
    // SAFETY: single-threaded, used only during setup.
    let s = unsafe { &mut *core::ptr::addr_of_mut!(SETUP) };
    let keep = extra.min(s.len());
    if !x.read_exact(&mut s[..keep]) {
        return Err(b"short X setup reply");
    }
    let mut skip = extra - keep;
    let mut tmp = [0u8; 256];
    while skip > 0 {
        let k = skip.min(256);
        x.read_exact(&mut tmp[..k]);
        skip -= k;
    }
    x.rid_base = rd32(s, 4);
    let vendor_len = rd16(s, 16) as usize;
    let max_req = rd16(s, 18) as usize;
    x.max_req_bytes = (max_req * 4).max(4096);
    let nformats = s[21] as usize;
    x.lsb_first = s[22] == 0;
    x.min_kc = s[26];
    x.max_kc = s[27];
    let mut p = 32 + ((vendor_len + 3) & !3);
    let fmt_start = p;
    p += nformats * 8;
    // first screen
    x.root = rd32(s, p);
    x.visual = rd32(s, p + 32);
    x.depth = s[p + 38];
    let ndepths = s[p + 39] as usize;
    let mut q = p + 40;
    let mut found = false;
    for _ in 0..ndepths {
        let d = s[q];
        let nv = rd16(s, q + 2) as usize;
        q += 8;
        for v in 0..nv {
            let o = q + v * 24;
            if o + 24 > keep {
                break;
            }
            if rd32(s, o) == x.visual && d == x.depth {
                x.rmask = rd32(s, o + 8);
                x.gmask = rd32(s, o + 12);
                x.bmask = rd32(s, o + 16);
                found = s[o + 4] == 4 || s[o + 4] == 5; // TrueColor / DirectColor
            }
        }
        q += nv * 24;
    }
    if !found || (x.depth != 24 && x.depth != 16 && x.depth != 32 && x.depth != 15) {
        return Err(b"no TrueColor visual on the default screen");
    }
    for f in 0..nformats {
        let o = fmt_start + f * 8;
        if s[o] == x.depth {
            x.bpp = s[o + 1];
        }
    }
    Ok(())
}

fn shift_of(mask: u32) -> (u32, u32) {
    if mask == 0 {
        return (0, 0);
    }
    let shift = mask.trailing_zeros();
    let bits = (mask >> shift).count_ones();
    (shift, bits)
}

pub fn run(e: &mut Engine, host: &mut dyn Host, env: &Env, args: &Args) -> Result<i32, &'static [u8]> {
    // SAFETY: single-threaded; the X state is used only here.
    let x = unsafe { &mut *core::ptr::addr_of_mut!(XS) };
    setup(env, x)?;
    let mut ev = EventQueue { q: [[0; 32]; 16], n: 0 };
    x.load_keymap(&mut ev);
    let wm_protocols = x.intern(b"WM_PROTOCOLS", &mut ev);
    let wm_delete = x.intern(b"WM_DELETE_WINDOW", &mut ev);
    let net_wm_name = x.intern(b"_NET_WM_NAME", &mut ev);
    let utf8 = x.intern(b"UTF8_STRING", &mut ev);
    let net_state = x.intern(b"_NET_WM_STATE", &mut ev);
    let net_full = x.intern(b"_NET_WM_STATE_FULLSCREEN", &mut ev);

    let scale = if args.scale > 0 { args.scale as usize } else { 3 };
    let (mut ww, mut wh) = (320 * scale, 240 * scale);
    let win = x.id();
    x.p8(1);
    x.p8(x.depth);
    x.p16(10);
    x.p32(win);
    x.p32(x.root);
    x.p16(0);
    x.p16(0);
    x.p16(ww as u16);
    x.p16(wh as u16);
    x.p16(0);
    x.p16(1);
    x.p32(x.visual);
    x.p32(0x2 | 0x800); // BackPixel | EventMask
    x.p32(0);
    x.p32(EV_KEYPRESS | EV_KEYRELEASE | EV_BUTTONPRESS | EV_BUTTONRELEASE | EV_POINTERMOTION | EV_EXPOSURE | EV_STRUCTURE | EV_FOCUS);
    let title = b"Hellbyte";
    x.change_property(win, 39, 31, 8, title); // WM_NAME, STRING
    x.change_property(win, net_wm_name, utf8, 8, title);
    x.change_property(win, 67, 31, 8, b"hellbyte\0Hellbyte\0"); // WM_CLASS
    let mut protos = [0u8; 4];
    protos.copy_from_slice(&ne32(wm_delete));
    x.change_property(win, wm_protocols, 4, 32, &protos); // type ATOM
    if args.fullscreen {
        let mut st = [0u8; 4];
        st.copy_from_slice(&ne32(net_full));
        x.change_property(win, net_state, 4, 32, &st);
    }
    // Invisible cursor for mouse look: a 1x1 pixmap with an empty mask.
    let pix = x.id();
    x.p8(53);
    x.p8(1);
    x.p16(4);
    x.p32(pix);
    x.p32(win);
    x.p16(1);
    x.p16(1);
    let cursor = x.id();
    x.p8(93);
    x.p8(0);
    x.p16(8);
    x.p32(cursor);
    x.p32(pix);
    x.p32(pix);
    for _ in 0..6 {
        x.p16(0);
    }
    x.p16(0);
    x.p16(0);
    let gc = x.id();
    x.p8(55);
    x.p8(0);
    x.p16(4);
    x.p32(gc);
    x.p32(win);
    x.p32(0);
    x.p8(8); // MapWindow
    x.p8(0);
    x.p16(2);
    x.p32(win);
    x.flush();

    e.init(320, 200, false, host);
    if let Some(m) = args.map {
        e.new_game(hellbyte_engine::game::Skill::from_u8(args.skill), m as usize);
    }

    let (rs, rb) = shift_of(x.rmask);
    let (gs, gb) = shift_of(x.gmask);
    let (bs, bb) = shift_of(x.bmask);
    let pack = |r: u8, g: u8, b: u8| -> u32 {
        ((r as u32 >> (8 - rb.min(8))) << rs) | ((g as u32 >> (8 - gb.min(8))) << gs) | ((b as u32 >> (8 - bb.min(8))) << bs)
    };
    let mut pal32 = [0u32; 256];
    let mut pal_hash = 0u32;
    let bytespp = (x.bpp as usize / 8).max(1);

    let tic_us: u64 = 1_000_000 / 35;
    let mut next_tic = now_us();
    let mut grabbed = false;
    let mut focused = true;
    let mut dirty = true;
    let mut quit = false;
    let (mut cx, mut cy) = ((ww / 2) as i32, (wh / 2) as i32);
    while !quit && !e.quit_requested() {
        // ---- events
        let avail = bytes_available(x.fd);
        let mut process = |x: &mut X, ev: &[u8; 32], e: &mut Engine| -> bool {
            match ev[0] & 0x7f {
                2 | 3 => {
                    let k = x.keymap[ev[1] as usize];
                    if k != 0 {
                        e.key(k, ev[0] & 0x7f == 2);
                    }
                }
                4 | 5 => {
                    let down = ev[0] & 0x7f == 4;
                    match ev[1] {
                        1 => e.mouse_button(0, down),
                        2 => e.mouse_button(2, down),
                        3 => e.mouse_button(1, down),
                        4 if down => e.key(KEY_WHEELUP, true),
                        5 if down => e.key(KEY_WHEELDOWN, true),
                        _ => {}
                    }
                }
                6 => {
                    let mx = rd16(ev, 24) as i16 as i32;
                    let my = rd16(ev, 26) as i16 as i32;
                    if grabbed && (mx != cx || my != cy) {
                        e.mouse_motion((mx - cx) * 2, my - cy);
                        // WarpPointer back to the centre.
                        x.p8(41);
                        x.p8(0);
                        x.p16(6);
                        x.p32(0);
                        x.p32(win);
                        x.p32(0);
                        x.p16(0);
                        x.p16(0);
                        x.p16(cx as u16);
                        x.p16(cy as u16);
                    }
                }
                9 => {
                    focused = true;
                }
                10 => {
                    focused = false;
                    e.release_all();
                }
                12 => dirty = true,
                22 => {
                    ww = rd16(ev, 20) as usize;
                    wh = rd16(ev, 22) as usize;
                    cx = (ww / 2) as i32;
                    cy = (wh / 2) as i32;
                    dirty = true;
                }
                33 => {
                    if rd32(ev, 8) == wm_protocols || rd32(ev, 12) == wm_delete {
                        return true;
                    }
                }
                34 => x.load_keymap(&mut EventQueue { q: [[0; 32]; 16], n: 0 }),
                _ => {}
            }
            false
        };
        for i in 0..ev.n {
            let q = ev.q[i];
            quit |= process(x, &q, e);
        }
        ev.n = 0;
        if avail > 0 {
            let mut remaining = avail;
            while remaining >= 32 {
                let mut evb = [0u8; 32];
                if !x.read_exact(&mut evb) {
                    quit = true;
                    break;
                }
                remaining -= 32;
                if evb[0] == 1 {
                    // stray reply: skip its payload
                    let extra = rd32(&evb, 4) as usize * 4;
                    let mut tmp = [0u8; 256];
                    let mut left = extra;
                    while left > 0 {
                        let k = left.min(256);
                        x.read_exact(&mut tmp[..k]);
                        left -= k;
                    }
                    remaining = remaining.saturating_sub(extra);
                    continue;
                }
                if evb[0] != 0 {
                    quit |= process(x, &evb, e);
                }
            }
        }
        // ---- pointer grab follows the game state
        let want = e.wants_pointer() && focused && !args.nomouse;
        if want != grabbed {
            if want {
                x.p8(26); // GrabPointer
                x.p8(0);
                x.p16(6);
                x.p32(win);
                x.p16((EV_POINTERMOTION | EV_BUTTONPRESS | EV_BUTTONRELEASE) as u16);
                x.p8(1);
                x.p8(1);
                x.p32(win);
                x.p32(cursor);
                x.p32(0);
                x.p8(41); // WarpPointer to centre
                x.p8(0);
                x.p16(6);
                x.p32(0);
                x.p32(win);
                x.p32(0);
                x.p16(0);
                x.p16(0);
                x.p16(cx as u16);
                x.p16(cy as u16);
            } else {
                x.p8(27); // UngrabPointer
                x.p8(0);
                x.p16(2);
                x.p32(0);
            }
            grabbed = want;
        }

        // ---- simulate
        let now = now_us();
        let mut steps = 0;
        while now >= next_tic && steps < 4 {
            e.tick(host);
            next_tic += tic_us;
            steps += 1;
        }
        if now > next_tic + tic_us * 8 {
            next_tic = now;
        }

        // ---- draw: scale 320x200 to the window at 4:3 with letterboxing
        if steps > 0 || dirty {
            dirty = false;
            e.draw();
            let pal = e.palette();
            let mut h = 0x811c_9dc5u32;
            for &b in pal.iter() {
                h = (h ^ b as u32).wrapping_mul(0x0100_0193);
            }
            if h != pal_hash {
                pal_hash = h;
                for i in 0..256 {
                    pal32[i] = pack(pal[i * 3], pal[i * 3 + 1], pal[i * 3 + 2]);
                }
            }
            let (px, sw, sh) = e.screen();
            let (dw, dh) = if ww * 3 > wh * 4 { (wh * 4 / 3, wh) } else { (ww, ww * 3 / 4) };
            let dw = dw.clamp(1, 1920);
            let dh = dh.max(1);
            let ox = (ww.saturating_sub(dw)) / 2;
            let oy = (wh.saturating_sub(dh)) / 2;
            let row_bytes = (dw * bytespp + 3) & !3;
            let rows_per_req = ((x.max_req_bytes - 24) / row_bytes).clamp(1, 512);
            // SAFETY: single-threaded scratch row.
            let row = unsafe { &mut *core::ptr::addr_of_mut!(ROW) };
            let mut y = 0;
            while y < dh {
                let n = rows_per_req.min(dh - y);
                x.p8(72); // PutImage, ZPixmap
                x.p8(2);
                x.p16(((24 + row_bytes * n) / 4) as u16);
                x.p32(win);
                x.p32(gc);
                x.p16(dw as u16);
                x.p16(n as u16);
                x.p16(ox as u16);
                x.p16((oy + y) as u16);
                x.p8(0);
                x.p8(x.depth);
                x.p16(0);
                x.flush();
                for r in 0..n {
                    let sy = ((y + r) * sh / dh).min(sh - 1);
                    let src = &px[sy * sw..sy * sw + sw];
                    for dx in 0..dw {
                        let c = pal32[src[dx * sw / dw] as usize];
                        let o = dx * bytespp;
                        let bytes = if x.lsb_first { c.to_le_bytes() } else { c.to_be_bytes() };
                        if bytespp == 4 {
                            row[o..o + 4].copy_from_slice(&bytes);
                        } else if bytespp == 2 {
                            let v = if x.lsb_first { (c as u16).to_le_bytes() } else { (c as u16).to_be_bytes() };
                            row[o..o + 2].copy_from_slice(&v);
                        } else {
                            row[o..o + bytespp].copy_from_slice(&bytes[..bytespp]);
                        }
                    }
                    write_all(x.fd, &row[..row_bytes]);
                }
                y += n;
            }
            x.flush();
        }
        let wait = next_tic.saturating_sub(now_us());
        if wait > 0 {
            sleep_us(wait.min(4_000));
        }
    }
    close(x.fd);
    Ok(0)
}
