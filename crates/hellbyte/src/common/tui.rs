//! Terminal renderer and keyboard decoder, shared by every OS.
//!
//! The 3D view is drawn with "upper half block" characters: each character
//! cell shows two vertical pixels (foreground = top, background = bottom).
//! Works in truecolor, 256-colour, 16-colour and plain-ASCII terminals, over
//! SSH or a serial line. Only changed rows are re-sent.

use hellbyte_engine::keys::*;
use hellbyte_engine::{Engine, TextKind};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Colors {
    True,
    X256,
    Ansi16,
    Ascii,
}

impl Colors {
    /// Pick a colour mode from the usual environment variables.
    pub fn detect(term: Option<&[u8]>, colorterm: Option<&[u8]>, forced: u16) -> Colors {
        match forced {
            24 => return Colors::True,
            255 | 256 => return Colors::X256,
            16 | 8 => return Colors::Ansi16,
            1 | 2 => return Colors::Ascii,
            _ => {}
        }
        if let Some(ct) = colorterm {
            if ct == b"truecolor" || ct == b"24bit" {
                return Colors::True;
            }
        }
        let t = term.unwrap_or(b"");
        if t.windows(3).any(|w| w == b"256") || t.starts_with(b"xterm-kitty") || t.starts_with(b"alacritty") || t.starts_with(b"foot") {
            Colors::X256
        } else if t == b"dumb" || t.starts_with(b"vt1") || t.starts_with(b"vt2") || t.is_empty() {
            Colors::Ascii
        } else {
            Colors::Ansi16
        }
    }
}

const OUT: usize = 16384;
const MAX_ROWS: usize = 128;

pub struct Tui {
    pub colors: Colors,
    pub cols: usize,
    pub rows: usize,
    out: [u8; OUT],
    len: usize,
    row_hash: [u32; MAX_ROWS],
    xterm: [u8; 256],
    ansi: [u8; 256],
    luma: [u8; 256],
    pal_hash: u32,
    pub force: bool,
}

/// The 16 standard ANSI colours (approximate RGB) used for mapping.
const ANSI_RGB: [[u8; 3]; 16] = [
    [0, 0, 0],
    [170, 0, 0],
    [0, 170, 0],
    [170, 85, 0],
    [0, 0, 170],
    [170, 0, 170],
    [0, 170, 170],
    [170, 170, 170],
    [85, 85, 85],
    [255, 85, 85],
    [85, 255, 85],
    [255, 255, 85],
    [85, 85, 255],
    [255, 85, 255],
    [85, 255, 255],
    [255, 255, 255],
];

/// "Redmean" colour distance: cheap, and keeps hues far better than plain
/// luminance-weighted RGB when squeezing into 256 or 16 colours.
fn dist(a: [i32; 3], b: [i32; 3]) -> i32 {
    let rmean = (a[0] + b[0]) / 2;
    let (dr, dg, db) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    (((512 + rmean) * dr * dr) >> 8) + 4 * dg * dg + (((767 - rmean) * db * db) >> 8)
}

fn xterm_rgb(i: usize) -> [i32; 3] {
    if i < 16 {
        let c = ANSI_RGB[i];
        return [c[0] as i32, c[1] as i32, c[2] as i32];
    }
    if i < 232 {
        let v = i - 16;
        let lvl = |x: usize| if x == 0 { 0 } else { 55 + 40 * x as i32 };
        return [lvl(v / 36), lvl((v / 6) % 6), lvl(v % 6)];
    }
    let g = 8 + 10 * (i as i32 - 232);
    [g, g, g]
}

impl Tui {
    pub const fn new() -> Tui {
        Tui {
            colors: Colors::X256,
            cols: 80,
            rows: 24,
            out: [0; OUT],
            len: 0,
            row_hash: [0; MAX_ROWS],
            xterm: [0; 256],
            ansi: [0; 256],
            luma: [0; 256],
            pal_hash: 1,
            force: true,
        }
    }

    /// Render resolution for the current terminal size (one text row is kept
    /// for the status line).
    pub fn view_size(&self) -> (usize, usize) {
        let w = self.cols.clamp(16, 320);
        let h = ((self.rows.saturating_sub(1)) * 2).clamp(8, 200);
        (w, h)
    }

