//! Procedural wall textures and flats. Everything is generated from code at
//! build time: noise, bricks, panels, rivets, animated liquids. No external
//! art exists anywhere in the project.
//!
//! Colours are palette indices `ramp * 16 + shade` (see palette.rs).

use std::fmt::Write;

pub struct Img {
    pub w: usize,
    pub h: usize,
    pub px: Vec<u8>,
}

impl Img {
    fn new(w: usize, h: usize, c: u8) -> Img {
        Img { w, h, px: vec![c; w * h] }
    }
    fn set(&mut self, x: i32, y: i32, c: u8) {
        let x = x.rem_euclid(self.w as i32) as usize;
        let y = y.rem_euclid(self.h as i32) as usize;
        self.px[y * self.w + x] = c;
    }
    fn get(&self, x: i32, y: i32) -> u8 {
        let x = x.rem_euclid(self.w as i32) as usize;
        let y = y.rem_euclid(self.h as i32) as usize;
        self.px[y * self.w + x]
    }
    fn fill(&mut self, f: impl Fn(i32, i32) -> u8) {
        for y in 0..self.h as i32 {
            for x in 0..self.w as i32 {
                let c = f(x, y);
                self.set(x, y, c);
            }
        }
    }
    fn rect(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, c: u8) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                self.set(x, y, c);
            }
        }
    }
    /// Shift a rectangle's shades (keeps texture noise).
    fn tint(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, d: i32) {
        for y in y0..=y1 {
            for x in x0..=x1 {
                let c = self.get(x, y);
                self.set(x, y, shift(c, d));
            }
        }
    }
    /// Raised-panel bevel: light top/left edge, dark bottom/right edge.
    fn bevel(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, hi: i32, lo: i32) {
        for x in x0..=x1 {
            let c = self.get(x, y0);
            self.set(x, y0, shift(c, hi));
            let c = self.get(x, y1);
            self.set(x, y1, shift(c, lo));
        }
        for y in y0..=y1 {
            let c = self.get(x0, y);
            self.set(x0, y, shift(c, hi));
            let c = self.get(x1, y);
            self.set(x1, y, shift(c, lo));
        }
    }
    fn rivet(&mut self, x: i32, y: i32) {
        let c = self.get(x, y);
        self.set(x, y, shift(c, 4));
        self.set(x + 1, y, shift(c, 2));
        self.set(x, y + 1, shift(c, 1));
        self.set(x + 1, y + 1, shift(c, -3));
        self.set(x + 2, y + 1, shift(c, -2));
        self.set(x + 1, y + 2, shift(c, -2));
    }
    fn text(&mut self, s: &str, x: i32, y: i32, c: u8, shadow: Option<u8>) {
        let mut cx = x;
        for ch in s.chars() {
            let g = glyph(ch);
            for (gy, row) in g.iter().enumerate() {
                for gx in 0..5 {
                    if row & (0x10 >> gx) != 0 {
                        if let Some(sc) = shadow {
                            self.set(cx + gx + 1, y + gy as i32 + 1, sc);
                        }
                        self.set(cx + gx, y + gy as i32, c);
                    }
                }
            }
            cx += 6;
        }
    }
}

fn glyph(c: char) -> [u8; 7] {
    match c {
        'E' => [0x1f, 0x10, 0x10, 0x1e, 0x10, 0x10, 0x1f],
        'X' => [0x11, 0x11, 0x0a, 0x04, 0x0a, 0x11, 0x11],
        'I' => [0x0e, 0x04, 0x04, 0x04, 0x04, 0x04, 0x0e],
        'T' => [0x1f, 0x04, 0x04, 0x04, 0x04, 0x04, 0x04],
        _ => [0; 7],
    }
}

pub fn sh(ramp: u8, s: f64) -> u8 {
    ramp * 16 + s.round().clamp(1.0, 15.0) as u8
}

fn shift(c: u8, d: i32) -> u8 {
    let ramp = c & 0xf0;
    let s = ((c & 15) as i32 + d).clamp(1, 15) as u8;
    ramp | s
}

fn hash(x: i32, y: i32, s: u32) -> u32 {
    let mut h = (x as u32).wrapping_mul(0x8da6_b343) ^ (y as u32).wrapping_mul(0xd816_3841) ^ s.wrapping_mul(0xcb1a_b31f);
    h ^= h >> 13;
    h = h.wrapping_mul(0x5bd1_e995);
    h ^= h >> 15;
    h
}

