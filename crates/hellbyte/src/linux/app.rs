//! Linux frontend: storage, headless modes and backend selection.

use super::nr;
use super::*;
use crate::common::args::{Args, Backend, HELP};
use crate::common::Buf;
use hellbyte_engine::game::Skill;
use hellbyte_engine::{Engine, Host};

// ------------------------------------------------------------------ storage

/// Save slots and settings as small files in the user's data directory.
pub struct FileHost {
    dir: Buf<256>,
    fd: i32,
}

impl FileHost {
    pub fn new(env: &Env) -> FileHost {
        let mut dir = Buf::new();
        if let Some(x) = env.var(b"XDG_DATA_HOME").filter(|v| !v.is_empty()) {
            dir.push(x).push(b"/hellbyte");
        } else if let Some(h) = env.var(b"HOME").filter(|v| !v.is_empty()) {
            dir.push(h).push(b"/.local/share/hellbyte");
        } else {
            dir.push(b".hellbyte");
        }
        FileHost { dir, fd: -1 }
    }

    fn path(&self, slot: u8) -> Buf<280> {
        let mut p = Buf::new();
        p.push(self.dir.as_bytes());
        if slot == hellbyte_engine::save::CONFIG_SLOT {
            p.push(b"/config.hbc");
        } else {
            p.push(b"/slot").num(slot as u64).push(b".hbs");
        }
        p
    }

    fn mkdirs(&self) {
        // mkdir -p, component by component
        let d = self.dir.as_bytes();
        for i in 1..=d.len() {
            if i == d.len() || d[i] == b'/' {
                let mut p: Buf<280> = Buf::new();
                p.push(&d[..i]);
                mkdir(p.cstr(), 0o755);
            }
        }
    }
}

impl Host for FileHost {
    fn save_begin(&mut self, slot: u8) -> bool {
        self.mkdirs();
        let mut p = self.path(slot);
        self.fd = open(p.cstr(), nr::O_WRONLY | nr::O_CREAT | nr::O_TRUNC, 0o644);
        self.fd >= 0
    }
    fn save_write(&mut self, data: &[u8]) -> bool {
        self.fd >= 0 && write_all(self.fd, data)
    }
    fn save_end(&mut self) -> bool {
        if self.fd >= 0 {
            close(self.fd);
            self.fd = -1;
            return true;
        }
        false
    }
    fn load_begin(&mut self, slot: u8) -> bool {
        let mut p = self.path(slot);
        self.fd = open(p.cstr(), nr::O_RDONLY, 0);
        self.fd >= 0
    }
    fn load_read(&mut self, buf: &mut [u8]) -> bool {
        let mut got = 0;
        while got < buf.len() {
            let n = read(self.fd, &mut buf[got..]);
            if n <= 0 {
                return false;
            }
            got += n as usize;
        }
        true
    }
    fn load_end(&mut self) {
        if self.fd >= 0 {
            close(self.fd);
            self.fd = -1;
        }
    }
}

// ------------------------------------------------------------------ helpers

pub fn out(s: &[u8]) {
    write_all(1, s);
}

pub fn err(s: &[u8]) {
    write_all(2, s);
}

pub fn out_num(v: u64) {
    let mut b: Buf<24> = Buf::new();
    b.num(v);
    out(b.as_bytes());
}

/// Peak resident memory of this process in KiB (VmHWM), if available.
pub fn peak_rss_kb() -> u64 {
    let fd = open(b"/proc/self/status\0", nr::O_RDONLY, 0);
    if fd < 0 {
        return 0;
    }
    let mut buf = [0u8; 2048];
    let n = read(fd, &mut buf);
    close(fd);
    if n <= 0 {
        return 0;
    }
    for line in buf[..n as usize].split(|&c| c == b'\n') {
        if let Some(rest) = line.strip_prefix(b"VmHWM:") {
            let digits: Buf<24> = {
                let mut d = Buf::new();
                for &c in rest {
                    if c.is_ascii_digit() {
                        d.push(&[c]);
                    }
                }
                d
            };
            return crate::common::parse_u32(digits.as_bytes()).unwrap_or(0) as u64;
        }
    }
    0
}