    fn put(&mut self, b: &[u8], w: &mut dyn FnMut(&[u8])) {
        for &c in b {
            if self.len == OUT {
                w(&self.out[..self.len]);
                self.len = 0;
            }
            self.out[self.len] = c;
            self.len += 1;
        }
    }

    fn num(&mut self, v: u32, w: &mut dyn FnMut(&[u8])) {
        let mut t = [0u8; 10];
        let mut i = 10;
        let mut n = v;
        loop {
            i -= 1;
            t[i] = b'0' + (n % 10) as u8;
            n /= 10;
            if n == 0 {
                break;
            }
        }
        self.put(&t[i..], w);
    }

    pub fn flush(&mut self, w: &mut dyn FnMut(&[u8])) {
        if self.len > 0 {
            w(&self.out[..self.len]);
            self.len = 0;
        }
    }

    /// Escape sequence to enter full-screen mode (alternate screen, hidden cursor).
    pub const ENTER: &'static [u8] = b"\x1b[?1049h\x1b[?25l\x1b[?7l\x1b[2J";
    /// Undo everything we changed, including the kitty keyboard mode.
    pub const LEAVE: &'static [u8] =
        b"\x1b[?1016l\x1b[?1006l\x1b[?1003l\x1b[<u\x1b[0m\x1b[?7h\x1b[?25h\x1b[?1049l";
    /// Ask the terminal for "report key releases" (kitty protocol, flags 1|2|8).
    pub const KITTY_ON: &'static [u8] = b"\x1b[>11u";
    /// Query whether the kitty protocol (reply: CSI ? flags u) and pixel mouse
    /// positions (CSI ? 1016 ; n $ y) are supported. The device-attributes
    /// query at the end is answered by every terminal, so its reply tells us
    /// all earlier answers are in.
    pub const KITTY_QUERY: &'static [u8] = b"\x1b[?u\x1b[?1016$p\x1b[c";
    /// Report mouse buttons and all motion, in the SGR format.
    pub const MOUSE_ON: &'static [u8] = b"\x1b[?1003h\x1b[?1006h";
    /// Mouse positions in pixels instead of character cells.
    pub const MOUSE_PIXELS: &'static [u8] = b"\x1b[?1016h";

    fn update_maps(&mut self, pal: &[u8; 768]) {
        let mut h: u32 = 0x811c_9dc5;
        for &b in pal.iter() {
            h = (h ^ b as u32).wrapping_mul(0x0100_0193);
        }
        if h == self.pal_hash {
            return;
        }
        self.pal_hash = h;
        self.force = true;
        for i in 0..256 {
            let c = [pal[i * 3] as i32, pal[i * 3 + 1] as i32, pal[i * 3 + 2] as i32];
            let mut best = (i32::MAX, 0);
            for x in 16..256 {
                let d = dist(c, xterm_rgb(x));
                if d < best.0 {
                    best = (d, x);
                }
            }
            self.xterm[i] = best.1 as u8;
            let mut best = (i32::MAX, 0);
            for (x, a) in ANSI_RGB.iter().enumerate() {
                let d = dist(c, [a[0] as i32, a[1] as i32, a[2] as i32]);
                if d < best.0 {
                    best = (d, x);
                }
            }
            self.ansi[i] = best.1 as u8;
            self.luma[i] = ((c[0] * 3 + c[1] * 6 + c[2]) / 10) as u8;
        }
    }

    fn color(&mut self, fg: bool, c: u8, pal: &[u8; 768], w: &mut dyn FnMut(&[u8])) {
        match self.colors {
            Colors::True => {
                self.put(if fg { b"\x1b[38;2;" } else { b"\x1b[48;2;" }, w);
                let i = c as usize * 3;
                self.num(pal[i] as u32, w);
                self.put(b";", w);
                self.num(pal[i + 1] as u32, w);
                self.put(b";", w);
                self.num(pal[i + 2] as u32, w);
                self.put(b"m", w);
            }
            Colors::X256 => {
                self.put(if fg { b"\x1b[38;5;" } else { b"\x1b[48;5;" }, w);
                let x = self.xterm[c as usize] as u32;
                self.num(x, w);
                self.put(b"m", w);
            }
            Colors::Ansi16 => {
                let a = self.ansi[c as usize] as u32;
                let code = if fg { if a < 8 { 30 + a } else { 90 + a - 8 } } else if a < 8 { 40 + a } else { 100 + a - 8 };
                self.put(b"\x1b[", w);
                self.num(code, w);
                self.put(b"m", w);
            }
            Colors::Ascii => {}
        }
    }

