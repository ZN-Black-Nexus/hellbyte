//! Windows platform layer: no C runtime, Win32 only (Windows 7 and newer;
//! the console renderer needs Windows 10 for VT sequences).

pub mod sys;
pub mod win;

use crate::common::args::{Args, HELP};
use crate::common::tui::{Colors, HeldKeys, KeyDecoder, Tui};
use crate::common::Buf;
use hellbyte_engine::game::Skill;
use hellbyte_engine::keys::*;
use hellbyte_engine::{Engine, Host};
use sys::*;

// ------------------------------------------------------------------ runtime symbols

/// MSVC-style code expects this when floating point appears anywhere.
#[unsafe(no_mangle)]
pub static _fltused: i32 = 0;

/// Stack probe for frames larger than a page (x86_64 MSVC ABI: size in rax,
/// touch each page, don't move rsp).
#[cfg(target_arch = "x86_64")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn __chkstk() {
    core::arch::naked_asm!(
        "push rcx",
        "push rax",
        "lea rcx, [rsp + 24]",
        "cmp rax, 0x1000",
        "jb 2f",
        "3:",
        "sub rcx, 0x1000",
        "test [rcx], rcx",
        "sub rax, 0x1000",
        "cmp rax, 0x1000",
        "ja 3b",
        "2:",
        "sub rcx, rax",
        "test [rcx], rcx",
        "pop rax",
        "pop rcx",
        "ret",
    )
}

/// 32-bit x86 MSVC stack probe: size in eax; this one also moves esp.
#[cfg(target_arch = "x86")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn _chkstk() {
    core::arch::naked_asm!(
        "push ecx",
        "lea ecx, [esp + 8]",
        "cmp eax, 0x1000",
        "jb 2f",
        "3:",
        "sub ecx, 0x1000",
        "test [ecx], eax",
        "sub eax, 0x1000",
        "cmp eax, 0x1000",
        "jae 3b",
        "2:",
        "sub ecx, eax",
        "test [ecx], eax",
        "mov eax, esp",
        "mov esp, ecx",
        "mov ecx, [eax]",
        "mov eax, [eax + 4]",
        "jmp eax",
    )
}

// 32-bit x86 MSVC code calls these for 64-bit arithmetic (normally in the
// C runtime). Arguments sit on the stack and the callee pops them; we
// re-push them for compiler_builtins' portable versions.
#[cfg(target_arch = "x86")]
unsafe extern "C" {
    fn __udivdi3(n: u64, d: u64) -> u64;
    fn __umoddi3(n: u64, d: u64) -> u64;
    fn __divdi3(n: i64, d: i64) -> i64;
    fn __moddi3(n: i64, d: i64) -> i64;
    fn __muldi3(a: i64, b: i64) -> i64;
}

#[cfg(target_arch = "x86")]
macro_rules! msvc_i64_helper {
    ($name:ident, $target:ident) => {
        #[unsafe(no_mangle)]
        #[unsafe(naked)]
        pub unsafe extern "C" fn $name() {
            core::arch::naked_asm!(
                "push dword ptr [esp + 16]",
                "push dword ptr [esp + 16]",
                "push dword ptr [esp + 16]",
                "push dword ptr [esp + 16]",
                "call {f}",
                "add esp, 16",
                "ret 16",
                f = sym $target,
            )
        }
    };
}

#[cfg(target_arch = "x86")]
msvc_i64_helper!(_aulldiv, __udivdi3);
#[cfg(target_arch = "x86")]
msvc_i64_helper!(_aullrem, __umoddi3);
#[cfg(target_arch = "x86")]
msvc_i64_helper!(_alldiv, __divdi3);
#[cfg(target_arch = "x86")]
msvc_i64_helper!(_allrem, __moddi3);
#[cfg(target_arch = "x86")]
msvc_i64_helper!(_allmul, __muldi3);

/// ARM64 Windows stack probe: size/16 in x15, touch each page, keep registers.
#[cfg(target_arch = "aarch64")]
#[unsafe(no_mangle)]
#[unsafe(naked)]
pub unsafe extern "C" fn __chkstk() {
    core::arch::naked_asm!(
        "lsl x16, x15, #4",
        "mov x17, sp",
        "1:",
        "sub x17, x17, #4096",
        "subs x16, x16, #4096",
        "ldr xzr, [x17]",
        "b.gt 1b",
        "ret",
    )
}

// ------------------------------------------------------------------ console output

static mut STDOUT: HANDLE = 0;