fn r01(x: i32, y: i32, s: u32) -> f64 {
    hash(x, y, s) as f64 / u32::MAX as f64
}

/// Tileable value noise with the given lattice period.
fn vnoise(x: f64, y: f64, px: i32, py: i32, s: u32) -> f64 {
    let (xi, yi) = (x.floor(), y.floor());
    let (fx, fy) = (x - xi, y - yi);
    let (x0, y0) = (xi as i32, yi as i32);
    let g = |a: i32, b: i32| r01(a.rem_euclid(px), b.rem_euclid(py), s);
    let sx = fx * fx * (3.0 - 2.0 * fx);
    let sy = fy * fy * (3.0 - 2.0 * fy);
    let a = g(x0, y0) + (g(x0 + 1, y0) - g(x0, y0)) * sx;
    let b = g(x0, y0 + 1) + (g(x0 + 1, y0 + 1) - g(x0, y0 + 1)) * sx;
    a + (b - a) * sy
}

/// Fractal noise that tiles over a `w` x `h` texture. Returns roughly 0..1.
fn fbm(x: i32, y: i32, w: usize, h: usize, cells: i32, oct: u32, s: u32) -> f64 {
    let mut sum = 0.0;
    let mut amp = 0.5;
    let mut norm = 0.0;
    for o in 0..oct {
        let cx = cells << o;
        let cy = (cells * h as i32 / w as i32).max(1) << o;
        sum += vnoise(x as f64 * cx as f64 / w as f64, y as f64 * cy as f64 / h as f64, cx, cy, s + o * 101) * amp;
        norm += amp;
        amp *= 0.5;
    }
    sum / norm
}

fn noisy(w: usize, h: usize, ramp: u8, base: f64, amp: f64, cells: i32, s: u32) -> Img {
    let mut im = Img::new(w, h, 0);
    im.fill(|x, y| sh(ramp, base + (fbm(x, y, w, h, cells, 4, s) - 0.5) * amp + (r01(x, y, s + 7) - 0.5) * 0.9));
    im
}

// ------------------------------------------------------------------ wall textures

fn base_panels(ramp: u8, seed: u32) -> Img {
    let mut im = noisy(64, 128, ramp, 9.0, 3.0, 4, seed);
    for p in 0..2 {
        let y0 = p * 64;
        im.bevel(0, y0, 63, y0 + 63, 3, -4);
        im.tint(2, y0 + 27, 61, y0 + 36, -2);
        im.bevel(2, y0 + 27, 61, y0 + 36, -2, 2);
        for (x, y) in [(4, 4), (58, 4), (4, 57), (58, 57)] {
            im.rivet(x, y0 + y);
        }
    }
    im
}

fn steel_ribs() -> Img {
    let mut im = noisy(64, 128, 13, 8.0, 2.5, 4, 11);
    for x in (0..64).step_by(16) {
        im.tint(x + 6, 0, x + 9, 127, 2);
        im.bevel(x + 6, 0, x + 9, 127, 2, -2);
        im.tint(x, 0, x, 127, -4);
    }
    for y in [0, 63, 64, 127] {
        im.tint(0, y, 63, y, -3);
    }
    for x in (3..64).step_by(16) {
        im.rivet(x, 8);
        im.rivet(x, 72);
    }
    im
}

fn stone_blocks(ramp: u8, seed: u32, h: usize) -> Img {
    let mut im = noisy(64, h, ramp, 8.0, 3.5, 8, seed);
    let mut y = 0;
    let mut row = 0;
    while y < h as i32 {
        let bh = if hash(row, 0, seed) % 3 == 0 { 32 } else { 16 };
        let bh = bh.min(h as i32 - y);
        let mut x = (hash(row, 1, seed) % 24) as i32;
        let start = x;
        while x < start + 64 {
            let bw = 16 + (hash(x, row, seed) % 3) as i32 * 8;
            let d = (hash(x, row, seed + 5) % 5) as i32 - 2;
            im.tint(x, y, x + bw - 1, y + bh - 1, d);
            im.bevel(x, y, x + bw - 1, y + bh - 1, 2, -2);
            for yy in y..y + bh {
                im.set(x + bw - 1, yy, sh(ramp, 2.0));
            }
            x += bw;
        }
        for xx in 0..64 {
            im.set(xx, y + bh - 1, sh(ramp, 2.0));
        }
        y += bh;
        row += 1;
    }
    im
}