    /// Draw one frame: the 3D view, then status/message/menu text.
    pub fn frame(&mut self, e: &Engine, w: &mut dyn FnMut(&[u8])) {
        let (px, sw, sh) = e.screen();
        let pal = *e.palette();
        self.update_maps(&pal);
        self.put(b"\x1b[?2026h", w); // synchronized update (ignored if unsupported)
        if self.force {
            self.put(b"\x1b[0m\x1b[2J", w);
            self.row_hash = [0; MAX_ROWS];
        }
        let rows = (sh / 2).min(self.rows.saturating_sub(1)).min(MAX_ROWS);
        let cols = sw.min(self.cols);
        let menu = e.menu.active;
        for r in 0..rows {
            let top = &px[(r * 2) * sw..(r * 2) * sw + cols];
            let bot = &px[(r * 2 + 1) * sw..(r * 2 + 1) * sw + cols];
            let mut h: u32 = 0x811c_9dc5 ^ (menu as u32);
            for i in 0..cols {
                h = (h ^ top[i] as u32).wrapping_mul(0x0100_0193);
                h = (h ^ bot[i] as u32).wrapping_mul(0x0100_0193);
            }
            if h == 0 {
                h = 1;
            }
            if !self.force && self.row_hash[r] == h {
                continue;
            }
            self.row_hash[r] = h;
            self.put(b"\x1b[", w);
            self.num(r as u32 + 1, w);
            self.put(b";1H", w);
            let dim = |c: u8| -> u8 {
                if menu { hellbyte_engine::data::COLORMAPS[22 * 256 + c as usize] } else { c }
            };
            if self.colors == Colors::Ascii {
                const RAMP: &[u8] = b" .:-=+*#%@";
                for i in 0..cols {
                    let l = (self.luma[dim(top[i]) as usize] as usize + self.luma[dim(bot[i]) as usize] as usize) / 2;
                    let ch = RAMP[(l * (RAMP.len() - 1) + 127) / 255];
                    self.put(&[ch], w);
                }
                continue;
            }
            let (mut lf, mut lb) = (-1i32, -1i32);
            for i in 0..cols {
                let (t, b) = (dim(top[i]), dim(bot[i]));
                let (tk, bk) = match self.colors {
                    Colors::X256 => (self.xterm[t as usize] as i32, self.xterm[b as usize] as i32),
                    Colors::Ansi16 => (self.ansi[t as usize] as i32, self.ansi[b as usize] as i32),
                    _ => (t as i32, b as i32),
                };
                if tk == bk {
                    // Same colour top and bottom: a space with only a background.
                    if bk != lb {
                        self.color(false, b, &pal, w);
                        lb = bk;
                    }
                    self.put(b" ", w);
                } else {
                    if tk != lf {
                        self.color(true, t, &pal, w);
                        lf = tk;
                    }
                    if bk != lb {
                        self.color(false, b, &pal, w);
                        lb = bk;
                    }
                    self.put("\u{2580}".as_bytes(), w);
                }
            }
            self.put(b"\x1b[0m", w);
        }
        self.force = false;
        // Text overlay: status on the last line, message at the top, menus centred.
        let status_row = self.rows;
        let mut menu_row = 0usize;
        let mut lines = 0usize;
        e.text_overlay(&mut |_, _| lines += 1);
        let first_menu_row = (rows / 2).saturating_sub(lines / 2) + 1;
        let cols_u = self.cols;
        e.text_overlay(&mut |kind, text| {
            let t = &text[..text.len().min(cols_u)];
            match kind {
                TextKind::Status => {
                    self.goto(status_row, 1, w);
                    self.put(b"\x1b[0m\x1b[1;33;40m ", w);
                    self.put(t, w);
                    self.put(b"\x1b[K\x1b[0m", w);
                }
                TextKind::Message => {
                    self.goto(1, 1, w);
                    self.put(b"\x1b[0m\x1b[1;37;41m ", w);
                    self.put(t, w);
                    self.put(b" \x1b[0m", w);
                    self.row_hash[0] = 0;
                }
                _ => {
                    let row = first_menu_row + menu_row;
                    menu_row += 1;
                    let col = cols_u.saturating_sub(t.len()) / 2 + 1;
                    self.goto(row, col.saturating_sub(2), w);
                    let style: &[u8] = match kind {
                        TextKind::Title => b"\x1b[0m\x1b[1;31;40m  ",
                        TextKind::Selected => b"\x1b[0m\x1b[1;30;43m> ",
                        _ => b"\x1b[0m\x1b[37;40m  ",
                    };
                    self.put(style, w);
                    self.put(t, w);
                    self.put(b"  \x1b[0m", w);
                    if row >= 1 && row - 1 < MAX_ROWS {
                        self.row_hash[row - 1] = 0;
                    }
                }
            }
        });
        if lines == 0 {
            self.goto(status_row, 1, w);
            self.put(b"\x1b[0m\x1b[K", w);
        }
        self.put(b"\x1b[?2026l", w);
        self.flush(w);
    }

