//! Terminal backend: plays in any terminal (local, SSH, serial console).

use super::nr;
use super::*;
use crate::common::args::Args;
use crate::common::tui::{Colors, HeldKeys, KeyDecoder, Tui};
use hellbyte_engine::keys::*;
use hellbyte_engine::{Engine, Host};

static mut SAVED_TERMIOS: [u8; 64] = [0; 64];
static mut RAW_ACTIVE: bool = false;
static mut TUI: Tui = Tui::new();

fn termios_get(buf: &mut [u8; 64]) -> bool {
    ioctl(0, nr::TCGETS, buf.as_mut_ptr() as usize) == 0
}

fn termios_set(buf: &[u8; 64]) -> bool {
    ioctl(0, nr::TCSETS, buf.as_ptr() as usize) == 0
}

fn get_u32(b: &[u8; 64], off: usize) -> u32 {
    u32::from_ne_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
}

fn set_u32(b: &mut [u8; 64], off: usize, v: u32) {
    b[off..off + 4].copy_from_slice(&v.to_ne_bytes());
}

/// Switch stdin to raw, non-blocking mode. Returns false if it isn't a terminal.
fn raw_mode() -> bool {
    let mut t = [0u8; 64];
    if !termios_get(&mut t) {
        return false;
    }
    unsafe {
        *core::ptr::addr_of_mut!(SAVED_TERMIOS) = t;
    }
    let iflag = get_u32(&t, 0) & !(nr::termios::IXON | nr::ICRNL | nr::BRKINT | nr::INPCK | nr::ISTRIP);
    let lflag = get_u32(&t, 12) & !(nr::termios::ICANON | nr::termios::ECHO | nr::termios::ISIG | nr::termios::IEXTEN);
    set_u32(&mut t, 0, iflag);
    set_u32(&mut t, 12, lflag);
    t[nr::termios::CC_OFFSET + nr::termios::VMIN] = 0;
    t[nr::termios::CC_OFFSET + nr::termios::VTIME] = 0;
    if !termios_set(&t) {
        return false;
    }
    unsafe {
        RAW_ACTIVE = true;
    }
    true
}

/// Raw keyboard mode for other backends (framebuffer) that read the tty.
pub fn raw_mode_public() -> bool {
    raw_mode()
}

/// Restore the terminal. Safe to call from the panic handler.
pub fn emergency_restore() {
    unsafe {
        if RAW_ACTIVE {
            let t = *core::ptr::addr_of!(SAVED_TERMIOS);
            termios_set(&t);
            write_all(1, Tui::LEAVE);
            RAW_ACTIVE = false;
        }
    }
}

fn winsize() -> (usize, usize) {
    let mut ws = [0u16; 4];
    if ioctl(1, nr::TIOCGWINSZ, ws.as_mut_ptr() as usize) == 0 && ws[0] > 0 && ws[1] > 0 {
        (ws[1] as usize, ws[0] as usize)
    } else {
        (80, 24)
    }
}

fn now_ms() -> u64 {
    now_us() / 1000
}

pub fn run(e: &mut Engine, host: &mut dyn Host, env: &Env, args: &Args) -> i32 {
    if !raw_mode() {
        write_all(2, b"hellbyte: stdin is not a terminal (use --x11 or --fb, or run in a terminal)\n");
        return 1;
    }
    // SAFETY: single-threaded; only this function uses the renderer state.
    let tui = unsafe { &mut *core::ptr::addr_of_mut!(TUI) };
    tui.colors = Colors::detect(env.var(b"TERM"), env.var(b"COLORTERM"), args.colors);
    let (cols, rows) = winsize();
    tui.cols = cols;
    tui.rows = rows;
    write_all(1, Tui::ENTER);
    write_all(1, Tui::KITTY_QUERY);
    let (w, h) = tui.view_size();
    e.init(w, h, true, host);
    e.set_text_ui(true);
    if let Some(m) = args.map {
        e.new_game(hellbyte_engine::game::Skill::from_u8(args.skill), m as usize);
    }

    let mut dec = KeyDecoder::new();
    let mut held = HeldKeys::new();
    let mut kitty_on = false;
    let tic_us: u64 = 1_000_000 / 35;
    let frame_us: u64 = 1_000_000 / args.fps.max(1) as u64;
    let mut next_tic = now_us();
    let mut last_frame = 0u64;
    let mut last_size_check = 0u64;
    let mut inbuf = [0u8; 256];
    let start = now_ms();
    let mut writer = |b: &[u8]| {
        write_all(1, b);
    };
    loop {
        // ---- input
        loop {
            let n = read(0, &mut inbuf);
            if n <= 0 {
                break;
            }
            let now = now_ms();
            dec.feed(&inbuf[..n as usize], &mut |ev| {
                if dec_kitty_hint(ev.key) {
                    return;
                }
                if kitty_on {
                    if ev.shift && ev.down {
                        e.key(KEY_SHIFT, true);
                    }
                    e.key(ev.key, ev.down);
                    return;
                }
                // No key-up events: synthesise them from the auto-repeat stream.
                if ev.shift {
                    if held.press(KEY_SHIFT, now) {
                        e.key(KEY_SHIFT, true);
                    }
                }
                if held.press(ev.key, now) {
                    e.key(ev.key, true);
                }
            });
            if dec.quit {
                break;
            }
        }
        if dec.kitty && !kitty_on {
            // Terminal supports release events: switch them on.
            kitty_on = true;
            write_all(1, Tui::KITTY_ON);
        }
        if !kitty_on {
            held.expire(now_ms(), &mut |k| e.key(k, false));
        }
        if dec.quit || e.quit_requested() {
            break;
        }
        // Give the kitty query ~150 ms to answer before trusting the fallback.
        let _ = start;

        // ---- simulation at 35 Hz
        let now = now_us();
        let mut steps = 0;
        while now >= next_tic && steps < 4 {
            e.tick(host);
            next_tic += tic_us;
            steps += 1;
        }
        if now > next_tic + tic_us * 8 {
            next_tic = now; // we fell far behind (slow terminal); don't spiral
        }

        // ---- resize
        if now - last_size_check > 500_000 {
            last_size_check = now;
            let (c, r) = winsize();
            if c != tui.cols || r != tui.rows {
                tui.cols = c;
                tui.rows = r;
                tui.force = true;
                let (w, h) = tui.view_size();
                e.resize(w, h, true);
            }
        }

        // ---- draw
        if steps > 0 && now - last_frame >= frame_us {
            last_frame = now;
            e.draw();
            tui.frame(e, &mut writer);
        }
        let wait = next_tic.saturating_sub(now_us());
        if wait > 0 {
            sleep_us(wait.min(10_000));
        }
    }
    emergency_restore();
    0
}

/// Keys that are only protocol noise (never passed to the game).
fn dec_kitty_hint(k: u16) -> bool {
    k == 0
}