fn bricks(ramp: u8, seed: u32) -> Img {
    let mut im = noisy(64, 64, ramp, 7.5, 3.0, 8, seed);
    for row in 0..8 {
        let y = row * 8;
        let off = if row % 2 == 0 { 0 } else { 8 };
        for b in 0..4 {
            let x = off + b * 16;
            let d = (hash(b, row, seed) % 5) as i32 - 2;
            im.tint(x, y, x + 15, y + 7, d);
            im.bevel(x, y, x + 15, y + 7, 1, -2);
            for yy in y..y + 8 {
                im.set(x + 15, yy, sh(1, 3.0));
            }
        }
        for x in 0..64 {
            im.set(x, y + 7, sh(1, 3.0));
        }
    }
    im
}

fn tech_panel() -> Img {
    let mut im = noisy(64, 128, 13, 5.5, 2.0, 4, 21);
    im.bevel(0, 0, 63, 127, 2, -3);
    im.bevel(4, 4, 59, 50, -2, 2);
    // light strip
    im.rect(4, 56, 59, 71, sh(13, 3.0));
    for i in 0..6 {
        let x = 7 + i * 9;
        let lit = hash(i, 3, 21) % 4 != 0;
        let c = if lit { 10 } else { 9 };
        im.rect(x, 59, x + 5, 68, sh(c, if lit { 12.0 } else { 7.0 }));
        im.rect(x + 1, 60, x + 4, 67, sh(c, if lit { 15.0 } else { 9.0 }));
    }
    im.bevel(4, 56, 59, 71, -2, 2);
    // vents
    for y in (80..120).step_by(5) {
        im.rect(10, y, 53, y + 1, sh(13, 2.0));
        im.rect(10, y + 2, 53, y + 2, sh(13, 8.0));
    }
    for (x, y) in [(2, 2), (60, 2), (2, 124), (60, 124)] {
        im.rivet(x, y);
    }
    im
}

fn computer() -> Img {
    let mut im = noisy(64, 64, 13, 4.5, 1.5, 4, 31);
    im.bevel(0, 0, 63, 63, 2, -2);
    im.rect(5, 5, 58, 33, sh(10, 3.0));
    for y in (8..31).step_by(3) {
        let len = 10 + (hash(y, 0, 31) % 36) as i32;
        for x in 8..8 + len {
            if hash(x, y, 32) % 5 != 0 {
                im.set(x, y, sh(10, 11.0 + (hash(x, y, 33) % 4) as f64));
            }
        }
    }
    im.bevel(5, 5, 58, 33, -3, 3);
    let cols = [4u8, 7, 6, 9, 4, 7];
    for (i, c) in cols.iter().enumerate() {
        let x = 7 + i as i32 * 9;
        im.rect(x, 41, x + 5, 45, sh(*c, 11.0));
        im.rect(x + 1, 41, x + 4, 42, sh(*c, 14.0));
    }
    for x in (6..58).step_by(4) {
        im.rect(x, 51, x + 2, 58, sh(13, 2.0));
    }
    im
}

fn wood_planks(ramp: u8, seed: u32, w: usize, h: usize, vertical: bool) -> Img {
    let mut im = Img::new(w, h, 0);
    im.fill(|x, y| {
        let (u, v) = if vertical { (x, y) } else { (y, x) };
        let plank = u / 16;
        let n = fbm(x, y, w, h, 2, 3, seed + plank as u32);
        let grain = ((v as f64 * 0.21 + n * 9.0 + plank as f64 * 3.0).sin() * 0.5 + 0.5) * 2.0;
        let mut s = 7.0 + grain + (hash(plank, 0, seed) % 3) as f64 - 1.0;
        if u % 16 == 0 {
            s = 2.0;
        } else if u % 16 == 1 {
            s -= 2.0;
        }
        sh(ramp, s)
    });
    im
}

fn marble(ramp: u8, vein: u8, seed: u32, w: usize, h: usize) -> Img {
    let mut im = Img::new(w, h, 0);
    im.fill(|x, y| {
        let n = fbm(x, y, w, h, 3, 5, seed);
        let v = ((x as f64 / w as f64 * 4.0 + n * 7.0) * std::f64::consts::PI).sin();
        if v.abs() < 0.10 {
            sh(vein, 10.0 + n * 3.0)
        } else {
            sh(ramp, 4.5 + n * 3.0 + (1.0 - v.abs()) * 1.5)
        }
    });
    im
}