    fn goto(&mut self, row: usize, col: usize, w: &mut dyn FnMut(&[u8])) {
        self.put(b"\x1b[", w);
        self.num(row as u32, w);
        self.put(b";", w);
        self.num(col.max(1) as u32, w);
        self.put(b"H", w);
    }
}

// ====================================================================== keyboard

/// Decodes terminal input bytes into key events. Understands plain bytes,
/// VT/xterm escape sequences and the kitty keyboard protocol (which reports
/// real key releases).
pub struct KeyDecoder {
    buf: [u8; 64],
    len: usize,
    pub kitty: bool,
    pub quit: bool,
    /// The terminal answered the device-attributes query (so it has also
    /// answered everything asked before it).
    pub answered: bool,
    /// The terminal can report mouse positions in pixels.
    pub pixel_mouse: bool,
    /// Last mouse position (1-based cells, or pixels in pixel mode).
    pub mouse: Option<(i32, i32)>,
}

pub struct KeyEvent {
    pub key: u16,
    pub down: bool,
    pub shift: bool,
}

impl KeyDecoder {
    pub const fn new() -> KeyDecoder {
        KeyDecoder { buf: [0; 64], len: 0, kitty: false, quit: false, answered: false, pixel_mouse: false, mouse: None }
    }

    /// Feed raw bytes; complete keys are passed to `emit`. A lone ESC at the
    /// end of a read is treated as the Escape key.
    pub fn feed(&mut self, data: &[u8], emit: &mut dyn FnMut(KeyEvent)) {
        for &b in data {
            if self.len < self.buf.len() {
                self.buf[self.len] = b;
                self.len += 1;
            }
        }
        let mut i = 0;
        let buf = self.buf; // decode() updates our other fields
        while i < self.len {
            let rest = &buf[i..self.len];
            match decode(rest, self) {
                Decoded::Key(n, k, down, shift) => {
                    if k == 3 {
                        self.quit = true; // Ctrl+C
                    }
                    if k != 0 {
                        emit(KeyEvent { key: k, down, shift });
                    }
                    i += n;
                }
                Decoded::Skip(n) => i += n,
                Decoded::Incomplete => {
                    if rest == [0x1b] {
                        emit(KeyEvent { key: KEY_ESCAPE, down: true, shift: false });
                        i += 1;
                    } else {
                        break;
                    }
                }
            }
        }
        self.buf.copy_within(i..self.len, 0);
        self.len -= i;
        if self.len == self.buf.len() {
            self.len = 0; // garbage; resync
        }
    }
}

enum Decoded {
    /// consumed bytes, key, down, shift
    Key(usize, u16, bool, bool),
    Skip(usize),
    Incomplete,
}

fn csi_final(rest: &[u8]) -> Option<usize> {
    // rest starts with ESC [ ; find the final byte 0x40..0x7e
    for (i, &c) in rest.iter().enumerate().skip(2) {
        if (0x40..=0x7e).contains(&c) {
            return Some(i);
        }
    }
    None
}

