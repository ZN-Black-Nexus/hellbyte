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
    pub const LEAVE: &'static [u8] = b"\x1b[<u\x1b[0m\x1b[?7h\x1b[?25h\x1b[?1049l";
    /// Ask the terminal for "report key releases" (kitty protocol, flags 1|2|8).
    pub const KITTY_ON: &'static [u8] = b"\x1b[>11u";
    /// Query whether the kitty protocol is supported (reply: CSI ? flags u).
    pub const KITTY_QUERY: &'static [u8] = b"\x1b[?u\x1b[c";

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
}

pub struct KeyEvent {
    pub key: u16,
    pub down: bool,
    pub shift: bool,
}

impl KeyDecoder {
    pub const fn new() -> KeyDecoder {
        KeyDecoder { buf: [0; 64], len: 0, kitty: false, quit: false }
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
        while i < self.len {
            let rest = &self.buf[i..self.len];
            match decode(rest, &mut self.kitty) {
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

fn decode(rest: &[u8], kitty: &mut bool) -> Decoded {
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
                // Reply to our kitty query: CSI ? flags u ; or device attributes.
                if fin == b'u' {
                    *kitty = true;
                }
                return Decoded::Skip(end + 1);
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
pub struct HeldKeys {
    keys: [(u16, u64); 16],
}

impl HeldKeys {
    pub const fn new() -> HeldKeys {
        HeldKeys { keys: [(0, 0); 16] }
    }

    /// Returns true if this is a new press (engine should get key-down).
    pub fn press(&mut self, key: u16, now_ms: u64) -> bool {
        for k in self.keys.iter_mut() {
            if k.0 == key && k.1 != 0 {
                // Auto-repeat: extend the hold a little past the repeat interval.
                k.1 = now_ms + 110;
                return false;
            }
        }
        if let Some(k) = self.keys.iter_mut().find(|k| k.1 == 0) {
            // First press: hold long enough to bridge the terminal's repeat delay.
            *k = (key, now_ms + 520);
        }
        true
    }

    /// Release keys whose repeat stream stopped; calls `up(key)` for each.
    pub fn expire(&mut self, now_ms: u64, up: &mut dyn FnMut(u16)) {
        for k in self.keys.iter_mut() {
            if k.1 != 0 && now_ms >= k.1 {
                up(k.0);
                *k = (0, 0);
            }
        }
    }
}