fn rock(ramp: u8, seed: u32, w: usize, h: usize, glow: Option<u8>) -> Img {
    let mut im = Img::new(w, h, 0);
    im.fill(|x, y| {
        let n = fbm(x, y, w, h, 4, 5, seed);
        let ridge = 1.0 - (fbm(x, y, w, h, 3, 4, seed + 50) * 2.0 - 1.0).abs();
        if ridge > 0.93 {
            match glow {
                Some(g) => sh(g, 9.0 + (ridge - 0.93) * 80.0),
                None => sh(ramp, 2.0),
            }
        } else {
            sh(ramp, 4.0 + n * 8.0 - ridge * 1.5)
        }
    });
    im
}

fn flesh(seed: u32, w: usize, h: usize) -> Img {
    let mut im = Img::new(w, h, 0);
    im.fill(|x, y| {
        let n = fbm(x, y, w, h, 4, 4, seed);
        let vein = 1.0 - (fbm(x, y, w, h, 2, 4, seed + 9) * 2.0 - 1.0).abs();
        let bump = fbm(x, y, w, h, 8, 2, seed + 3);
        if vein > 0.9 {
            sh(14, 3.0 + (1.0 - vein) * 20.0)
        } else {
            sh(12, 5.0 + n * 5.0 + (bump - 0.5) * 4.0)
        }
    });
    im
}

fn door(band: Option<u8>) -> Img {
    let mut im = noisy(64, 128, 13, 8.0, 2.0, 4, 41);
    for y in (0..112).step_by(16) {
        im.bevel(1, y, 62, y + 15, 2, -3);
    }
    for y in 0..128 {
        im.set(31, y, sh(13, 2.0));
        im.set(32, y, sh(13, 11.0));
    }
    for y in 112..128 {
        for x in 0..64 {
            let stripe = ((x + y) / 6) % 2 == 0;
            im.set(x, y, if stripe { sh(6, 12.0) } else { sh(0, 2.0) });
        }
    }
    im.bevel(0, 0, 63, 127, 2, -4);
    if let Some(c) = band {
        for y0 in [20, 84] {
            im.rect(2, y0, 61, y0 + 7, sh(c, 9.0));
            im.rect(2, y0 + 2, 61, y0 + 5, sh(c, 13.0));
        }
        // key emblem
        im.rect(24, 52, 39, 60, sh(0, 3.0));
        im.rect(26, 54, 29, 58, sh(c, 14.0));
        im.rect(30, 55, 37, 57, sh(c, 14.0));
        im.rect(35, 57, 36, 59, sh(c, 14.0));
    }
    im
}

fn door_track() -> Img {
    let mut im = noisy(8, 128, 13, 4.0, 1.0, 2, 51);
    for y in 0..128 {
        im.set(0, y, sh(13, 9.0));
        im.set(7, y, sh(13, 2.0));
        if y % 8 == 0 {
            for x in 1..7 {
                im.set(x, y, sh(13, 2.0));
            }
        }
    }
    im
}

fn switch(on: bool, exit: bool) -> Img {
    let mut im = noisy(64, 64, 1, 9.0, 3.0, 4, 3);
    im.bevel(0, 0, 63, 63, 3, -4);
    im.rect(20, 12, 43, 55, sh(13, 3.0));
    im.bevel(20, 12, 43, 55, -2, 3);
    // indicator light
    let lc = if on { 7 } else { 4 };
    im.rect(28, 16, 35, 21, sh(lc, 10.0));
    im.rect(29, 17, 34, 20, sh(lc, 15.0));
    // lever
    let (y0, y1) = if on { (25, 36) } else { (38, 49) };
    im.rect(29, y0, 34, y1, sh(0, 11.0));
    im.rect(29, y0, 30, y1, sh(0, 14.0));
    im.rect(27, if on { y0 } else { y1 - 3 }, 36, if on { y0 + 3 } else { y1 }, sh(4, 9.0));
    if exit {
        im.rect(8, 2, 55, 10, sh(0, 1.0));
        im.text("EXIT", 20, 3, sh(4, if on { 15.0 } else { 11.0 }), None);
    }
    im
}