fn params(p: &[u8], out: &mut [u32; 4]) -> usize {
    let mut n = 0;
    let mut v = 0u32;
    let mut any = false;
    for &c in p {
        if c.is_ascii_digit() {
            v = v.saturating_mul(10).saturating_add((c - b'0') as u32);
            any = true;
        } else if c == b';' || c == b':' {
            if n < 4 {
                out[n] = if any { v } else { 0 };
            }
            n += 1;
            v = 0;
            any = false;
        }
    }
    if n < 4 {
        out[n] = if any { v } else { 0 };
    }
    n + 1
}

fn decode(rest: &[u8], st: &mut KeyDecoder) -> Decoded {
    let b = rest[0];
    if b != 0x1b {
        let key = match b {
            b'\r' | b'\n' => KEY_ENTER,
            0x7f | 0x08 => KEY_BACKSPACE,
            b'\t' => KEY_TAB,
            0x03 => 3,
            b'A'..=b'Z' => return Decoded::Key(1, (b + 32) as u16, true, true),
            0x20..=0x7e => b as u16,
            _ => 0,
        };
        return Decoded::Key(1, key, true, false);
    }
    if rest.len() < 2 {
        return Decoded::Incomplete;
    }
    match rest[1] {
        b'[' => {
            let Some(end) = csi_final(rest) else { return Decoded::Incomplete };
            let fin = rest[end];
            let body = &rest[2..end];
            if body.first() == Some(&b'?') {
                // Replies to our queries: CSI ? flags u (kitty), CSI ? 1016 ; n $ y
                // (pixel mouse: 1 = on, 2 = off but supported), CSI ? ... c.
                let mut p = [0u32; 4];
                params(&body[1..], &mut p);
                match fin {
                    b'u' => st.kitty = true,
                    b'y' if p[0] == 1016 && matches!(p[1], 1 | 2) => st.pixel_mouse = true,
                    b'c' => st.answered = true,
                    _ => {}
                }
                return Decoded::Skip(end + 1);
            }
            if body.first() == Some(&b'<') {
                // SGR mouse: CSI < button ; x ; y (M = press/motion, m = release)
                let mut p = [0u32; 4];
                params(&body[1..], &mut p);
                let b = p[0];
                st.mouse = Some((p[1] as i32, p[2] as i32));
                let key = if b & 64 != 0 {
                    if b & 1 == 0 { KEY_WHEELUP } else { KEY_WHEELDOWN }
                } else if b & 32 != 0 {
                    0 // motion only
                } else {
                    match b & 3 {
                        0 => KEY_MOUSE1,
                        1 => KEY_MOUSE3,
                        2 => KEY_MOUSE2,
                        _ => 0,
                    }
                };
                if key == 0 || (b & 64 != 0 && fin == b'm') {
                    return Decoded::Skip(end + 1);
                }
                return Decoded::Key(end + 1, key, fin == b'M', false);
            }
            let mut p = [0u32; 4];
            let n = params(body, &mut p);
            let mods = if n >= 2 { p[1].max(1) - 1 } else { 0 };
            let shift = mods & 1 != 0;
            // kitty: modifiers field may be "mods:event" -> event in p[2]
            let event = if body.contains(&b':') && n >= 3 { p[2] } else { 1 };
            let down = event != 3;
            let key = match fin {
                b'A' => KEY_UP,
                b'B' => KEY_DOWN,
                b'C' => KEY_RIGHT,
                b'D' => KEY_LEFT,
                b'H' => KEY_HOME,
                b'F' => KEY_END,
                b'P' => KEY_F1,
                b'Q' => KEY_F2,
                b'R' => KEY_F3,
                b'S' => KEY_F4,
                b'~' => match p[0] {
                    1 | 7 => KEY_HOME,
                    2 => KEY_INSERT,
                    3 => KEY_DELETE,
                    4 | 8 => KEY_END,
                    5 => KEY_PGUP,
                    6 => KEY_PGDN,
                    11 => KEY_F1,
                    12 => KEY_F2,
                    13 => KEY_F3,
                    14 => KEY_F4,
                    15 => KEY_F5,
                    17 => KEY_F6,
                    18 => KEY_F7,
                    19 => KEY_F8,
                    20 => KEY_F9,
                    21 => KEY_F10,
                    23 => KEY_F11,
                    24 => KEY_F12,
                    _ => 0,
                },
                b'u' => {
                    // kitty CSI unicode ; mods[:event] u
                    let cp = p[0];
                    match cp {
                        13 => KEY_ENTER,
                        9 => KEY_TAB,
                        27 => KEY_ESCAPE,
                        127 | 8 => KEY_BACKSPACE,
                        57441 | 57447 => KEY_SHIFT,
                        57442 | 57448 => KEY_CTRL,
                        57443 | 57449 => KEY_ALT,
                        57362 => KEY_PAUSE,
                        32..=126 => {
                            let c = cp as u8;
                            if c.is_ascii_uppercase() { (c + 32) as u16 } else { c as u16 }
                        }
                        _ => 0,
                    }
                }
                _ => 0,
            };
            if fin == b'c' {
                return Decoded::Skip(end + 1);
            }
            // Report Ctrl held via kitty modifiers as a separate key the engine understands.
            Decoded::Key(end + 1, key, down, shift)
        }
        b'O' => {
            if rest.len() < 3 {
                return Decoded::Incomplete;
            }
            let key = match rest[2] {
                b'A' => KEY_UP,
                b'B' => KEY_DOWN,
                b'C' => KEY_RIGHT,
                b'D' => KEY_LEFT,
                b'H' => KEY_HOME,
                b'F' => KEY_END,
                b'P' => KEY_F1,
                b'Q' => KEY_F2,
                b'R' => KEY_F3,
                b'S' => KEY_F4,
                _ => 0,
            };
            Decoded::Key(3, key, true, false)
        }
        0x1b => Decoded::Key(1, KEY_ESCAPE, true, false),
        c => {
            // Alt+key: treat as the key itself.
            let k = if c.is_ascii_uppercase() { (c + 32) as u16 } else { c as u16 };
            Decoded::Key(2, k, true, false)
        }
    }
}

