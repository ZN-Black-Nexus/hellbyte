//! macOS platform layer. Links only libSystem (the one library every macOS
//! program must use); the Cocoa window backend loads AppKit at run time.

pub mod cocoa;

use crate::common::args::{Args, Backend, HELP};
use crate::common::tui::{Colors, HeldKeys, KeyDecoder, Tui};
use crate::common::Buf;
use hellbyte_engine::game::Skill;
use hellbyte_engine::keys::*;
use hellbyte_engine::{Engine, Host};

#[link(name = "System")]
unsafe extern "C" {
    fn read(fd: i32, buf: *mut u8, n: usize) -> isize;
    fn write(fd: i32, buf: *const u8, n: usize) -> isize;
    // Variadic in C: on Apple ARM64 variadic arguments go on the stack, so
    // these must be declared variadic here too.
    fn open(path: *const u8, flags: i32, ...) -> i32;
    fn ioctl(fd: i32, req: u64, ...) -> i32;
    fn close(fd: i32) -> i32;
    fn mkdir(path: *const u8, mode: u16) -> i32;
    fn tcgetattr(fd: i32, t: *mut u8) -> i32;
    fn tcsetattr(fd: i32, act: i32, t: *const u8) -> i32;
    fn usleep(us: u32) -> i32;
    fn mach_absolute_time() -> u64;
    fn mach_timebase_info(info: *mut [u32; 2]) -> i32;
    pub fn dlopen(path: *const u8, mode: i32) -> *mut u8;
    pub fn dlsym(handle: *mut u8, name: *const u8) -> *mut u8;
    fn exit(code: i32) -> !;
}

const O_RDONLY: i32 = 0;
const O_WRONLY: i32 = 1;
const O_CREAT: i32 = 0x200;
const O_TRUNC: i32 = 0x400;
const TIOCGWINSZ: u64 = 0x4008_7468;

/// Exit the process now.
pub fn quit(code: i32) -> ! {
    unsafe { exit(code) }
}

pub fn write_all(fd: i32, mut b: &[u8]) -> bool {
    while !b.is_empty() {
        let n = unsafe { write(fd, b.as_ptr(), b.len()) };
        if n <= 0 {
            return false;
        }
        b = &b[n as usize..];
    }
    true
}

pub fn now_us() -> u64 {
    static mut TB: [u32; 2] = [0, 0];
    unsafe {
        let tb = &mut *core::ptr::addr_of_mut!(TB);
        if tb[1] == 0 {
            mach_timebase_info(tb);
        }
        let t = mach_absolute_time() as u128;
        (t * tb[0].max(1) as u128 / tb[1].max(1) as u128 / 1000) as u64
    }
}

pub fn sleep_us(us: u64) {
    unsafe {
        usleep(us.min(1_000_000) as u32);
    }
}

unsafe fn cstr(p: *const u8) -> &'static [u8] {
    let mut n = 0;
    unsafe {
        while *p.add(n) != 0 {
            n += 1;
        }
        core::slice::from_raw_parts(p, n)
    }
}

pub struct Env {
    argc: usize,
    argv: *const *const u8,
    envp: *const *const u8,
}

impl Env {
    pub fn arg(&self, i: usize) -> Option<&'static [u8]> {
        if i >= self.argc {
            return None;
        }
        unsafe { Some(cstr(*self.argv.add(i))) }
    }
    pub fn var(&self, name: &[u8]) -> Option<&'static [u8]> {
        let mut p = self.envp;
        unsafe {
            while !(*p).is_null() {
                let kv = cstr(*p);
                if kv.len() > name.len() && kv[name.len()] == b'=' && &kv[..name.len()] == name {
                    return Some(&kv[name.len() + 1..]);
                }
                p = p.add(1);
            }
        }
        None
    }
}

// ------------------------------------------------------------------ storage

pub struct FileHost {
    dir: Buf<256>,
    fd: i32,
}

impl FileHost {
    fn new(env: &Env) -> FileHost {
        let mut dir = Buf::new();
        match env.var(b"HOME") {
            Some(h) if !h.is_empty() => {
                dir.push(h).push(b"/Library/Application Support/Hellbyte");
            }
            _ => {
                dir.push(b".hellbyte");
            }
        }
        FileHost { dir, fd: -1 }
    }
    fn path(&self, slot: u8) -> Buf<300> {
        let mut p = Buf::new();
        p.push(self.dir.as_bytes());
        if slot == hellbyte_engine::save::CONFIG_SLOT {
            p.push(b"/config.hbc");
        } else {
            p.push(b"/slot").num(slot as u64).push(b".hbs");
        }
        p
    }
}