pub fn console_out(b: &[u8]) {
    unsafe {
        if STDOUT == 0 {
            AttachConsole(0xffff_ffff); // ATTACH_PARENT_PROCESS
            STDOUT = GetStdHandle(0xffff_fff5); // STD_OUTPUT_HANDLE
        }
        if STDOUT != 0 && STDOUT != INVALID_HANDLE {
            stdout_write(STDOUT, b);
        }
    }
}

/// A crash: print it to the console if there is one, show it in a message
/// box (a double-clicked game has no console), and exit with code 101.
pub fn crash(msg: &[u8]) -> ! {
    console_out(msg);
    let mut w = [0u16; 600];
    let mut t = [0u16; 32];
    unsafe {
        MessageBoxW(0, wide(msg, &mut w).as_ptr(), wide(b"Hellbyte crashed", &mut t).as_ptr(), 0x10);
        ExitProcess(101)
    }
}

pub fn fatal(msg: &[u8]) -> ! {
    let mut w = [0u16; 256];
    let mut t = [0u16; 16];
    unsafe {
        MessageBoxW(0, wide(msg, &mut w).as_ptr(), wide(b"Hellbyte", &mut t).as_ptr(), 0x10);
        ExitProcess(1)
    }
}

// ------------------------------------------------------------------ command line

static mut ARGBUF: [u8; 2048] = [0; 2048];
static mut ARGS: [(u16, u16); 32] = [(0, 0); 32];
static mut ARGC: usize = 0;

fn parse_cmdline() {
    unsafe {
        let mut p = GetCommandLineW();
        let buf = &mut *core::ptr::addr_of_mut!(ARGBUF);
        let args = &mut *core::ptr::addr_of_mut!(ARGS);
        let mut len = 0usize;
        let mut n = 0usize;
        loop {
            while *p == b' ' as u16 || *p == b'\t' as u16 {
                p = p.add(1);
            }
            if *p == 0 || n == args.len() {
                break;
            }
            let start = len;
            let mut quoted = false;
            while *p != 0 && (quoted || (*p != b' ' as u16 && *p != b'\t' as u16)) {
                if *p == b'"' as u16 {
                    quoted = !quoted;
                } else if len < buf.len() {
                    buf[len] = if *p < 128 { *p as u8 } else { b'?' };
                    len += 1;
                }
                p = p.add(1);
            }
            args[n] = (start as u16, len as u16);
            n += 1;
        }
        ARGC = n;
    }
}

fn arg(i: usize) -> Option<&'static [u8]> {
    unsafe {
        if i >= ARGC {
            return None;
        }
        let (a, b) = (*core::ptr::addr_of!(ARGS))[i];
        let buf: &'static [u8; 2048] = &*core::ptr::addr_of!(ARGBUF);
        Some(&buf[a as usize..b as usize])
    }
}

// ------------------------------------------------------------------ storage

pub struct FileHost {
    dir: [u16; 300],
    dirlen: usize,
    h: HANDLE,
}

impl FileHost {
    pub fn new() -> FileHost {
        let mut fh = FileHost { dir: [0; 300], dirlen: 0, h: INVALID_HANDLE };
        let mut name = [0u16; 16];
        let n = unsafe { GetEnvironmentVariableW(wide(b"APPDATA", &mut name).as_ptr(), fh.dir.as_mut_ptr(), 250) } as usize;
        let suffix: &[u8] = b"\\Hellbyte";
        let mut len = if n > 0 && n < 250 { n } else { 0 };
        if len == 0 {
            fh.dir[0] = b'.' as u16;
            len = 1;
        }
        for &c in suffix {
            fh.dir[len] = c as u16;
            len += 1;
        }
        fh.dirlen = len;
        fh
    }

    fn path(&self, slot: u8, out: &mut [u16; 320]) {
        out[..self.dirlen].copy_from_slice(&self.dir[..self.dirlen]);
        let mut name: Buf<32> = Buf::new();
        if slot == hellbyte_engine::save::CONFIG_SLOT {
            name.push(b"\\config.hbc");
        } else {
            name.push(b"\\slot").num(slot as u64).push(b".hbs");
        }
        let mut i = self.dirlen;
        for &c in name.as_bytes() {
            out[i] = c as u16;
            i += 1;
        }
        out[i] = 0;
    }
}

