//! 2D drawing on the 8-bit screen: fonts, digits, rectangles, lines.
//! Both fonts are original pixel designs made for Hellbyte.

use crate::data::COLORMAPS;

pub struct Surface<'a> {
    pub px: &'a mut [u8],
    pub w: i32,
    pub h: i32,
}

/// 5x7 glyphs, one byte per row (bit 4 = leftmost). Lowercase maps to uppercase.
fn glyph5x7(c: u8) -> [u8; 7] {
    match c.to_ascii_uppercase() {
        b'0' => [0x0e, 0x11, 0x13, 0x15, 0x19, 0x11, 0x0e],
        b'1' => [0x04, 0x0c, 0x04, 0x04, 0x04, 0x04, 0x0e],
        b'2' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x08, 0x1f],
        b'3' => [0x1e, 0x01, 0x01, 0x0e, 0x01, 0x01, 0x1e],
        b'4' => [0x02, 0x06, 0x0a, 0x12, 0x1f, 0x02, 0x02],
        b'5' => [0x1f, 0x10, 0x1e, 0x01, 0x01, 0x11, 0x0e],
        b'6' => [0x06, 0x08, 0x10, 0x1e, 0x11, 0x11, 0x0e],
        b'7' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x08, 0x08],
        b'8' => [0x0e, 0x11, 0x11, 0x0e, 0x11, 0x11, 0x0e],
        b'9' => [0x0e, 0x11, 0x11, 0x0f, 0x01, 0x02, 0x0c],
        b'A' => [0x0e, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        b'B' => [0x1e, 0x11, 0x11, 0x1e, 0x11, 0x11, 0x1e],
        b'C' => [0x0e, 0x11, 0x10, 0x10, 0x10, 0x11, 0x0e],
        b'D' => [0x1c, 0x12, 0x11, 0x11, 0x11, 0x12, 0x1c],
        b'E' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        b'F' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x10],
        b'G' => [0x0e, 0x11, 0x10, 0x17, 0x11, 0x11, 0x0f],
        b'H' => [0x11, 0x11, 0x11, 0x1f, 0x11, 0x11, 0x11],
        b'I' => [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        b'J' => [0x07, 0x02, 0x02, 0x02, 0x02, 0x12, 0x0c],
        b'K' => [0x11, 0x12, 0x14, 0x18, 0x14, 0x12, 0x11],
        b'L' => [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x1f],
        b'M' => [0x11, 0x1b, 0x15, 0x15, 0x11, 0x11, 0x11],
        b'N' => [0x11, 0x11, 0x19, 0x15, 0x13, 0x11, 0x11],
        b'O' => [0x0e, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        b'P' => [0x1e, 0x11, 0x11, 0x1e, 0x10, 0x10, 0x10],
        b'Q' => [0x0e, 0x11, 0x11, 0x11, 0x15, 0x12, 0x0d],
        b'R' => [0x1e, 0x11, 0x11, 0x1e, 0x14, 0x12, 0x11],
        b'S' => [0x0f, 0x10, 0x10, 0x0e, 0x01, 0x01, 0x1e],
        b'T' => [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        b'U' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x11, 0x0e],
        b'V' => [0x11, 0x11, 0x11, 0x11, 0x11, 0x0a, 0x04],
        b'W' => [0x11, 0x11, 0x11, 0x15, 0x15, 0x15, 0x0a],
        b'X' => [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
        b'Y' => [0x11, 0x11, 0x0a, 0x04, 0x04, 0x04, 0x04],
        b'Z' => [0x1f, 0x01, 0x02, 0x04, 0x08, 0x10, 0x1f],
        b'!' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x00, 0x04],
        b'"' => [0x0a, 0x0a, 0x00, 0x00, 0x00, 0x00, 0x00],
        b'\'' => [0x04, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00],
        b'(' => [0x02, 0x04, 0x08, 0x08, 0x08, 0x04, 0x02],
        b')' => [0x08, 0x04, 0x02, 0x02, 0x02, 0x04, 0x08],
        b',' => [0x00, 0x00, 0x00, 0x00, 0x06, 0x04, 0x08],
        b'-' => [0x00, 0x00, 0x00, 0x1f, 0x00, 0x00, 0x00],
        b'.' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x0c, 0x0c],
        b'/' => [0x01, 0x02, 0x02, 0x04, 0x08, 0x08, 0x10],
        b':' => [0x00, 0x0c, 0x0c, 0x00, 0x0c, 0x0c, 0x00],
        b';' => [0x00, 0x0c, 0x0c, 0x00, 0x0c, 0x04, 0x08],
        b'?' => [0x0e, 0x11, 0x01, 0x02, 0x04, 0x00, 0x04],
        b'%' => [0x18, 0x19, 0x02, 0x04, 0x08, 0x13, 0x03],
        b'+' => [0x00, 0x04, 0x04, 0x1f, 0x04, 0x04, 0x00],
        b'*' => [0x00, 0x15, 0x0e, 0x1f, 0x0e, 0x15, 0x00],
        b'_' => [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1f],
        b'<' => [0x02, 0x04, 0x08, 0x10, 0x08, 0x04, 0x02],
        b'>' => [0x08, 0x04, 0x02, 0x01, 0x02, 0x04, 0x08],
        b'=' => [0x00, 0x00, 0x1f, 0x00, 0x1f, 0x00, 0x00],
        b'[' => [0x0e, 0x08, 0x08, 0x08, 0x08, 0x08, 0x0e],
        b']' => [0x0e, 0x02, 0x02, 0x02, 0x02, 0x02, 0x0e],
        b'#' => [0x0a, 0x0a, 0x1f, 0x0a, 0x1f, 0x0a, 0x0a],
        b'|' => [0x04, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        _ => [0; 7],
    }
}