/// Emulates key-up events for terminals that only send key presses: a key
/// counts as held while presses (auto-repeat) keep arriving.
///
/// Such terminals only auto-repeat the *last* key pressed, so holding W and
/// then pressing Left silences W even though it is still down. To let keys
/// overlap, a key that was being held (or a movement key) when a different
/// key arrived is "carried": it stays down for as long as any newer key keeps
/// repeating, and is released together with the last of them.
pub struct HeldKeys {
    keys: [Held; 16],
    last: u16,
}

#[derive(Clone, Copy)]
struct Held {
    key: u16,
    /// Release time if no more repeats arrive (0 = slot free).
    until: u64,
    /// Seen auto-repeat, so it was really held down.
    repeating: bool,
    carried: bool,
}

const FREE: Held = Held { key: 0, until: 0, repeating: false, carried: false };

/// Keys worth keeping down through other keys even after a single press:
/// walking a little too far is harmless, losing the walk mid-turn is not.
fn is_move(k: u16) -> bool {
    matches!(k, KEY_UP | KEY_DOWN | KEY_LEFT | KEY_RIGHT) || matches!(k as u8, b'w' | b'a' | b's' | b'd' | b',' | b'.') && k < 128
}

/// Pressing W after S (or Left after Right...) means the other was let go.
fn same_axis(a: u16, b: u16) -> bool {
    // fold Up/Down onto W/S, then compare axes
    let axis = |k: u16| match k {
        KEY_UP | KEY_DOWN => 1,
        KEY_LEFT | KEY_RIGHT => 2,
        _ if k < 128 => match k as u8 {
            b'w' | b's' => 1,
            b'a' | b'd' | b',' | b'.' => 3,
            _ => 0,
        },
        _ => 0,
    };
    a != b && axis(a) != 0 && axis(a) == axis(b)
}

impl HeldKeys {
    pub const fn new() -> HeldKeys {
        HeldKeys { keys: [FREE; 16], last: 0 }
    }