fn write_png(path: &[u8], e: &Engine, scale: usize) -> bool {
    let mut p: Buf<280> = Buf::new();
    p.push(path);
    let fd = open(p.cstr(), nr::O_WRONLY | nr::O_CREAT | nr::O_TRUNC, 0o644);
    if fd < 0 {
        return false;
    }
    let (px, w, h) = e.screen();
    let pal = e.palette();
    let mut buf = [0u8; 4096];
    let mut len = 0usize;
    let mut ok = true;
    hellbyte_engine::png::write_png(
        w * scale,
        h * scale,
        &mut |x, y| {
            let c = px[(y / scale) * w + x / scale] as usize;
            [pal[c * 3], pal[c * 3 + 1], pal[c * 3 + 2]]
        },
        &mut |bytes| {
            for &b in bytes {
                if len == buf.len() {
                    ok &= write_all(fd, &buf);
                    len = 0;
                }
                buf[len] = b;
                len += 1;
            }
        },
    );
    ok &= write_all(fd, &buf[..len]);
    close(fd);
    ok
}

// ------------------------------------------------------------------ entry

pub fn run(env: Env) -> i32 {
    let args = Args::parse(&|i| env.arg(i));
    if args.help {
        out(HELP);
        return 0;
    }
    if args.version {
        out(b"hellbyte ");
        out(env!("CARGO_PKG_VERSION").as_bytes());
        out(b" (GPL-3.0-or-later)\n");
        return 0;
    }
    let mut host = FileHost::new(&env);
    // SAFETY: the only call; the engine lives for the whole program.
    let e = unsafe { hellbyte_engine::instance() };

    if args.shot.is_some() || args.bench {
        let (w, h) = args.size.unwrap_or((320, 200));
        e.init(w, h, false, &mut host);
        e.new_game(Skill::from_u8(args.skill), args.map.unwrap_or(0) as usize);
        if let Some((x, y, a)) = args.warp {
            e.debug_warp(x, y, a);
        }
        if let Some(w) = args.show {
            e.debug_show(w);
        }
        let t0 = now_us();
        let mut tics = 0;
        if let Some(s) = args.script {
            tics += crate::common::script::run(e, &mut host, s);
        }
        let mut draws = 0u32;
        while tics < args.tics {
            e.tick(&mut host);
            tics += 1;
            if args.bench {
                e.draw();
                draws += 1;
            }
        }
        e.draw();
        let dt = now_us() - t0;
        if let Some(path) = args.shot {
            if !write_png(path, e, if args.scale > 0 { args.scale as usize } else { 1 }) {
                err(b"hellbyte: could not write screenshot\n");
                return 1;
            }
        }
        if args.bench {
            out(b"tics ");
            out_num(tics as u64);
            out(b"  frames ");
            out_num(draws as u64 + 1);
            out(b"  total ");
            out_num(dt / 1000);
            out(b" ms  per frame ");
            out_num(dt / (draws as u64 + 1));
            out(b" us  peak RSS ");
            out_num(peak_rss_kb());
            out(b" KiB  engine state ");
            out_num((core::mem::size_of::<hellbyte_engine::Engine>() / 1024) as u64);
            out(b" KiB  hash ");
            let mut hx: Buf<12> = Buf::new();
            let h = e.state_hash();
            for i in (0..8).rev() {
                hx.push(&[b"0123456789abcdef"[((h >> (i * 4)) & 15) as usize]]);
            }
            out(hx.as_bytes());
            out(b"\n");
        }
        return 0;
    }

    let backend = match args.backend {
        Backend::Auto => {
            if env.var(b"DISPLAY").is_some_and(|d| !d.is_empty()) {
                Backend::X11
            } else if super::fb::available() && !env.var(b"SSH_CONNECTION").is_some_and(|v| !v.is_empty()) {
                Backend::Fb
            } else {
                Backend::Term
            }
        }
        b => b,
    };
    let code = match backend {
        Backend::X11 | Backend::Window => match super::x11::run(e, &mut host, &env, &args) {
            Ok(c) => c,
            Err(msg) => {
                err(b"hellbyte: X11: ");
                err(msg);
                err(b"; falling back to the terminal\n");
                super::term::run(e, &mut host, &env, &args)
            }
        },
        Backend::Fb => match super::fb::run(e, &mut host, &env, &args) {
            Ok(c) => c,
            Err(msg) => {
                err(b"hellbyte: framebuffer: ");
                err(msg);
                err(b"; falling back to the terminal\n");
                super::term::run(e, &mut host, &env, &args)
            }
        },
        _ => super::term::run(e, &mut host, &env, &args),
    };
    code
}
