//! Win32/GDI window: an 8-bit palette DIB stretched to the window.

use super::sys::*;
use super::{fatal, vk_key};
use crate::common::args::Args;
use hellbyte_engine::keys::*;
use hellbyte_engine::{Engine, Host};

const WM_DESTROY: u32 = 0x0002;
const WM_SIZE: u32 = 0x0005;
const WM_KILLFOCUS: u32 = 0x0008;
const WM_PAINT: u32 = 0x000f;
const WM_CLOSE: u32 = 0x0010;
const WM_ERASEBKGND: u32 = 0x0014;
const WM_SETCURSOR: u32 = 0x0020;
const WM_INPUT: u32 = 0x00ff;
const WM_KEYDOWN: u32 = 0x0100;
const WM_KEYUP: u32 = 0x0101;
const WM_SYSKEYDOWN: u32 = 0x0104;
const WM_SYSKEYUP: u32 = 0x0105;
const WM_LBUTTONDOWN: u32 = 0x0201;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_RBUTTONDOWN: u32 = 0x0204;
const WM_RBUTTONUP: u32 = 0x0205;
const WM_MBUTTONDOWN: u32 = 0x0207;
const WM_MBUTTONUP: u32 = 0x0208;
const WM_MOUSEWHEEL: u32 = 0x020a;

const WS_OVERLAPPEDWINDOW: u32 = 0x00cf_0000;
const WS_POPUP: u32 = 0x8000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const GWL_STYLE: i32 = -16;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_FRAMECHANGED: u32 = 0x0020;
const VK_RETURN: usize = 0x0d;

struct State {
    engine: *mut Engine,
    quit: bool,
    grabbed: bool,
    focused: bool,
    dirty: bool,
    w: i32,
    h: i32,
    /// Borderless window covering the monitor.
    full: bool,
    /// Where the normal window goes when leaving fullscreen.
    windowed: RECT,
}

static mut ST: State = State {
    engine: core::ptr::null_mut(),
    quit: false,
    grabbed: false,
    focused: true,
    dirty: true,
    w: 960,
    h: 720,
    full: false,
    windowed: RECT { left: 0, top: 0, right: 0, bottom: 0 },
};
static mut BMI: BITMAPINFO256 = BITMAPINFO256 {
    biSize: 40,
    biWidth: 320,
    biHeight: -200,
    biPlanes: 1,
    biBitCount: 8,
    biCompression: 0,
    biSizeImage: 0,
    biXPelsPerMeter: 0,
    biYPelsPerMeter: 0,
    biClrUsed: 256,
    biClrImportant: 256,
    colors: [0; 256],
};

fn st() -> &'static mut State {
    // SAFETY: the window procedure and main loop run on the same thread.
    unsafe { &mut *core::ptr::addr_of_mut!(ST) }
}

fn engine() -> Option<&'static mut Engine> {
    let p = st().engine;
    if p.is_null() { None } else { Some(unsafe { &mut *p }) }
}

