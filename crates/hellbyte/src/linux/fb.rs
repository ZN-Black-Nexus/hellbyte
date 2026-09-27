//! Linux framebuffer backend: draws straight into /dev/fb0 and reads the
//! keyboard/mouse from evdev. For consoles, kiosks, appliances and other
//! devices with a screen but no window system.

use super::nr;
use super::*;
use crate::common::args::Args;
use crate::common::tui::{HeldKeys, KeyDecoder};
use crate::common::Buf;
use hellbyte_engine::keys::*;
use hellbyte_engine::{Engine, Host};

const W: usize = core::mem::size_of::<usize>();
static mut VT_GRAPHICS: bool = false;

pub fn available() -> bool {
    let fd = open(b"/dev/fb0\0", nr::O_RDWR, 0);
    if fd < 0 {
        return false;
    }
    close(fd);
    true
}

pub fn restore_console() {
    unsafe {
        if VT_GRAPHICS {
            ioctl(0, nr::KDSETMODE, nr::KD_TEXT);
            VT_GRAPHICS = false;
        }
    }
    super::term::emergency_restore();
}

fn rd32(b: &[u8], o: usize) -> u32 {
    u32::from_ne_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

/// Linux input keycode -> engine key.
fn evkey(code: u16) -> u16 {
    const ROW1: &[u8] = b"1234567890-=";
    const ROWQ: &[u8] = b"qwertyuiop[]";
    const ROWA: &[u8] = b"asdfghjkl;'`";
    const ROWZ: &[u8] = b"zxcvbnm,./";
    match code {
        1 => KEY_ESCAPE,
        2..=13 => ROW1[(code - 2) as usize] as u16,
        14 => KEY_BACKSPACE,
        15 => KEY_TAB,
        16..=27 => ROWQ[(code - 16) as usize] as u16,
        28 | 96 => KEY_ENTER,
        29 | 97 => KEY_CTRL,
        30..=41 => ROWA[(code - 30) as usize] as u16,
        42 | 54 => KEY_SHIFT,
        44..=53 => ROWZ[(code - 44) as usize] as u16,
        56 | 100 => KEY_ALT,
        57 => KEY_SPACE,
        59..=68 => KEY_F1 + (code - 59),
        87 => KEY_F11,
        88 => KEY_F12,
        102 => KEY_HOME,
        103 => KEY_UP,
        104 => KEY_PGUP,
        105 => KEY_LEFT,
        106 => KEY_RIGHT,
        107 => KEY_END,
        108 => KEY_DOWN,
        109 => KEY_PGDN,
        110 => KEY_INSERT,
        111 => KEY_DELETE,
        119 => KEY_PAUSE,
        0x110 => KEY_MOUSE1,
        0x111 => KEY_MOUSE2,
        0x112 => KEY_MOUSE3,
        _ => 0,
    }
}

pub fn run(e: &mut Engine, host: &mut dyn Host, _env: &Env, args: &Args) -> Result<i32, &'static [u8]> {
    let fb = open(b"/dev/fb0\0", nr::O_RDWR, 0);
    if fb < 0 {
        return Err(b"cannot open /dev/fb0");
    }
    let mut var = [0u8; 160];
    let mut fix = [0u8; 80];
    if ioctl(fb, nr::FBIOGET_VSCREENINFO, var.as_mut_ptr() as usize) != 0
        || ioctl(fb, nr::FBIOGET_FSCREENINFO, fix.as_mut_ptr() as usize) != 0
    {
        close(fb);
        return Err(b"framebuffer ioctls failed");
    }
    let xres = rd32(&var, 0) as usize;
    let yres = rd32(&var, 4) as usize;
    let yoff = rd32(&var, 20) as usize;
    let bpp = rd32(&var, 24) as usize;
    let (ro, rl, go, gl, bo, bl) = (rd32(&var, 32), rd32(&var, 36), rd32(&var, 44), rd32(&var, 48), rd32(&var, 56), rd32(&var, 60));
    let smem_len = rd32(&fix, 16 + W) as usize;
    let stride = rd32(&fix, 16 + W + 24) as usize;
    if !(bpp == 16 || bpp == 24 || bpp == 32) || xres == 0 || yres == 0 {
        close(fb);
        return Err(b"unsupported framebuffer format (need 16/24/32 bpp)");
    }
    let map_len = smem_len.max(stride * (yres + yoff));
    let mem = mmap_shared(fb, map_len, nr::PROT_READ | nr::PROT_WRITE);
    if mem.is_null() {
        close(fb);
        return Err(b"cannot map the framebuffer");
    }
    // SAFETY: the kernel mapped `map_len` bytes for us.
    let screen = unsafe { core::slice::from_raw_parts_mut(mem, map_len) };

    // Keyboard/mouse: every readable evdev node (needs the 'input' group or root).
    let mut evfds = [-1i32; 16];
    let mut nev = 0;
    for i in 0..32 {
        let mut p: Buf<40> = Buf::new();
        p.push(b"/dev/input/event").num(i);
        let fd = open(p.cstr(), nr::O_RDONLY | nr::O_NONBLOCK, 0);
        if fd >= 0 && nev < evfds.len() {
            // Keep keys away from the console while we play.
            let one: usize = 1;
            ioctl(fd, nr::EVIOCGRAB, one);
            evfds[nev] = fd;
            nev += 1;
        }
    }
    // The tty: raw mode so keys don't echo, graphics mode so the console stays quiet.
    let tty_raw = {
        let mut t = [0u8; 64];
        ioctl(0, nr::TCGETS, t.as_mut_ptr() as usize) == 0
    };
    if tty_raw {
        super::term::raw_mode_public();
    }
    if ioctl(0, nr::KDSETMODE, nr::KD_GRAPHICS) == 0 {
        unsafe {
            VT_GRAPHICS = true;
        }
    }

    e.init(320, 200, false, host);
    if let Some(m) = args.map {
        e.new_game(hellbyte_engine::game::Skill::from_u8(args.skill), m as usize);
    }
    let pack = |r: u8, g: u8, b: u8| -> u32 {
        let c = |v: u8, off: u32, len: u32| ((v as u32) >> (8 - len.min(8))) << off;
        c(r, ro, rl) | c(g, go, gl) | c(b, bo, bl)
    };
    let mut pal32 = [0u32; 256];
    let mut pal_hash = 0u32;
    // Fit 320x200 at 4:3 in the middle of the screen.
    let (dw, dh) = if xres * 3 > yres * 4 { (yres * 4 / 3, yres) } else { (xres, xres * 3 / 4) };
    let (ox, oy) = ((xres - dw) / 2, (yres - dh) / 2 + yoff);
    let bytespp = bpp / 8;
    // Clear the whole visible screen once.
    for y in 0..yres {
        let row = (y + yoff) * stride;
        screen[row..row + xres * bytespp].fill(0);
    }

    let tic_us: u64 = 1_000_000 / 35;
    let mut next_tic = now_us();
    let mut dec = KeyDecoder::new();
    let mut held = HeldKeys::new();
    let mut ctrl = false;
    let mut evbuf = [0u8; 24 * 32];
    let evsize = 2 * W + 8;
    let mut quit = false;
    while !quit && !e.quit_requested() {
        // evdev input
        for &fd in &evfds[..nev] {
            loop {
                let n = read(fd, &mut evbuf);
                if n <= 0 {
                    break;
                }
                for ev in evbuf[..n as usize].chunks_exact(evsize) {
                    let ty = u16::from_ne_bytes([ev[2 * W], ev[2 * W + 1]]);
                    let code = u16::from_ne_bytes([ev[2 * W + 2], ev[2 * W + 3]]);
                    let val = i32::from_ne_bytes([ev[2 * W + 4], ev[2 * W + 5], ev[2 * W + 6], ev[2 * W + 7]]);
                    match ty {
                        1 => {
                            let k = evkey(code);
                            if k == KEY_CTRL {
                                ctrl = val != 0;
                            }
                            if ctrl && k == b'c' as u16 && val == 1 {
                                quit = true;
                            }
                            if k != 0 && val != 2 {
                                e.key(k, val != 0);
                            }
                        }
                        2 => match code {
                            0 => e.mouse_motion(val * 2, 0),
                            8 if val > 0 => e.key(KEY_WHEELUP, true),
                            8 if val < 0 => e.key(KEY_WHEELDOWN, true),
                            _ => {}
                        },
                        _ => {}
                    }
                }
            }
        }
        // tty input: always drained; used for keys when no evdev device is readable
        let mut tb = [0u8; 64];
        loop {
            let n = read(0, &mut tb);
            if n <= 0 {
                break;
            }
            if nev == 0 {
                let now = now_us() / 1000;
                dec.feed(&tb[..n as usize], &mut |ev| {
                    if held.press(ev.key, now) {
                        e.key(ev.key, true);
                    }
                });
                if dec.quit {
                    quit = true;
                }
            }
        }
        if nev == 0 {
            held.expire(now_us() / 1000, &mut |k| e.key(k, false));
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
        if steps > 0 {
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
            for y in 0..dh {
                let src = &px[(y * sh / dh) * sw..][..sw];
                let row = (oy + y) * stride + ox * bytespp;
                for x in 0..dw {
                    let c = pal32[src[x * sw / dw] as usize];
                    let o = row + x * bytespp;
                    match bytespp {
                        4 => screen[o..o + 4].copy_from_slice(&c.to_ne_bytes()),
                        2 => screen[o..o + 2].copy_from_slice(&(c as u16).to_ne_bytes()),
                        _ => screen[o..o + 3].copy_from_slice(&c.to_le_bytes()[..3]),
                    }
                }
            }
        }
        let wait = next_tic.saturating_sub(now_us());
        if wait > 0 {
            sleep_us(wait.min(4_000));
        }
    }
    // Leave the screen black and give the console back.
    for y in 0..yres {
        let row = (y + yoff) * stride;
        screen[row..row + xres * bytespp].fill(0);
    }
    for &fd in &evfds[..nev] {
        close(fd);
    }
    restore_console();
    Ok(0)
}