impl Host for FileHost {
    fn save_begin(&mut self, slot: u8) -> bool {
        let d = self.dir.as_bytes();
        for i in 1..=d.len() {
            if i == d.len() || d[i] == b'/' {
                let mut p: Buf<300> = Buf::new();
                p.push(&d[..i]);
                unsafe {
                    mkdir(p.cstr().as_ptr(), 0o755);
                }
            }
        }
        let mut p = self.path(slot);
        self.fd = unsafe { open(p.cstr().as_ptr(), O_WRONLY | O_CREAT | O_TRUNC, 0o644 as u32) };
        self.fd >= 0
    }
    fn save_write(&mut self, data: &[u8]) -> bool {
        self.fd >= 0 && write_all(self.fd, data)
    }
    fn save_end(&mut self) -> bool {
        if self.fd >= 0 {
            unsafe {
                close(self.fd);
            }
            self.fd = -1;
            return true;
        }
        false
    }
    fn load_begin(&mut self, slot: u8) -> bool {
        let mut p = self.path(slot);
        self.fd = unsafe { open(p.cstr().as_ptr(), O_RDONLY) };
        self.fd >= 0
    }
    fn load_read(&mut self, buf: &mut [u8]) -> bool {
        let mut got = 0;
        while got < buf.len() {
            let n = unsafe { read(self.fd, buf[got..].as_mut_ptr(), buf.len() - got) };
            if n <= 0 {
                return false;
            }
            got += n as usize;
        }
        true
    }
    fn load_end(&mut self) {
        self.save_end();
    }
}

// ------------------------------------------------------------------ entry

#[unsafe(no_mangle)]
pub extern "C" fn main(argc: i32, argv: *const *const u8, envp: *const *const u8) -> i32 {
    let env = Env { argc: argc as usize, argv, envp };
    let code = run(&env);
    unsafe { exit(code) }
}

fn run(env: &Env) -> i32 {
    let args = Args::parse(&|i| env.arg(i));
    if args.help {
        write_all(1, HELP);
        return 0;
    }
    if args.version {
        write_all(1, b"hellbyte ");
        write_all(1, env!("CARGO_PKG_VERSION").as_bytes());
        write_all(1, b" (GPL-3.0-or-later)\n");
        return 0;
    }
    let mut host = FileHost::new(env);
    // SAFETY: the only call; the engine lives for the whole program.
    let e = unsafe { hellbyte_engine::instance() };
    if args.bench || args.shot.is_some() {
        let (w, h) = args.size.unwrap_or((320, 200));
        e.init(w, h, false, &mut host);
        e.new_game(Skill::from_u8(args.skill), args.map.unwrap_or(0) as usize);
        let t0 = now_us();
        let mut tics = 0;
        if let Some(s) = args.script {
            tics += crate::common::script::run(e, &mut host, s);
        }
        while tics < args.tics {
            e.tick(&mut host);
            tics += 1;
            e.draw();
        }
        e.draw();
        let mut l: Buf<160> = Buf::new();
        let h = e.state_hash();
        l.push(b"tics ").num(tics as u64).push(b"  total ").num((now_us() - t0) / 1000).push(b" ms  hash ");
        for i in (0..8).rev() {
            l.push(&[b"0123456789abcdef"[((h >> (i * 4)) & 15) as usize]]);
        }
        l.push(b"\n");
        write_all(1, l.as_bytes());
        return 0;
    }
    let use_term = args.backend == Backend::Term;
    if !use_term {
        match cocoa::run(e, &mut host, &args) {
            Ok(c) => return c,
            Err(msg) => {
                write_all(2, b"hellbyte: window: ");
                write_all(2, msg);
                write_all(2, b"; using the terminal\n");
            }
        }
    }
    term_run(e, &mut host, env, &args)
}

// ------------------------------------------------------------------ terminal

static mut SAVED: [u8; 128] = [0; 128];
static mut TUI: Tui = Tui::new();