/// 3x5 digits (and '/', '%', '-') for tiny read-outs.
fn glyph3x5(c: u8) -> [u8; 5] {
    match c {
        b'0' => [7, 5, 5, 5, 7],
        b'1' => [2, 6, 2, 2, 7],
        b'2' => [7, 1, 7, 4, 7],
        b'3' => [7, 1, 7, 1, 7],
        b'4' => [5, 5, 7, 1, 1],
        b'5' => [7, 4, 7, 1, 7],
        b'6' => [7, 4, 7, 5, 7],
        b'7' => [7, 1, 1, 2, 2],
        b'8' => [7, 5, 7, 5, 7],
        b'9' => [7, 5, 7, 1, 7],
        b'/' => [1, 1, 2, 4, 4],
        b'%' => [5, 1, 2, 4, 5],
        b'-' => [0, 0, 7, 0, 0],
        b'A' | b'a' => [2, 5, 7, 5, 5],
        b'H' | b'h' => [5, 5, 7, 5, 5],
        b'R' | b'r' => [6, 5, 6, 5, 5],
        b'M' | b'm' => [5, 7, 7, 5, 5],
        b'O' | b'o' => [7, 5, 5, 5, 7],
        _ => [0; 5],
    }
}

pub const TEXT_W: i32 = 6;
pub const TEXT_H: i32 = 8;

