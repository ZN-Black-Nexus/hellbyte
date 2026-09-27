//! Native macOS window. AppKit, QuartzCore and CoreGraphics are loaded at
//! run time with dlopen and driven through the Objective-C runtime, so the
//! binary links nothing but libSystem.

use super::{dlopen, dlsym, now_us, sleep_us};
use crate::common::args::Args;
use hellbyte_engine::keys::*;
use hellbyte_engine::{Engine, Host};

type Id = *mut u8;
type Sel = *mut u8;

#[repr(C)]
#[derive(Clone, Copy)]
struct NSRect {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

struct Rt {
    msg: *mut u8,
    get_class: unsafe extern "C" fn(*const u8) -> Id,
    sel_reg: unsafe extern "C" fn(*const u8) -> Sel,
    pool_push: unsafe extern "C" fn() -> *mut u8,
    pool_pop: unsafe extern "C" fn(*mut u8),
    color_space: unsafe extern "C" fn() -> *mut u8,
    provider: unsafe extern "C" fn(*mut u8, *const u8, usize, *const u8) -> *mut u8,
    image: unsafe extern "C" fn(usize, usize, usize, usize, usize, *mut u8, u32, *mut u8, *const f64, bool, i32) -> *mut u8,
    image_release: unsafe extern "C" fn(*mut u8),
    provider_release: unsafe extern "C" fn(*mut u8),
    assoc_mouse: unsafe extern "C" fn(u32) -> i32,
    run_loop_mode: Id,
    filter_nearest: Id,
    gravity_aspect: Id,
}

impl Rt {
    unsafe fn cls(&self, name: &[u8]) -> Id {
        unsafe { (self.get_class)(name.as_ptr()) }
    }
    unsafe fn sel(&self, name: &[u8]) -> Sel {
        unsafe { (self.sel_reg)(name.as_ptr()) }
    }
    unsafe fn send(&self, obj: Id, sel: &[u8]) -> Id {
        unsafe {
            let f: unsafe extern "C" fn(Id, Sel) -> Id = core::mem::transmute(self.msg);
            f(obj, self.sel(sel))
        }
    }
    unsafe fn send_id(&self, obj: Id, sel: &[u8], a: Id) -> Id {
        unsafe {
            let f: unsafe extern "C" fn(Id, Sel, Id) -> Id = core::mem::transmute(self.msg);
            f(obj, self.sel(sel), a)
        }
    }
    unsafe fn send_u64(&self, obj: Id, sel: &[u8], a: u64) -> Id {
        unsafe {
            let f: unsafe extern "C" fn(Id, Sel, u64) -> Id = core::mem::transmute(self.msg);
            f(obj, self.sel(sel), a)
        }
    }
    unsafe fn send_bool(&self, obj: Id, sel: &[u8], a: bool) -> Id {
        unsafe {
            let f: unsafe extern "C" fn(Id, Sel, i8) -> Id = core::mem::transmute(self.msg);
            f(obj, self.sel(sel), a as i8)
        }
    }
    unsafe fn get_u64(&self, obj: Id, sel: &[u8]) -> u64 {
        unsafe {
            let f: unsafe extern "C" fn(Id, Sel) -> u64 = core::mem::transmute(self.msg);
            f(obj, self.sel(sel))
        }
    }
    unsafe fn get_u16(&self, obj: Id, sel: &[u8]) -> u16 {
        unsafe {
            let f: unsafe extern "C" fn(Id, Sel) -> u16 = core::mem::transmute(self.msg);
            f(obj, self.sel(sel))
        }
    }
    unsafe fn get_bool(&self, obj: Id, sel: &[u8]) -> bool {
        unsafe {
            let f: unsafe extern "C" fn(Id, Sel) -> i8 = core::mem::transmute(self.msg);
            f(obj, self.sel(sel)) != 0
        }
    }
    unsafe fn get_f64(&self, obj: Id, sel: &[u8]) -> f64 {
        unsafe {
            let f: unsafe extern "C" fn(Id, Sel) -> f64 = core::mem::transmute(self.msg);
            f(obj, self.sel(sel))
        }
    }
    unsafe fn nsstring(&self, s: &[u8]) -> Id {
        unsafe { self.send_id(self.cls(b"NSString\0"), b"stringWithUTF8String:\0", s.as_ptr() as Id) }
    }
}

unsafe fn sym(lib: *mut u8, name: &[u8]) -> *mut u8 {
    if lib.is_null() { core::ptr::null_mut() } else { unsafe { dlsym(lib, name.as_ptr()) } }
}

/// Global NSString constants are exported as pointers to the object.
unsafe fn const_id(lib: *mut u8, name: &[u8]) -> Id {
    let p = unsafe { sym(lib, name) } as *const Id;
    if p.is_null() { core::ptr::null_mut() } else { unsafe { *p } }
}

fn keycode(k: u16) -> u16 {
    // macOS virtual key codes (ANSI layout positions).
    const MAP: [u8; 0x33] = [
        b'a', b's', b'd', b'f', b'h', b'g', b'z', b'x', b'c', b'v', 0, b'b', b'q', b'w', b'e', b'r', b'y', b't', b'1', b'2',
        b'3', b'4', b'6', b'5', b'=', b'9', b'7', b'-', b'8', b'0', b']', b'o', b'u', b'[', b'i', b'p', 0, b'l', b'j', b'\'',
        b'k', b';', b'\\', b',', b'/', b'n', b'm', b'.', 0, b' ', b'`',
    ];
    match k {
        0x24 | 0x4c => KEY_ENTER,
        0x30 => KEY_TAB,
        0x31 => KEY_SPACE,
        0x33 => KEY_BACKSPACE,
        0x35 => KEY_ESCAPE,
        0x7b => KEY_LEFT,
        0x7c => KEY_RIGHT,
        0x7d => KEY_DOWN,
        0x7e => KEY_UP,
        0x7a => KEY_F1,
        0x78 => KEY_F2,
        0x63 => KEY_F3,
        0x76 => KEY_F4,
        0x60 => KEY_F5,
        0x61 => KEY_F6,
        0x62 => KEY_F7,
        0x64 => KEY_F8,
        0x65 => KEY_F9,
        0x6d => KEY_F10,
        0x67 => KEY_F11,
        0x6f => KEY_F12,
        0x73 => KEY_HOME,
        0x77 => KEY_END,
        0x74 => KEY_PGUP,
        0x79 => KEY_PGDN,
        0x75 => KEY_DELETE,
        0x45 => b'=' as u16,
        0x4e => b'-' as u16,
        k if (k as usize) < MAP.len() => MAP[k as usize] as u16,
        _ => 0,
    }
}

const PW: usize = 320;
const PH: usize = 240; // shown at 4:3: 200 rows stretched to 240
static mut PIX: [[u8; PW * PH * 4]; 2] = [[0; PW * PH * 4]; 2];

pub fn run(e: &mut Engine, host: &mut dyn Host, args: &Args) -> Result<i32, &'static [u8]> {
    unsafe {
        let objc = dlopen(b"/usr/lib/libobjc.A.dylib\0".as_ptr(), 2);
        let appkit = dlopen(b"/System/Library/Frameworks/AppKit.framework/AppKit\0".as_ptr(), 2);
        let fnd = dlopen(b"/System/Library/Frameworks/Foundation.framework/Foundation\0".as_ptr(), 2);
        let qc = dlopen(b"/System/Library/Frameworks/QuartzCore.framework/QuartzCore\0".as_ptr(), 2);
        let cg = dlopen(b"/System/Library/Frameworks/CoreGraphics.framework/CoreGraphics\0".as_ptr(), 2);
        if objc.is_null() || appkit.is_null() || fnd.is_null() || qc.is_null() || cg.is_null() {
            return Err(b"could not load AppKit");
        }
        let need = |p: *mut u8| -> Result<*mut u8, &'static [u8]> {
            if p.is_null() { Err(b"missing Objective-C/CoreGraphics symbol") } else { Ok(p) }
        };
        let rt = Rt {
            msg: need(sym(objc, b"objc_msgSend\0"))?,
            get_class: core::mem::transmute(need(sym(objc, b"objc_getClass\0"))?),
            sel_reg: core::mem::transmute(need(sym(objc, b"sel_registerName\0"))?),
            pool_push: core::mem::transmute(need(sym(objc, b"objc_autoreleasePoolPush\0"))?),
            pool_pop: core::mem::transmute(need(sym(objc, b"objc_autoreleasePoolPop\0"))?),
            color_space: core::mem::transmute(need(sym(cg, b"CGColorSpaceCreateDeviceRGB\0"))?),
            provider: core::mem::transmute(need(sym(cg, b"CGDataProviderCreateWithData\0"))?),
            image: core::mem::transmute(need(sym(cg, b"CGImageCreate\0"))?),
            image_release: core::mem::transmute(need(sym(cg, b"CGImageRelease\0"))?),
            provider_release: core::mem::transmute(need(sym(cg, b"CGDataProviderRelease\0"))?),
            assoc_mouse: core::mem::transmute(need(sym(cg, b"CGAssociateMouseAndMouseCursorPosition\0"))?),
            run_loop_mode: const_id(fnd, b"NSDefaultRunLoopMode\0"),
            filter_nearest: const_id(qc, b"kCAFilterNearest\0"),
            gravity_aspect: const_id(qc, b"kCAGravityResizeAspect\0"),
        };
        if rt.run_loop_mode.is_null() {
            return Err(b"missing NSDefaultRunLoopMode");
        }
        let pool = (rt.pool_push)();
        let app = rt.send(rt.cls(b"NSApplication\0"), b"sharedApplication\0");
        if app.is_null() {
            (rt.pool_pop)(pool);
            return Err(b"no NSApplication");
        }
        rt.send_u64(app, b"setActivationPolicy:\0", 0);
        // Minimal menu bar so Cmd+Q and Cmd+H work.
        let bar = rt.send(rt.send(rt.cls(b"NSMenu\0"), b"alloc\0"), b"init\0");
        let app_item = rt.send(rt.send(rt.cls(b"NSMenuItem\0"), b"alloc\0"), b"init\0");
        rt.send_id(bar, b"addItem:\0", app_item);
        rt.send_id(app, b"setMainMenu:\0", bar);
        let app_menu = rt.send(rt.send(rt.cls(b"NSMenu\0"), b"alloc\0"), b"init\0");
        let quit_item = {
            let f: unsafe extern "C" fn(Id, Sel, Id, Sel, Id) -> Id = core::mem::transmute(rt.msg);
            let item = rt.send(rt.cls(b"NSMenuItem\0"), b"alloc\0");
            f(item, rt.sel(b"initWithTitle:action:keyEquivalent:\0"), rt.nsstring(b"Quit Hellbyte\0"), rt.sel(b"terminate:\0"), rt.nsstring(b"q\0"))
        };
        rt.send_id(app_menu, b"addItem:\0", quit_item);
        rt.send_id(app_item, b"setSubmenu:\0", app_menu);