fn support() -> Img {
    let mut im = Img::new(32, 128, 0);
    im.fill(|x, y| {
        let edge = x < 3 || x > 28;
        let web = (12..=19).contains(&x);
        let s = if edge { 10.0 } else if web { 7.0 } else { 5.0 } + (r01(x, y, 61) - 0.5) * 1.5;
        sh(13, s)
    });
    for y in (8..128).step_by(32) {
        im.rivet(5, y);
        im.rivet(24, y);
    }
    im
}

fn step_riser() -> Img {
    let mut im = noisy(64, 16, 13, 5.0, 2.0, 4, 71);
    for x in 0..64 {
        im.set(x, 0, sh(6, 12.0));
        im.set(x, 1, sh(6, 9.0));
        im.set(x, 15, sh(13, 2.0));
    }
    im
}

fn hazard(w: usize, h: usize) -> Img {
    let mut im = Img::new(w, h, 0);
    im.fill(|x, y| {
        let stripe = ((x + y) / 8) % 2 == 0;
        let n = r01(x, y, 81) - 0.5;
        if stripe { sh(6, 11.5 + n) } else { sh(0, 2.5 + n) }
    });
    im.bevel(0, 0, w as i32 - 1, h as i32 - 1, 2, -3);
    im
}

fn pipes() -> Img {
    let mut im = noisy(64, 128, 13, 3.0, 1.5, 4, 91);
    for (i, px) in [2, 18, 34, 50].iter().enumerate() {
        let ramp = if i % 2 == 0 { 15 } else { 13 };
        for x in 0..12 {
            let t = (x as f64 + 0.5) / 12.0;
            let lit = (t * std::f64::consts::PI).sin();
            for y in 0..128 {
                im.set(px + x, y, sh(ramp, 3.0 + lit * 10.0 - if x > 8 { 2.0 } else { 0.0 }));
            }
        }
        for y in [20 + i as i32 * 9, 84 + i as i32 * 5] {
            im.rect(*px - 1, y, *px + 12, y + 4, sh(13, 9.0));
            im.bevel(*px - 1, y, *px + 12, y + 4, 2, -3);
        }
    }
    im
}

fn crate_side() -> Img {
    let mut im = wood_planks(2, 101, 64, 64, false);
    im.tint(0, 0, 63, 5, 2);
    im.tint(0, 58, 63, 63, 2);
    im.tint(0, 0, 5, 63, 2);
    im.tint(58, 0, 63, 63, 2);
    im.bevel(0, 0, 63, 63, 2, -3);
    im.bevel(6, 6, 57, 57, -2, 2);
    for i in 6..58 {
        for d in -2..=2 {
            let c = im.get(i + d, i);
            im.set(i + d, i, shift(c, 2));
            let c = im.get(63 - i + d, i);
            im.set(63 - i + d, i, shift(c, 1));
        }
    }
    for (x, y) in [(2, 2), (60, 2), (2, 60), (60, 60)] {
        im.rivet(x, y);
    }
    im
}

fn light_panel() -> Img {
    let mut im = Img::new(32, 128, sh(13, 5.0));
    im.bevel(0, 0, 31, 127, 2, -3);
    for y in 4..124 {
        for x in 5..27 {
            let d = ((x as f64 - 15.5).abs() / 11.0).powi(2);
            im.set(x, y, sh(0, 15.0 - d * 3.0));
        }
    }
    im.bevel(4, 3, 27, 124, -3, 3);
    im
}

fn exit_sign() -> Img {
    let mut im = Img::new(64, 16, sh(0, 2.0));
    im.bevel(0, 0, 63, 15, 3, -1);
    im.text("EXIT", 21, 4, sh(4, 14.0), Some(sh(4, 5.0)));
    im
}

fn concrete() -> Img {
    let mut im = noisy(64, 64, 1, 8.5, 3.0, 4, 111);
    for i in 0..40 {
        let x = (hash(i, 0, 112) % 64) as i32;
        let y = (hash(i, 1, 112) % 64) as i32;
        im.set(x, y, sh(1, 4.0));
    }
    for y in 0..64 {
        let n = fbm(0, y, 64, 64, 2, 2, 113);
        for x in 0..64 {
            if fbm(x, y, 64, 64, 3, 3, 114) > 0.72 {
                let c = im.get(x, y);
                im.set(x, y, shift(c, -2 - (n * 2.0) as i32));
            }
        }
    }
    im
}