fn term_run(e: &mut Engine, host: &mut dyn Host, env: &Env, args: &Args) -> i32 {
    // macOS termios: 64-bit flag words, c_cc at offset 32, VMIN=16, VTIME=17.
    let mut t = [0u8; 128];
    unsafe {
        if tcgetattr(0, t.as_mut_ptr()) != 0 {
            write_all(2, b"hellbyte: stdin is not a terminal\n");
            return 1;
        }
        *core::ptr::addr_of_mut!(SAVED) = t;
    }
    let rd = |t: &[u8; 128], o: usize| u64::from_ne_bytes([t[o], t[o + 1], t[o + 2], t[o + 3], t[o + 4], t[o + 5], t[o + 6], t[o + 7]]);
    let iflag = rd(&t, 0) & !(0x200 | 0x100 | 0x2 | 0x10 | 0x20); // IXON ICRNL BRKINT INPCK ISTRIP
    let lflag = rd(&t, 24) & !(0x100 | 0x8 | 0x80 | 0x400); // ICANON ECHO ISIG IEXTEN
    t[0..8].copy_from_slice(&iflag.to_ne_bytes());
    t[24..32].copy_from_slice(&lflag.to_ne_bytes());
    t[32 + 16] = 0;
    t[32 + 17] = 0;
    unsafe {
        tcsetattr(0, 0, t.as_ptr());
    }
    let tui = unsafe { &mut *core::ptr::addr_of_mut!(TUI) };
    tui.colors = Colors::detect(env.var(b"TERM"), env.var(b"COLORTERM"), args.colors);
    if env.var(b"TERM_PROGRAM").is_some_and(|p| p == b"Apple_Terminal") && args.colors == 0 {
        tui.colors = Colors::X256; // Terminal.app lacks truecolor
    }
    let winsize = |tui: &mut Tui| {
        let mut ws = [0u16; 4];
        if unsafe { ioctl(1, TIOCGWINSZ, ws.as_mut_ptr()) } == 0 && ws[0] > 0 {
            tui.cols = ws[1] as usize;
            tui.rows = ws[0] as usize;
        }
    };
    winsize(tui);
    write_all(1, Tui::ENTER);
    write_all(1, Tui::KITTY_QUERY);
    let (w, h) = tui.view_size();
    e.init(w, h, true, host);
    e.set_text_ui(true);
    if let Some(m) = args.map {
        e.new_game(Skill::from_u8(args.skill), m as usize);
    }
    let mut dec = KeyDecoder::new();
    let mut held = HeldKeys::new();
    let mut kitty = false;
    let tic_us: u64 = 1_000_000 / 35;
    let mut next_tic = now_us();
    let mut last_size = 0u64;
    let mut inbuf = [0u8; 256];
    let mut writer = |b: &[u8]| {
        write_all(1, b);
    };
    loop {
        loop {
            let n = unsafe { read(0, inbuf.as_mut_ptr(), inbuf.len()) };
            if n <= 0 {
                break;
            }
            let now = now_us() / 1000;
            dec.feed(&inbuf[..n as usize], &mut |ev| {
                if ev.key == 0 {
                    return;
                }
                if kitty {
                    e.key(ev.key, ev.down);
                } else if held.press(ev.key, now) {
                    e.key(ev.key, true);
                }
            });
        }
        if dec.kitty && !kitty {
            kitty = true;
            write_all(1, Tui::KITTY_ON);
        }
        if !kitty {
            held.expire(now_us() / 1000, &mut |k| e.key(k, false));
        }
        if dec.quit || e.quit_requested() {
            break;
        }
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
        if now - last_size > 500_000 {
            last_size = now;
            let (c, r) = (tui.cols, tui.rows);
            winsize(tui);
            if c != tui.cols || r != tui.rows {
                tui.force = true;
                let (w, h) = tui.view_size();
                e.resize(w, h, true);
            }
        }
        if steps > 0 {
            e.draw();
            tui.frame(e, &mut writer);
        }
        let wait = next_tic.saturating_sub(now_us());
        if wait > 0 {
            sleep_us(wait.min(10_000));
        }
    }
    write_all(1, Tui::LEAVE);
    unsafe {
        tcsetattr(0, 0, core::ptr::addr_of!(SAVED) as *const u8);
    }
    let _ = KEY_ESCAPE;
    0
}

pub fn emergency_restore() {
    write_all(1, Tui::LEAVE);
    unsafe {
        tcsetattr(0, 0, core::ptr::addr_of!(SAVED) as *const u8);
    }
}