/// Alt+Enter: switch between the normal window and a borderless window
/// covering the monitor it is on.
fn toggle_fullscreen(hwnd: HWND) {
    let s = st();
    // SAFETY: plain Win32 calls on our own window.
    unsafe {
        let r = if s.full {
            SetWindowLongW(hwnd, GWL_STYLE, (WS_OVERLAPPEDWINDOW | WS_VISIBLE) as i32);
            s.windowed
        } else {
            GetWindowRect(hwnd, &mut s.windowed);
            let mut mi = MONITORINFO { cbSize: core::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
            GetMonitorInfoW(MonitorFromWindow(hwnd, 2), &mut mi); // nearest monitor
            SetWindowLongW(hwnd, GWL_STYLE, (WS_POPUP | WS_VISIBLE) as i32);
            mi.rcMonitor
        };
        SetWindowPos(hwnd, 0, r.left, r.top, r.right - r.left, r.bottom - r.top, SWP_NOZORDER | SWP_FRAMECHANGED);
        s.full = !s.full;
        if s.grabbed {
            let mut w = RECT::default();
            GetWindowRect(hwnd, &mut w);
            ClipCursor(&w);
        }
    }
    s.dirty = true;
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: usize, lp: isize) -> isize {
    let s = st();
    match msg {
        WM_CLOSE | WM_DESTROY => {
            s.quit = true;
            return 0;
        }
        WM_ERASEBKGND => return 1,
        WM_SIZE => {
            s.w = (lp & 0xffff) as i32;
            s.h = ((lp >> 16) & 0xffff) as i32;
            s.dirty = true;
        }
        WM_PAINT => {
            let mut ps: PAINTSTRUCT = unsafe { core::mem::zeroed() };
            unsafe {
                BeginPaint(hwnd, &mut ps);
                EndPaint(hwnd, &ps);
            }
            s.dirty = true;
            return 0;
        }
        WM_KILLFOCUS => {
            s.focused = false;
            if let Some(e) = engine() {
                e.release_all();
            }
        }
        WM_SETCURSOR => {
            if s.grabbed && (lp & 0xffff) == 1 {
                unsafe {
                    SetCursor(0);
                }
                return 1;
            }
        }
        WM_SYSKEYDOWN if wp == VK_RETURN && (lp & (1 << 29)) != 0 => {
            if (lp & (1 << 30)) == 0 {
                toggle_fullscreen(hwnd); // Alt+Enter (not its auto-repeat)
            }
            return 0;
        }
        WM_KEYDOWN | WM_SYSKEYDOWN | WM_KEYUP | WM_SYSKEYUP => {
            let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            let k = vk_key(wp as u16);
            if let Some(e) = engine() {
                // Ignore auto-repeat except in menus (lp bit 30 = was already down).
                let repeat = down && (lp & (1 << 30)) != 0;
                if k != 0 && !(repeat && !e.menu.active) {
                    e.key(k, down);
                }
                if down && k == KEY_F4 && (lp & (1 << 29)) != 0 {
                    s.quit = true; // Alt+F4
                }
            }
            return 0; // swallow Alt/F10 so the system menu doesn't steal focus
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONUP | WM_MBUTTONDOWN | WM_MBUTTONUP => {
            s.focused = true;
            if let Some(e) = engine() {
                let (b, down) = match msg {
                    WM_LBUTTONDOWN => (0, true),
                    WM_LBUTTONUP => (0, false),
                    WM_RBUTTONDOWN => (1, true),
                    WM_RBUTTONUP => (1, false),
                    WM_MBUTTONDOWN => (2, true),
                    _ => (2, false),
                };
                e.mouse_button(b, down);
            }
            return 0;
        }
        WM_MOUSEWHEEL => {
            if let Some(e) = engine() {
                let delta = ((wp >> 16) & 0xffff) as i16;
                e.key(if delta > 0 { KEY_WHEELUP } else { KEY_WHEELDOWN }, true);
            }
            return 0;
        }
        WM_INPUT => {
            let mut raw: RAWMOUSEINPUT = unsafe { core::mem::zeroed() };
            let mut size = core::mem::size_of::<RAWMOUSEINPUT>() as u32;
            let hdr = (2 * core::mem::size_of::<usize>() + 8) as u32;
            let got = unsafe { GetRawInputData(lp, 0x1000_0003, &mut raw as *mut _ as *mut u8, &mut size, hdr) };
            if got != u32::MAX && raw.dwType == 0 && s.grabbed && raw.usFlags & 1 == 0 {
                if let Some(e) = engine() {
                    e.mouse_motion(raw.lLastX * 2, raw.lLastY);
                }
            }
        }
        _ => {}
    }
    unsafe { DefWindowProcW(hwnd, msg, wp, lp) }
}

pub fn run(e: &mut Engine, host: &mut dyn Host, args: &Args) -> i32 {
    let s = st();
    s.engine = e as *mut Engine;
    let mut cls_name = [0u16; 16];
    let mut title = [0u16; 16];
    let cls = wide(b"HellbyteWnd", &mut cls_name).as_ptr();
    unsafe {
        let inst = GetModuleHandleW(core::ptr::null());
        let wc = WNDCLASSEXW {
            cbSize: core::mem::size_of::<WNDCLASSEXW>() as u32,
            style: 0x3, // CS_HREDRAW | CS_VREDRAW
            lpfnWndProc: wndproc,
            cbClsExtra: 0,
            cbWndExtra: 0,
            hInstance: inst,
            hIcon: 0,
            hCursor: LoadCursorW(0, 32512 as *const u16), // IDC_ARROW
            hbrBackground: 0,
            lpszMenuName: core::ptr::null(),
            lpszClassName: cls,
            hIconSm: 0,
        };
        if RegisterClassExW(&wc) == 0 {
            fatal(b"Could not register the window class.");
        }
        let scale = if args.scale > 0 { args.scale as i32 } else { 3 };
        let mut r = RECT { left: 0, top: 0, right: 320 * scale, bottom: 240 * scale };
        AdjustWindowRect(&mut r, WS_OVERLAPPEDWINDOW, 0);
        let (ww, wh) = (r.right - r.left, r.bottom - r.top);
        let (sw, sh) = (GetSystemMetrics(0), GetSystemMetrics(1));
        let (style, x, y, w, h) = if args.fullscreen {
            // Alt+Enter later brings back a normal window in the middle of the screen.
            s.full = true;
            s.windowed = RECT { left: (sw - ww) / 2, top: (sh - wh) / 2, right: (sw + ww) / 2, bottom: (sh + wh) / 2 };
            (WS_POPUP | WS_VISIBLE, 0, 0, sw, sh)
        } else {
            (WS_OVERLAPPEDWINDOW | WS_VISIBLE, 0x8000_0000u32 as i32, 0x8000_0000u32 as i32, ww, wh)
        };
        let hwnd = CreateWindowExW(0, cls, wide(b"Hellbyte", &mut title).as_ptr(), style, x, y, w, h, 0, 0, inst, core::ptr::null());
        if hwnd == 0 {
            fatal(b"Could not create the window.");
        }
        ShowWindow(hwnd, 1);
        let rid = RAWINPUTDEVICE { usUsagePage: 1, usUsage: 2, dwFlags: 0, hwndTarget: hwnd };
        RegisterRawInputDevices(&rid, 1, core::mem::size_of::<RAWINPUTDEVICE>() as u32);

        e.init(320, 200, false, host);
        if let Some(m) = args.map {
            e.new_game(hellbyte_engine::game::Skill::from_u8(args.skill), m as usize);
        }
        let tic_us: u64 = 1_000_000 / 35;
        let mut next_tic = now_us();
        let mut msg: MSG = core::mem::zeroed();
        let bmi = &mut *core::ptr::addr_of_mut!(BMI);
        while !s.quit && !e.quit_requested() {
            while PeekMessageW(&mut msg, 0, 0, 0, 1) != 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            s.focused = GetForegroundWindow() == hwnd;
            let want = e.wants_pointer() && s.focused && !args.nomouse;
            if want != s.grabbed {
                s.grabbed = want;
                if want {
                    let mut r = RECT::default();
                    GetWindowRect(hwnd, &mut r);
                    ClipCursor(&r);
                    ShowCursor(0);
                } else {
                    ClipCursor(core::ptr::null());
                    ShowCursor(1);
                }
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
            if steps > 0 || s.dirty {
                s.dirty = false;
                e.draw();
                let pal = e.palette();
                for i in 0..256 {
                    bmi.colors[i] = (pal[i * 3] as u32) << 16 | (pal[i * 3 + 1] as u32) << 8 | pal[i * 3 + 2] as u32;
                }
                let (px, sw, sh) = e.screen();
                bmi.biWidth = sw as i32;
                bmi.biHeight = -(sh as i32);
                let mut rc = RECT::default();
                GetClientRect(hwnd, &mut rc);
                let (cw, ch) = (rc.right.max(1), rc.bottom.max(1));
                let (dw, dh) = if cw * 3 > ch * 4 { (ch * 4 / 3, ch) } else { (cw, cw * 3 / 4) };
                let (ox, oy) = ((cw - dw) / 2, (ch - dh) / 2);
                let dc = GetDC(hwnd);
                // Black letterbox bars (BLACKNESS raster op).
                if ox > 0 {
                    PatBlt(dc, 0, 0, ox, ch, 0x42);
                    PatBlt(dc, ox + dw, 0, cw - ox - dw, ch, 0x42);
                }
                if oy > 0 {
                    PatBlt(dc, 0, 0, cw, oy, 0x42);
                    PatBlt(dc, 0, oy + dh, cw, ch - oy - dh, 0x42);
                }
                StretchDIBits(dc, ox, oy, dw, dh, 0, 0, sw as i32, sh as i32, px.as_ptr(), bmi, 0, 0x00cc_0020);
                ReleaseDC(hwnd, dc);
            }
            let wait = next_tic.saturating_sub(now_us());
            if wait > 1000 {
                sleep_us(wait.min(4_000));
            }
        }
        ClipCursor(core::ptr::null());
    }
    0
}