        let scale = if args.scale > 0 { args.scale as f64 } else { 3.0 };
        let rect = NSRect { x: 0.0, y: 0.0, w: 320.0 * scale, h: 240.0 * scale };
        let win = {
            let f: unsafe extern "C" fn(Id, Sel, NSRect, u64, u64, i8) -> Id = core::mem::transmute(rt.msg);
            let w = rt.send(rt.cls(b"NSWindow\0"), b"alloc\0");
            // titled | closable | miniaturizable | resizable, buffered backing
            f(w, rt.sel(b"initWithContentRect:styleMask:backing:defer:\0"), rect, 1 | 2 | 4 | 8, 2, 0)
        };
        if win.is_null() {
            (rt.pool_pop)(pool);
            return Err(b"could not create a window");
        }
        rt.send_bool(win, b"setReleasedWhenClosed:\0", false);
        rt.send_id(win, b"setTitle:\0", rt.nsstring(b"Hellbyte\0"));
        rt.send_bool(win, b"setAcceptsMouseMovedEvents:\0", true);
        rt.send(win, b"center\0");
        rt.send_id(win, b"makeKeyAndOrderFront:\0", core::ptr::null_mut());
        if args.fullscreen {
            rt.send_id(win, b"toggleFullScreen:\0", core::ptr::null_mut());
        }
        let view = rt.send(win, b"contentView\0");
        rt.send_bool(view, b"setWantsLayer:\0", true);
        let layer = rt.send(view, b"layer\0");
        if layer.is_null() {
            (rt.pool_pop)(pool);
            return Err(b"no Core Animation layer");
        }
        if !rt.filter_nearest.is_null() {
            rt.send_id(layer, b"setMagnificationFilter:\0", rt.filter_nearest);
        }
        if !rt.gravity_aspect.is_null() {
            rt.send_id(layer, b"setContentsGravity:\0", rt.gravity_aspect);
        }
        rt.send_bool(app, b"activateIgnoringOtherApps:\0", true);
        rt.send(app, b"finishLaunching\0");
        (rt.pool_pop)(pool);