fn metal_plates() -> Img {
    let mut im = noisy(64, 64, 13, 7.5, 2.5, 4, 121);
    for (x0, y0) in [(0, 0), (32, 0), (0, 32), (32, 32)] {
        let d = if (x0 + y0) % 64 == 0 { 1 } else { -1 };
        im.tint(x0, y0, x0 + 31, y0 + 31, d);
        im.bevel(x0, y0, x0 + 31, y0 + 31, 2, -3);
        im.rivet(x0 + 3, y0 + 3);
        im.rivet(x0 + 27, y0 + 3);
        im.rivet(x0 + 3, y0 + 27);
        im.rivet(x0 + 27, y0 + 27);
    }
    im
}

fn grate_wall() -> Img {
    // Index 0 is transparent: used as a two-sided "mid" texture (fences, bars).
    let mut im = Img::new(64, 128, 0);
    for y in 0..128 {
        for x in 0..64 {
            let bar_v = x % 16 < 3;
            let bar_h = y % 32 < 3 || y > 124;
            if bar_v || bar_h {
                let s = if bar_v { 6.0 + (x % 16) as f64 * 2.0 } else { 7.0 + (y % 32) as f64 };
                im.set(x, y, sh(13, s));
            }
        }
    }
    im
}

// ------------------------------------------------------------------ flats (64x64)

fn tiles(ramp: u8, seed: u32, size: i32, alt: f64) -> Img {
    let mut im = noisy(64, 64, ramp, 8.0, 2.0, 4, seed);
    for ty in 0..64 / size {
        for tx in 0..64 / size {
            let (x0, y0) = (tx * size, ty * size);
            if (tx + ty) % 2 == 1 {
                im.tint(x0, y0, x0 + size - 1, y0 + size - 1, alt as i32);
            }
            im.bevel(x0, y0, x0 + size - 1, y0 + size - 1, 2, -3);
        }
    }
    im
}

fn diamond_plate() -> Img {
    let mut im = noisy(64, 64, 13, 7.0, 1.5, 4, 131);
    for y in 0..64i32 {
        for x in 0..64i32 {
            let (cx, cy) = (x % 8, y % 8);
            let flip = ((x / 8) + (y / 8)) % 2 == 0;
            let d = if flip { cx - cy } else { cx + cy - 7 };
            if d.abs() <= 1 && (1..7).contains(&cx) && (1..7).contains(&cy) {
                let c = im.get(x, y);
                im.set(x, y, shift(c, if d < 0 { 3 } else if d > 0 { -2 } else { 2 }));
            }
        }
    }
    im
}

fn grating() -> Img {
    let mut im = Img::new(64, 64, 0);
    im.fill(|x, y| {
        let bx = x % 8 < 2;
        let by = y % 8 < 2;
        if bx || by {
            sh(13, 7.0 + if bx && by { 2.0 } else { 0.0 } + (r01(x, y, 141) - 0.5))
        } else {
            sh(13, 1.5 + r01(x, y, 142))
        }
    });
    im
}

fn ceiling_tiles() -> Img {
    let mut im = noisy(64, 64, 1, 9.5, 1.5, 4, 151);
    for (x0, y0) in [(0, 0), (32, 0), (0, 32), (32, 32)] {
        im.bevel(x0, y0, x0 + 31, y0 + 31, -3, 2);
        for y in (y0 + 5..y0 + 28).step_by(4) {
            for x in (x0 + 5..x0 + 28).step_by(4) {
                let c = im.get(x, y);
                im.set(x, y, shift(c, -3));
            }
        }
    }
    im
}

fn beams() -> Img {
    let mut im = noisy(64, 64, 13, 4.0, 1.5, 4, 161);
    for (a, b) in [(0, 7), (32, 39)] {
        im.rect(a, 0, b, 63, sh(13, 7.0));
        im.bevel(a, 0, b, 63, 2, -2);
        im.rect(0, a, 63, b, sh(13, 6.0));
        im.bevel(0, a, 63, b, 2, -2);
    }
    im
}

fn ceiling_light() -> Img {
    let mut im = noisy(64, 64, 13, 6.0, 1.0, 4, 171);
    im.bevel(0, 0, 63, 63, 2, -2);
    for y in 12..52 {
        for x in 12..52 {
            let d = ((x as f64 - 31.5).abs().max((y as f64 - 31.5).abs()) / 20.0).powi(3);
            im.set(x, y, sh(0, 15.0 - d * 3.5));
        }
    }
    im.bevel(11, 11, 52, 52, -3, 3);
    im
}