impl Host for FileHost {
    fn save_begin(&mut self, slot: u8) -> bool {
        let mut d = [0u16; 301];
        d[..self.dirlen].copy_from_slice(&self.dir[..self.dirlen]);
        unsafe {
            CreateDirectoryW(d.as_ptr(), core::ptr::null());
        }
        let mut p = [0u16; 320];
        self.path(slot, &mut p);
        // GENERIC_WRITE, CREATE_ALWAYS
        self.h = unsafe { CreateFileW(p.as_ptr(), 0x4000_0000, 0, core::ptr::null(), 2, 0x80, 0) };
        self.h != INVALID_HANDLE
    }
    fn save_write(&mut self, data: &[u8]) -> bool {
        self.h != INVALID_HANDLE && stdout_write(self.h, data)
    }
    fn save_end(&mut self) -> bool {
        if self.h != INVALID_HANDLE {
            unsafe {
                CloseHandle(self.h);
            }
            self.h = INVALID_HANDLE;
            return true;
        }
        false
    }
    fn load_begin(&mut self, slot: u8) -> bool {
        let mut p = [0u16; 320];
        self.path(slot, &mut p);
        // GENERIC_READ, FILE_SHARE_READ, OPEN_EXISTING
        self.h = unsafe { CreateFileW(p.as_ptr(), 0x8000_0000, 1, core::ptr::null(), 3, 0x80, 0) };
        self.h != INVALID_HANDLE
    }
    fn load_read(&mut self, buf: &mut [u8]) -> bool {
        let mut got = 0;
        while got < buf.len() {
            let mut n = 0u32;
            let ok = unsafe { ReadFile(self.h, buf[got..].as_mut_ptr(), (buf.len() - got) as u32, &mut n, core::ptr::null_mut()) };
            if ok == 0 || n == 0 {
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
pub extern "system" fn WinMainCRTStartup() -> ! {
    parse_cmdline();
    let code = run();
    unsafe { ExitProcess(code as u32) }
}

#[unsafe(no_mangle)]
pub extern "system" fn mainCRTStartup() -> ! {
    WinMainCRTStartup()
}

fn run() -> i32 {
    let args = Args::parse(&arg);
    if args.help {
        console_out(HELP);
        return 0;
    }
    if args.version {
        console_out(b"hellbyte ");
        console_out(env!("CARGO_PKG_VERSION").as_bytes());
        console_out(b" (GPL-3.0-or-later)\r\n");
        return 0;
    }
    unsafe {
        timeBeginPeriod(1);
    }
    let mut host = FileHost::new();
    // SAFETY: the only call; the engine lives for the whole program.
    let e = unsafe { hellbyte_engine::instance() };
    if args.bench || args.shot.is_some() {
        let (w, h) = args.size.unwrap_or((320, 200));
        e.init(w, h, false, &mut host);
        e.new_game(Skill::from_u8(args.skill), args.map.unwrap_or(0) as usize);
        if let Some((x, y, a)) = args.warp {
            e.debug_warp(x, y, a);
        }
        let t0 = now_us();
        let mut tics = 0;
        if let Some(s) = args.script {
            tics += crate::common::script::run(e, &mut host, s);
        }
        let mut frames = 0u64;
        while tics < args.tics {
            e.tick(&mut host);
            tics += 1;
            if args.bench {
                e.draw();
                frames += 1;
            }
        }
        e.draw();
        let dt = now_us() - t0;
        let mut l: Buf<160> = Buf::new();
        l.push(b"tics ").num(tics as u64).push(b"  frames ").num(frames + 1).push(b"  total ").num(dt / 1000);
        l.push(b" ms  per frame ").num(dt / (frames + 1)).push(b" us  hash ");
        let h = e.state_hash();
        for i in (0..8).rev() {
            l.push(&[b"0123456789abcdef"[((h >> (i * 4)) & 15) as usize]]);
        }
        l.push(b"\r\n");
        console_out(l.as_bytes());
        return 0;
    }
    if args.backend == crate::common::args::Backend::Term {
        return console_run(e, &mut host, &args);
    }
    win::run(e, &mut host, &args)
}

// ------------------------------------------------------------------ console mode

static mut TUI: Tui = Tui::new();

fn vk_to_key(vk: u16, ch: u16) -> u16 {
    match vk {
        0x25 => KEY_LEFT,
        0x26 => KEY_UP,
        0x27 => KEY_RIGHT,
        0x28 => KEY_DOWN,
        0x0d => KEY_ENTER,
        0x1b => KEY_ESCAPE,
        0x09 => KEY_TAB,
        0x08 => KEY_BACKSPACE,
        0x20 => KEY_SPACE,
        0x10 | 0xa0 | 0xa1 => KEY_SHIFT,
        0x11 | 0xa2 | 0xa3 => KEY_CTRL,
        0x12 | 0xa4 | 0xa5 => KEY_ALT,
        0x70..=0x7b => KEY_F1 + (vk - 0x70),
        0x13 => KEY_PAUSE,
        0x21 => KEY_PGUP,
        0x22 => KEY_PGDN,
        0x23 => KEY_END,
        0x24 => KEY_HOME,
        0x2d => KEY_INSERT,
        0x2e => KEY_DELETE,
        0x30..=0x39 => vk,
        0x41..=0x5a => vk + 32,
        0xbd | 0x6d => b'-' as u16,
        0xbb | 0x6b => b'=' as u16,
        0xbc => b',' as u16,
        0xbe => b'.' as u16,
        0xbf => b'/' as u16,
        _ => {
            if (32..127).contains(&ch) {
                (ch as u8).to_ascii_lowercase() as u16
            } else {
                0
            }
        }
    }
}

pub fn vk_key(vk: u16) -> u16 {
    vk_to_key(vk, 0)
}

fn console_run(e: &mut Engine, host: &mut dyn Host, args: &Args) -> i32 {
    unsafe {
        AttachConsole(0xffff_ffff);
        let out = GetStdHandle(0xffff_fff5);
        let inp = GetStdHandle(0xffff_fff6);
        STDOUT = out;
        let mut mode = 0u32;
        GetConsoleMode(out, &mut mode);
        // ENABLE_PROCESSED_OUTPUT | ENABLE_VIRTUAL_TERMINAL_PROCESSING | DISABLE_NEWLINE_AUTO_RETURN
        if SetConsoleMode(out, mode | 0x1 | 0x4 | 0x8) == 0 {
            fatal(b"This console can't show VT graphics (needs Windows 10 or newer). Run without --term.");
        }
        SetConsoleOutputCP(65001);
        let mut imode = 0u32;
        GetConsoleMode(inp, &mut imode);
        // Raw keys: no line input / echo / processed input.
        SetConsoleMode(inp, 0x80);
        let tui = &mut *core::ptr::addr_of_mut!(TUI);
        tui.colors = if args.colors != 0 { Colors::detect(None, None, args.colors) } else { Colors::True };
        let size = |tui: &mut Tui| {
            let mut info = CONSOLE_SCREEN_BUFFER_INFO::default();
            if GetConsoleScreenBufferInfo(out, &mut info) != 0 {
                tui.cols = (info.srWindow[2] - info.srWindow[0] + 1).max(16) as usize;
                tui.rows = (info.srWindow[3] - info.srWindow[1] + 1).max(8) as usize;
            }
        };
        size(tui);
        stdout_write(out, Tui::ENTER);
        let (w, h) = tui.view_size();
        e.init(w, h, true, host);
        e.set_text_ui(true);
        if let Some(m) = args.map {
            e.new_game(Skill::from_u8(args.skill), m as usize);
        }
        let tic_us: u64 = 1_000_000 / 35;
        let mut next_tic = now_us();
        let mut last_size = 0u64;
        let mut recs: [INPUT_RECORD; 32] = core::mem::zeroed();
        let _unused_decoder = KeyDecoder::new();
        let _unused_held = HeldKeys::new();
        let mut writer = |b: &[u8]| {
            stdout_write(out, b);
        };
        loop {
            let mut n = 0u32;
            GetNumberOfConsoleInputEvents(inp, &mut n);
            while n > 0 {
                let mut got = 0u32;
                ReadConsoleInputW(inp, recs.as_mut_ptr(), 32, &mut got);
                for r in &recs[..got as usize] {
                    if r.EventType == 1 {
                        let k = &r.Event;
                        let key = vk_to_key(k.wVirtualKeyCode, k.uChar);
                        if key == b'c' as u16 && k.dwControlKeyState & 0xc != 0 && k.bKeyDown != 0 {
                            stdout_write(out, Tui::LEAVE);
                            return 0;
                        }
                        if key != 0 {
                            e.key(key, k.bKeyDown != 0);
                        }
                    }
                }
                GetNumberOfConsoleInputEvents(inp, &mut n);
            }
            if e.quit_requested() {
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
                size(tui);
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
            if wait > 1000 {
                sleep_us(wait.min(10_000));
            }
        }
        stdout_write(out, Tui::LEAVE);
        SetConsoleMode(inp, imode);
    }
    0
}