impl Surface<'_> {
    #[inline]
    pub fn put(&mut self, x: i32, y: i32, c: u8) {
        if x >= 0 && y >= 0 && x < self.w && y < self.h {
            self.px[(y * self.w + x) as usize] = c;
        }
    }

    pub fn fill(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: u8) {
        for y in y0.max(0)..=y1.min(self.h - 1) {
            let row = (y * self.w) as usize;
            for x in x0.max(0)..=x1.min(self.w - 1) {
                self.px[row + x as usize] = c;
            }
        }
    }

    /// Darken (or otherwise remap) a rectangle through a colormap.
    pub fn remap(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, map: usize) {
        let cm = &COLORMAPS[map * 256..map * 256 + 256];
        for y in y0.max(0)..=y1.min(self.h - 1) {
            let row = (y * self.w) as usize;
            for x in x0.max(0)..=x1.min(self.w - 1) {
                let p = &mut self.px[row + x as usize];
                *p = cm[*p as usize];
            }
        }
    }

    pub fn bevel(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, hi: u8, lo: u8) {
        for x in x0..=x1 {
            self.put(x, y0, hi);
            self.put(x, y1, lo);
        }
        for y in y0..=y1 {
            self.put(x0, y, hi);
            self.put(x1, y, lo);
        }
    }

    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: u8) {
        let (dx, dy) = ((x1 - x0).abs(), -(y1 - y0).abs());
        let (sx, sy) = (if x0 < x1 { 1 } else { -1 }, if y0 < y1 { 1 } else { -1 });
        let (mut x, mut y, mut err) = (x0, y0, dx + dy);
        for _ in 0..4096 {
            self.put(x, y, c);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Draw text with the 5x7 font at integer `scale`; returns the end x.
    pub fn text(&mut self, x: i32, y: i32, s: &[u8], c: u8, shadow: Option<u8>, scale: i32) -> i32 {
        let mut cx = x;
        for &ch in s {
            let g = glyph5x7(ch);
            for (gy, row) in g.iter().enumerate() {
                for gx in 0..5 {
                    if row & (0x10 >> gx) == 0 {
                        continue;
                    }
                    for sy in 0..scale {
                        for sx in 0..scale {
                            let px = cx + gx * scale + sx;
                            let py = y + gy as i32 * scale + sy;
                            if let Some(sc) = shadow {
                                self.put(px + scale.max(1), py + scale.max(1), sc);
                            }
                            self.put(px, py, c);
                        }
                    }
                }
            }
            cx += TEXT_W * scale;
        }
        cx
    }

    /// Text with a vertical colour gradient (ramp shades from `hi` down to `lo`).
    pub fn text_grad(&mut self, x: i32, y: i32, s: &[u8], ramp: u8, hi: u8, lo: u8, scale: i32) {
        let mut cx = x;
        for &ch in s {
            let g = glyph5x7(ch);
            for (gy, row) in g.iter().enumerate() {
                let shade = hi as i32 - (hi as i32 - lo as i32) * gy as i32 / 6;
                let c = ramp * 16 + shade.clamp(1, 15) as u8;
                for gx in 0..5 {
                    if row & (0x10 >> gx) == 0 {
                        continue;
                    }
                    for sy in 0..scale {
                        for sx in 0..scale {
                            self.put(cx + gx * scale + sx + scale / 2 + 1, y + gy as i32 * scale + sy + scale / 2 + 1, 1);
                            self.put(cx + gx * scale + sx, y + gy as i32 * scale + sy, c);
                        }
                    }
                }
            }
            cx += TEXT_W * scale;
        }
    }

    pub fn text_centered(&mut self, y: i32, s: &[u8], c: u8, scale: i32) {
        let w = s.len() as i32 * TEXT_W * scale;
        self.text((self.w - w) / 2, y, s, c, Some(1), scale);
    }

    pub fn small(&mut self, x: i32, y: i32, s: &[u8], c: u8) -> i32 {
        let mut cx = x;
        for &ch in s {
            let g = glyph3x5(ch);
            for (gy, row) in g.iter().enumerate() {
                for gx in 0..3 {
                    if row & (4 >> gx) != 0 {
                        self.put(cx + gx, y + gy as i32, c);
                    }
                }
            }
            cx += 4;
        }
        cx
    }

    /// A seven-segment style digit, `w` x `h` pixels, lit segments in `on`.
    pub fn seg_digit(&mut self, x: i32, y: i32, d: u8, w: i32, h: i32, on: u8, off: u8) {
        const SEGS: [u8; 10] = [0x3f, 0x06, 0x5b, 0x4f, 0x66, 0x6d, 0x7d, 0x07, 0x7f, 0x6f];
        // 10 = minus sign, anything else = blank
        let bits = if d <= 9 { SEGS[d as usize] } else if d == 10 { 0x40 } else { 0 };
        let t = (w / 5).max(1);
        let mid = y + h / 2;
        let seg = |s: &mut Self, i: u8, x0: i32, y0: i32, x1: i32, y1: i32| {
            let c = if bits & (1 << i) != 0 { on } else { off };
            if c != 0 {
                s.fill(x0, y0, x1, y1, c);
            }
        };
        seg(self, 0, x + t, y, x + w - 1 - t, y + t - 1);
        seg(self, 1, x + w - t, y + t, x + w - 1, mid - 1);
        seg(self, 2, x + w - t, mid + 1, x + w - 1, y + h - 1 - t);
        seg(self, 3, x + t, y + h - t, x + w - 1 - t, y + h - 1);
        seg(self, 4, x, mid + 1, x + t - 1, y + h - 1 - t);
        seg(self, 5, x, y + t, x + t - 1, mid - 1);
        seg(self, 6, x + t, mid - t / 2, x + w - 1 - t, mid - t / 2 + t - 1);
    }

    /// Right-aligned number in seven-segment digits, ending at `right`.
    /// Unused leading positions show dim "off" segments like a real LCD.
    #[allow(clippy::too_many_arguments)]
    pub fn seg_number(&mut self, right: i32, y: i32, v: i32, digits: i32, w: i32, h: i32, on: u8, off: u8) {
        let mut n = v.unsigned_abs();
        let mut minus_pending = v < 0;
        let mut x = right - w;
        for i in 0..digits {
            let d = if i == 0 || n > 0 {
                (n % 10) as u8
            } else if minus_pending {
                minus_pending = false;
                10
            } else {
                11
            };
            self.seg_digit(x, y, d, w, h, on, off);
            n /= 10;
            x -= w + w / 4 + 1;
        }
    }
}

/// Format an integer into `buf`, returning the used slice.
pub fn itoa(v: i32, buf: &mut [u8; 12]) -> &[u8] {
    let mut n = v.unsigned_abs();
    let mut i = 12;
    loop {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    if v < 0 {
        i -= 1;
        buf[i] = b'-';
    }
    &buf[i..]
}

/// Tiny fixed-capacity string builder for UI text.
pub struct Line {
    pub buf: [u8; 64],
    pub len: usize,
}

impl Line {
    pub fn new() -> Line {
        Line { buf: [0; 64], len: 0 }
    }
    pub fn s(&mut self, s: &[u8]) -> &mut Self {
        for &c in s {
            if self.len < 64 {
                self.buf[self.len] = c;
                self.len += 1;
            }
        }
        self
    }
    pub fn n(&mut self, v: i32) -> &mut Self {
        let mut b = [0; 12];
        let t = itoa(v, &mut b);
        let mut tmp = [0u8; 12];
        let l = t.len();
        tmp[..l].copy_from_slice(t);
        self.s(&tmp[..l])
    }
    pub fn pad(&mut self, width: usize) -> &mut Self {
        while self.len < width.min(64) {
            self.buf[self.len] = b' ';
            self.len += 1;
        }
        self
    }
    pub fn as_bytes(&self) -> &[u8] {
        &self.buf[..self.len]
    }
}

impl Default for Line {
    fn default() -> Self {
        Self::new()
    }
}