fn liquid(ramp: u8, frame: u32, seed: u32, base: f64, amp: f64, spark: Option<u8>) -> Img {
    let mut im = Img::new(64, 64, 0);
    let t = frame as f64 / 3.0;
    im.fill(|x, y| {
        let ph = t * std::f64::consts::TAU;
        let a = fbm(x + (ph.cos() * 4.0) as i32, y + (ph.sin() * 4.0) as i32, 64, 64, 3, 4, seed);
        let b = fbm(x - (ph.sin() * 3.0) as i32, y + (ph.cos() * 3.0) as i32, 64, 64, 5, 2, seed + 7);
        let v = a * 0.7 + b * 0.3;
        match spark {
            Some(sp) if v > 0.72 => sh(sp, 11.0 + (v - 0.72) * 20.0),
            _ => sh(ramp, base + (v - 0.5) * amp),
        }
    });
    im
}

fn telepad() -> Img {
    let mut im = noisy(64, 64, 13, 3.5, 1.0, 4, 181);
    for y in 0..64 {
        for x in 0..64 {
            let d = ((x as f64 - 31.5).powi(2) + (y as f64 - 31.5).powi(2)).sqrt();
            if (24.0..29.0).contains(&d) {
                im.set(x, y, sh(10, 14.0 - (d - 26.5).abs() * 2.0));
            } else if d < 20.0 && ((x + y) % 8 < 2 || (x - y).rem_euclid(8) < 2) {
                im.set(x, y, sh(10, 7.0));
            }
        }
    }
    im
}

fn grass() -> Img {
    let mut im = noisy(64, 64, 7, 5.0, 3.0, 4, 191);
    for i in 0..220 {
        let x = (hash(i, 0, 192) % 64) as i32;
        let y = (hash(i, 1, 192) % 64) as i32;
        im.set(x, y, sh(7, 8.0 + (hash(i, 2, 192) % 3) as f64));
        im.set(x, y + 1, sh(7, 6.0));
    }
    im
}

fn marble_tiles() -> Img {
    let a = marble(7, 0, 201, 64, 64);
    let b = marble(0, 7, 202, 64, 64);
    let mut im = Img::new(64, 64, 0);
    im.fill(|x, y| if ((x / 32) + (y / 32)) % 2 == 0 { a.get(x, y) } else { shift(b.get(x, y), -2) });
    for (x0, y0) in [(0, 0), (32, 0), (0, 32), (32, 32)] {
        im.bevel(x0, y0, x0 + 31, y0 + 31, 1, -2);
    }
    im
}

// ------------------------------------------------------------------ catalogue

pub struct Set {
    pub walls: Vec<(String, Img)>,
    pub flats: Vec<(String, Img)>,
}