        e.init(320, 200, false, host);
        if let Some(m) = args.map {
            e.new_game(hellbyte_engine::game::Skill::from_u8(args.skill), m as usize);
        }
        let space = (rt.color_space)();
        let tic_us: u64 = 1_000_000 / 35;
        let mut next_tic = now_us();
        let mut mods: u64 = 0;
        let mut grabbed = false;
        let mut buf = 0usize;
        let pix = &mut *core::ptr::addr_of_mut!(PIX);
        loop {
            let pool = (rt.pool_push)();
            let past = rt.send(rt.cls(b"NSDate\0"), b"distantPast\0");
            loop {
                let ev = {
                    let f: unsafe extern "C" fn(Id, Sel, u64, Id, Id, i8) -> Id = core::mem::transmute(rt.msg);
                    f(app, rt.sel(b"nextEventMatchingMask:untilDate:inMode:dequeue:\0"), u64::MAX, past, rt.run_loop_mode, 1)
                };
                if ev.is_null() {
                    break;
                }
                let ty = rt.get_u64(ev, b"type\0");
                let mut forward = true;
                match ty {
                    10 | 11 => {
                        let m = rt.get_u64(ev, b"modifierFlags\0");
                        if m & (1 << 20) == 0 {
                            // not a Cmd shortcut: it's ours
                            forward = false;
                            let repeat = ty == 10 && rt.get_bool(ev, b"isARepeat\0");
                            let k = keycode(rt.get_u16(ev, b"keyCode\0"));
                            if k != 0 && !(repeat && !e.menu.active) {
                                e.key(k, ty == 10);
                            }
                        }
                    }
                    12 => {
                        let m = rt.get_u64(ev, b"modifierFlags\0");
                        for (bit, key) in [(17, KEY_SHIFT), (18, KEY_CTRL), (19, KEY_ALT)] {
                            let (was, now) = (mods & (1 << bit) != 0, m & (1 << bit) != 0);
                            if was != now {
                                e.key(key, now);
                            }
                        }
                        mods = m;
                    }
                    1 | 2 | 3 | 4 | 25 | 26 => {
                        if grabbed || ty == 2 || ty == 4 || ty == 26 {
                            let (b, down) = match ty {
                                1 => (0, true),
                                2 => (0, false),
                                3 => (1, true),
                                4 => (1, false),
                                25 => (2, true),
                                _ => (2, false),
                            };
                            e.mouse_button(b, down);
                        } else if ty == 1 {
                            e.mouse_button(0, true);
                        }
                        forward = !grabbed;
                    }
                    5 | 6 | 7 | 27 => {
                        if grabbed {
                            let dx = rt.get_f64(ev, b"deltaX\0");
                            e.mouse_motion((dx * 2.0) as i32, 0);
                            forward = false;
                        }
                    }
                    22 => {
                        let dy = rt.get_f64(ev, b"scrollingDeltaY\0");
                        if dy > 0.5 {
                            e.key(KEY_WHEELUP, true);
                        } else if dy < -0.5 {
                            e.key(KEY_WHEELDOWN, true);
                        }
                    }
                    _ => {}
                }
                if forward {
                    rt.send_id(app, b"sendEvent:\0", ev);
                }
            }
            rt.send(app, b"updateWindows\0");
            let visible = rt.get_bool(win, b"isVisible\0");
            let key_window = rt.get_bool(win, b"isKeyWindow\0");
            if !visible || e.quit_requested() {
                (rt.pool_pop)(pool);
                break;
            }
            if !key_window {
                e.release_all();
            }
            let want = e.wants_pointer() && key_window && !args.nomouse;
            if want != grabbed {
                grabbed = want;
                (rt.assoc_mouse)(if want { 0 } else { 1 });
                rt.send(rt.cls(b"NSCursor\0"), if want { b"hide\0" as &[u8] } else { b"unhide\0" });
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
                let (px, sw, sh) = e.screen();
                let pal = e.palette();
                buf ^= 1;
                let out = &mut pix[buf];
                for y in 0..PH {
                    let sy = (y * sh / PH).min(sh - 1);
                    for x in 0..PW {
                        let c = px[sy * sw + (x * sw / PW)] as usize * 3;
                        let o = (y * PW + x) * 4;
                        out[o] = pal[c];
                        out[o + 1] = pal[c + 1];
                        out[o + 2] = pal[c + 2];
                        out[o + 3] = 255;
                    }
                }
                let prov = (rt.provider)(core::ptr::null_mut(), out.as_ptr(), out.len(), core::ptr::null());
                // 8 bits per component, 32 per pixel, RGBX (kCGImageAlphaNoneSkipLast = 5)
                let img = (rt.image)(PW, PH, 8, 32, PW * 4, space, 5, prov, core::ptr::null(), false, 0);
                rt.send_id(layer, b"setContents:\0", img);
                rt.send(rt.cls(b"CATransaction\0"), b"flush\0");
                (rt.image_release)(img);
                (rt.provider_release)(prov);
            }
            (rt.pool_pop)(pool);
            let wait = next_tic.saturating_sub(now_us());
            if wait > 1000 {
                sleep_us(wait.min(4_000));
            }
        }
        if grabbed {
            (rt.assoc_mouse)(1);
            rt.send(rt.cls(b"NSCursor\0"), b"unhide\0");
        }
    }
    Ok(0)
}