    /// Returns true if this is a new press (engine should get key-down).
    /// Keys that the press implies were released are passed to `up`.
    pub fn press(&mut self, key: u16, now_ms: u64, up: &mut dyn FnMut(u16)) -> bool {
        if key != self.last {
            for k in self.keys.iter_mut().filter(|k| k.until != 0 && k.key != key) {
                if same_axis(k.key, key) {
                    // W then S: the player switched direction.
                    up(k.key);
                    *k = FREE;
                } else if k.repeating || is_move(k.key) {
                    k.carried = true;
                }
            }
            self.last = key;
        }
        if let Some(k) = self.keys.iter_mut().find(|k| k.until != 0 && k.key == key) {
            // Auto-repeat: extend the hold a little past the repeat interval.
            k.until = now_ms + 110;
            k.repeating = true;
            k.carried = false;
            return false;
        }
        if let Some(k) = self.keys.iter_mut().find(|k| k.until == 0) {
            // First press: hold long enough to bridge the terminal's repeat delay.
            *k = Held { key, until: now_ms + 520, repeating: false, carried: false };
        }
        true
    }

    /// Release keys whose repeat stream stopped; calls `up(key)` for each.
    pub fn expire(&mut self, now_ms: u64, up: &mut dyn FnMut(u16)) {
        let mut live = false;
        for k in self.keys.iter_mut().filter(|k| k.until != 0 && !k.carried) {
            if now_ms >= k.until {
                up(k.key);
                *k = FREE;
            } else {
                live = true;
            }
        }
        if !live {
            for k in self.keys.iter_mut().filter(|k| k.until != 0) {
                up(k.key);
                *k = FREE;
            }
            self.last = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(events: &[(u64, u16)], until: u64) -> [[bool; 3]; 64] {
        // returns, every 50 ms, whether w / left / s are down
        let mut held = HeldKeys::new();
        let mut down = [false; KEY_COUNT];
        let mut out = [[false; 3]; 64];
        let mut ev = events.iter().peekable();
        for t in 0..until {
            while let Some(&&(at, k)) = ev.peek() {
                if at != t {
                    break;
                }
                ev.next();
                if held.press(k, t, &mut |u| down[u as usize] = false) {
                    down[k as usize] = true;
                }
            }
            held.expire(t, &mut |u| down[u as usize] = false);
            if t % 50 == 0 && ((t / 50) as usize) < out.len() {
                out[(t / 50) as usize] = [down[b'w' as usize], down[KEY_LEFT as usize], down[b's' as usize]];
            }
        }
        out
    }

    fn repeat(k: u16, from: u64, to: u64, v: &mut alloc_free::Vec) {
        v.push((from, k));
        let mut t = from + 500;
        while t < to {
            v.push((t, k));
            t += 33;
        }
    }

    mod alloc_free {
        pub struct Vec {
            pub items: [(u64, u16); 256],
            pub len: usize,
        }
        impl Vec {
            pub fn new() -> Vec {
                Vec { items: [(0, 0); 256], len: 0 }
            }
            pub fn push(&mut self, x: (u64, u16)) {
                self.items[self.len] = x;
                self.len += 1;
            }
            pub fn sorted(&mut self) -> &[(u64, u16)] {
                self.items[..self.len].sort();
                &self.items[..self.len]
            }
        }
    }

    #[test]
    fn walking_survives_turning() {
        // hold W from 0 ms, hold Left from 1000 to 2000 ms (W repeat stops then)
        let mut v = alloc_free::Vec::new();
        repeat(b'w' as u16, 0, 1000, &mut v);
        repeat(KEY_LEFT, 1000, 2000, &mut v);
        let o = run(v.sorted(), 3000);
        assert!(o[25][0] && o[25][1], "walking and turning at 1.25 s");
        assert!(o[30][0] && o[30][1], "walking and turning at 1.5 s");
        assert!(!o[45][0] && !o[45][1], "everything released after the stream ends");
    }

    #[test]
    fn reversing_releases_the_old_direction() {
        let mut v = alloc_free::Vec::new();
        repeat(b'w' as u16, 0, 1000, &mut v);
        repeat(b's' as u16, 1000, 2000, &mut v);
        let o = run(v.sorted(), 2500);
        assert!(!o[30][0] && o[30][2]);
    }

    #[test]
    fn a_tap_is_released() {
        let o = run(&[(0, b'w' as u16)], 2000);
        assert!(o[5][0] && !o[15][0]);
    }
}