pub fn build() -> Set {
    let walls: Vec<(&str, Img)> = vec![
        ("BASE1", base_panels(1, 1)),
        ("BASE2", steel_ribs()),
        ("BASE3", base_panels(3, 2)),
        ("STONE1", stone_blocks(1, 7, 128)),
        ("STONE2", stone_blocks(13, 8, 128)),
        ("BRICK1", bricks(14, 9)),
        ("BRICK2", bricks(2, 10)),
        ("TECH1", tech_panel()),
        ("COMP1", computer()),
        ("WOOD1", wood_planks(2, 12, 64, 128, true)),
        ("MARBLE1", marble(7, 8, 13, 64, 128)),
        ("ROCK1", rock(2, 14, 64, 128, None)),
        ("ROCK2", rock(1, 15, 64, 128, None)),
        ("FLESH1", flesh(16, 64, 128)),
        ("LAVAWALL", rock(14, 17, 64, 64, Some(5))),
        ("DOOR1", door(None)),
        ("DOORRED", door(Some(4))),
        ("DOORBLU", door(Some(9))),
        ("DOORYEL", door(Some(6))),
        ("DOORTRAK", door_track()),
        ("SWITCH0", switch(false, false)),
        ("SWITCH1", switch(true, false)),
        ("EXITSW0", switch(false, true)),
        ("EXITSW1", switch(true, true)),
        ("SUPPORT1", support()),
        ("STEP1", step_riser()),
        ("LIFT1", hazard(64, 64)),
        ("PIPES1", pipes()),
        ("CRATE1", crate_side()),
        ("LIGHT1", light_panel()),
        ("EXITSIGN", exit_sign()),
        ("CEMENT1", concrete()),
        ("METAL1", metal_plates()),
        ("HELLROCK", rock(14, 18, 64, 128, None)),
        ("GRATE1", grate_wall()),
    ];
    let mut flats: Vec<(&str, Img)> = vec![
        ("FLOOR1", tiles(13, 211, 32, 1.0)),
        ("FLOOR2", diamond_plate()),
        ("FLOOR3", tiles(3, 212, 16, -1.0)),
        ("FLOOR4", grating()),
        ("CEIL1", ceiling_tiles()),
        ("CEIL2", beams()),
        ("CEILLITE", ceiling_light()),
    ];
    for f in 0..3 {
        flats.push((["NUKAGE1", "NUKAGE2", "NUKAGE3"][f as usize], liquid(7, f, 221, 8.0, 7.0, Some(7))));
    }
    for f in 0..3 {
        flats.push((["LAVA1", "LAVA2", "LAVA3"][f as usize], liquid(5, f, 231, 8.0, 10.0, Some(6))));
    }
    for f in 0..3 {
        flats.push((["WATER1", "WATER2", "WATER3"][f as usize], liquid(9, f, 241, 7.0, 6.0, None)));
    }
    flats.extend(vec![
        ("DIRT1", rock(2, 251, 64, 64, None)),
        ("ROCKFLR", rock(1, 252, 64, 64, None)),
        ("TELEPAD", telepad()),
        ("STEPTOP", tiles(13, 253, 64, 0.0)),
        ("CRATETOP", {
            let mut im = wood_planks(2, 254, 64, 64, true);
            im.bevel(0, 0, 63, 63, 3, -3);
            im.bevel(4, 4, 59, 59, -2, 2);
            im
        }),
        ("WOODFLR", wood_planks(2, 255, 64, 64, false)),
        ("HELLFLR", flesh(256, 64, 64)),
        ("MARBFLR", marble_tiles()),
        ("GRASS1", grass()),
        ("CEMENTF", concrete()),
        ("SKY", Img::new(64, 64, sh(9, 8.0))),
    ]);
    Set {
        walls: walls.into_iter().map(|(n, i)| (n.to_string(), i)).collect(),
        flats: flats.into_iter().map(|(n, i)| (n.to_string(), i)).collect(),
    }
}

/// Emits texture metadata as Rust and the pixels as binary blobs
/// (walls column-major for the column drawer, flats row-major).
pub fn emit(set: &Set, out: &mut String, walls_bin: &mut Vec<u8>, flats_bin: &mut Vec<u8>) {
    // Wall index 0 means "no texture".
    out.push_str("pub static WALLS: &[WallTex] = &[WallTex { w: 0, h: 0, off: 0 },");
    for (_, im) in &set.walls {
        assert!(im.w.is_power_of_two() && im.h.is_power_of_two(), "texture sizes must be powers of two");
        write!(out, "WallTex {{ w: {}, h: {}, off: {} }},", im.w, im.h, walls_bin.len()).unwrap();
        for x in 0..im.w {
            for y in 0..im.h {
                walls_bin.push(im.px[y * im.w + x]);
            }
        }
    }
    out.push_str("];\npub static WALL_NAMES: &[&str] = &[\"-\",");
    for (n, _) in &set.walls {
        write!(out, "{n:?},").unwrap();
    }
    out.push_str("];\npub static FLAT_NAMES: &[&str] = &[");
    for (n, im) in &set.flats {
        assert!(im.w == 64 && im.h == 64);
        write!(out, "{n:?},").unwrap();
        flats_bin.extend_from_slice(&im.px);
    }
    out.push_str("];\n");
    let widx = |n: &str| set.walls.iter().position(|(w, _)| w == n).unwrap() + 1;
    writeln!(
        out,
        "pub static SWITCH_PAIRS: &[(u8, u8)] = &[({}, {}), ({}, {})];",
        widx("SWITCH0"),
        widx("SWITCH1"),
        widx("EXITSW0"),
        widx("EXITSW1")
    )
    .unwrap();
    let fidx = |n: &str| set.flats.iter().position(|(w, _)| w == n).unwrap();
    writeln!(out, "pub const SKY_FLAT: u8 = {};", fidx("SKY")).unwrap();
    writeln!(
        out,
        "pub static FLAT_ANIMS: &[(u8, u8)] = &[({}, 3), ({}, 3), ({}, 3)];",
        fidx("NUKAGE1"),
        fidx("LAVA1"),
        fidx("WATER1")
    )
    .unwrap();
}
