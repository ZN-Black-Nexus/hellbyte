//! Procedural sprites. Nothing here is stored as pixel art: every creature is
//! a tiny 3D model of spheres and capsules rendered (orthographically, with
//! a depth buffer and simple lighting) into a small canvas each time it is
//! drawn, so it can be seen from any of 16 angles and animated freely.
//! Items, effects and first-person weapons are drawn from 2D shapes.
//!
//! Canvas pixels are palette indices; 0 is transparent.

use crate::info::Spr;

pub const CANVAS_W: usize = 128;
pub const CANVAS_H: usize = 128;

pub struct Canvas {
    pub w: usize,
    pub h: usize,
    px: [u8; CANVAS_W * CANVAS_H],
    depth: [u8; CANVAS_W * CANVAS_H],
}

#[derive(Clone, Copy, Debug)]
pub struct Dims {
    pub w: i32,
    pub h: i32,
    /// Origin offset from the canvas' left edge.
    pub left: i32,
    /// Height of the canvas top above the origin.
    pub top: i32,
}

impl Canvas {
    fn begin(&mut self, w: usize, h: usize) {
        self.w = w.min(CANVAS_W);
        self.h = h.min(CANVAS_H);
        let n = self.w * self.h;
        self.px[..n].fill(0);
        self.depth[..n].fill(255);
    }

    pub fn column(&self, x: usize) -> &[u8] {
        &self.px[x * self.h..(x + 1) * self.h]
    }

    #[inline]
    fn plot(&mut self, x: i32, y: i32, c: u8) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            self.px[x as usize * self.h + y as usize] = c;
        }
    }

    #[inline]
    fn plot_z(&mut self, x: i32, y: i32, z: i32, c: u8) {
        if x >= 0 && y >= 0 && (x as usize) < self.w && (y as usize) < self.h {
            let i = x as usize * self.h + y as usize;
            let z = z.clamp(0, 254) as u8;
            if z < self.depth[i] {
                self.depth[i] = z;
                self.px[i] = c;
            }
        }
    }
}

// ------------------------------------------------------------------ small maths

const fn sqrt_table() -> [u8; 1025] {
    let mut t = [0u8; 1025];
    let mut i = 0;
    while i <= 1024 {
        let mut r = 0;
        while (r + 1) * (r + 1) <= i {
            r += 1;
        }
        t[i] = r as u8;
        i += 1;
    }
    t
}
static SQRT: [u8; 1025] = sqrt_table();

fn isqrt(v: i32) -> i32 {
    if v <= 0 {
        return 0;
    }
    if v <= 1024 {
        return SQRT[v as usize] as i32;
    }
    let mut x = v;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + v / x) / 2;
    }
    x
}

/// sin/cos of `a` (0..256 = full turn) scaled by 256.
fn sc(a: i32) -> (i32, i32) {
    // 64-entry quarter table of sin * 256
    const Q: [i16; 65] = [
        0, 6, 13, 19, 25, 31, 38, 44, 50, 56, 62, 68, 74, 80, 86, 92, 98, 104, 109, 115, 121, 126, 132, 137, 142, 147,
        152, 157, 162, 167, 172, 177, 181, 185, 190, 194, 198, 202, 206, 209, 213, 216, 220, 223, 226, 229, 231, 234,
        237, 239, 241, 243, 245, 247, 248, 250, 251, 252, 253, 254, 255, 255, 256, 256, 256,
    ];
    let s = |a: i32| -> i32 {
        let a = a & 255;
        match a >> 6 {
            0 => Q[a as usize] as i32,
            1 => Q[(128 - a) as usize] as i32,
            2 => -(Q[(a - 128) as usize] as i32),
            _ => -(Q[(256 - a) as usize] as i32),
        }
    };
    (s(a), s(a + 64))
}

fn hash(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x45d9_f3b);
    x ^= x >> 16;
    x
}

// ------------------------------------------------------------------ 3D primitives

#[derive(Clone, Copy)]
struct Col {
    ramp: u8,
    lo: u8,
    hi: u8,
}

const fn col(ramp: u8, lo: u8, hi: u8) -> Col {
    Col { ramp, lo, hi }
}

#[derive(Clone, Copy)]
struct P3 {
    x: i32,
    y: i32,
    z: i32,
}

const fn p3(x: i32, y: i32, z: i32) -> P3 {
    P3 { x, y, z }
}

#[derive(Clone, Copy)]
enum Prim {
    Ball(P3, i32, Col),
    Limb(P3, P3, i32, Col),
}

const MAX_PRIMS: usize = 64;

struct Model {
    prims: [Prim; MAX_PRIMS],
    n: usize,
    /// Pitch applied to every point about the feet (falling over), 0..64 = 0..90 degrees.
    fall: i32,
    /// Scatter amount for gib frames.
    burst: i32,
    lift: i32,
}

impl Model {
    fn new() -> Model {
        Model { prims: [Prim::Ball(p3(0, 0, 0), 0, col(0, 0, 0)); MAX_PRIMS], n: 0, fall: 0, burst: 0, lift: 0 }
    }
    fn ball(&mut self, c: P3, r: i32, k: Col) {
        if self.n < MAX_PRIMS {
            self.prims[self.n] = Prim::Ball(c, r, k);
            self.n += 1;
        }
    }
    fn limb(&mut self, a: P3, b: P3, r: i32, k: Col) {
        if self.n < MAX_PRIMS {
            self.prims[self.n] = Prim::Limb(a, b, r, k);
            self.n += 1;
        }
    }
    /// Left/right mirrored pair of limbs (y negated).
    fn pair(&mut self, a: P3, b: P3, r: i32, k: Col) {
        self.limb(a, b, r, k);
        self.limb(p3(a.x, -a.y, a.z), p3(b.x, -b.y, b.z), r, k);
    }

    fn xform(&self, p: P3, idx: usize) -> P3 {
        let mut p = p;
        if self.burst > 0 {
            let h = hash(idx as u32 * 7 + 3);
            let dx = (h & 31) as i32 - 16;
            let dy = ((h >> 5) & 31) as i32 - 16;
            p.x += dx * self.burst / 16;
            p.y += dy * self.burst / 16;
            p.z = (p.z * (8 - self.burst.min(8)) / 8).max(2) + ((h >> 10) & 7) as i32 * (8 - self.burst.min(8)) / 4;
        }
        if self.fall > 0 {
            let (s, c) = sc(self.fall);
            let x = (p.x * c - p.z * s) >> 8;
            let z = (p.x * s + p.z * c) >> 8;
            p.x = x;
            p.z = z;
        }
        p.z += self.lift;
        p
    }
}

/// Orthographic camera for a relative viewing angle `rot` (0..15, 8 = front).
struct Cam {
    dx: i32,
    dy: i32,
    rx: i32,
    ry: i32,
    ox: i32,
    oy: i32,
}

impl Cam {
    fn new(rot: u8, ox: i32, oy: i32) -> Cam {
        let (s, c) = sc(rot as i32 * 16);
        // view direction (camera -> model) and screen-right vector, *256
        Cam { dx: c, dy: s, rx: s, ry: -c, ox, oy }
    }
    fn proj(&self, p: P3) -> (i32, i32, i32) {
        let u = (p.x * self.rx + p.y * self.ry) >> 8;
        let d = (p.x * self.dx + p.y * self.dy) >> 8;
        (self.ox + u, self.oy - p.z, 128 + d)
    }
}

fn shade(k: Col, dx: i32, dy: i32, z: i32, r: i32) -> u8 {
    if k.lo == k.hi {
        return k.ramp * 16 + k.hi;
    }
    // light from upper-left, towards the viewer
    let dot = (-110 * dx - 150 * dy + 180 * z) / r.max(1);
    let i = 80 + (176 * dot.max(0)) / 256; // 80..256
    let s = k.lo as i32 + (k.hi as i32 - k.lo as i32) * i / 256;
    k.ramp * 16 + s.clamp(1, 15) as u8
}

fn render_model(cv: &mut Canvas, m: &Model, cam: &Cam) {
    for i in 0..m.n {
        match m.prims[i] {
            Prim::Ball(c, r, k) => {
                let (cu, cy, cd) = cam.proj(m.xform(c, i));
                for y in cy - r..=cy + r {
                    for x in cu - r..=cu + r {
                        let (dx, dy) = (x - cu, y - cy);
                        let d2 = dx * dx + dy * dy;
                        if d2 > r * r {
                            continue;
                        }
                        let z = isqrt(r * r - d2);
                        cv.plot_z(x, y, cd - z, shade(k, dx, dy, z, r));
                    }
                }
            }
            Prim::Limb(a, b, r, k) => {
                let (ua, va, da) = cam.proj(m.xform(a, i));
                let (ub, vb, db) = cam.proj(m.xform(b, i));
                let (ex, ey) = (ub - ua, vb - va);
                let len2 = ex * ex + ey * ey;
                for y in va.min(vb) - r..=va.max(vb) + r {
                    for x in ua.min(ub) - r..=ua.max(ub) + r {
                        let (px, py) = (x - ua, y - va);
                        let t = if len2 > 0 { ((px * ex + py * ey) * 256 / len2).clamp(0, 256) } else { 0 };
                        let (cx, cyy) = (ua + ex * t / 256, va + ey * t / 256);
                        let (dx, dy) = (x - cx, y - cyy);
                        let d2 = dx * dx + dy * dy;
                        if d2 > r * r {
                            continue;
                        }
                        let z = isqrt(r * r - d2);
                        let depth = da + (db - da) * t / 256 - z;
                        cv.plot_z(x, y, depth, shade(k, dx, dy, z, r));
                    }
                }
            }
        }
    }
}

// ------------------------------------------------------------------ creature designs

const BRIGHT_RED: Col = col(4, 15, 15);
const FLASH: Col = col(5, 15, 15);

/// Frame layout shared by all creatures:
/// 0-3 walk, 4 aim, 5 fire, 6 pain, 7-11 death (11 = corpse), 12-16 gibbed.
fn death_pose(m: &mut Model, frame: u8) -> bool {
    match frame {
        7..=11 => {
            m.fall = [12, 28, 44, 58, 64][(frame - 7) as usize];
            m.lift = 0;
            true
        }
        12..=16 => {
            m.burst = [3, 6, 9, 12, 14][(frame - 12) as usize];
            true
        }
        _ => false,
    }
}

struct Human {
    body: Col,
    dark: Col,
    skin: Col,
    head: Col,
    eye: Col,
    gun: Col,
    bulk: i32,
    gun_len: i32,
    gun_r: i32,
    barrels: i32,
    pads: Option<Col>,
}

fn humanoid(m: &mut Model, h: &Human, frame: u8) {
    let walk = if frame < 4 { [0, 1, 0, -1][frame as usize] } else { 0 };
    let dead = death_pose(m, frame);
    let pain = frame == 6;
    let stride = walk * 6;
    let b = h.bulk;
    // legs: hip -> knee -> foot
    for side in [-1, 1] {
        let sw = stride * side;
        let hip = p3(0, 5 * side, 27);
        let knee = p3(3 + sw / 2, 5 * side, 14);
        let foot = p3(sw, 5 * side, 3);
        m.limb(hip, knee, 4 + b / 3, h.body);
        m.limb(knee, foot, 4, h.dark);
        m.ball(p3(sw + 3, 5 * side, 2), 3, h.dark);
    }
    // torso, belt and pack
    let lean = if pain { -4 } else { 0 };
    m.limb(p3(0, 0, 28), p3(lean, 0, 43), 8 + b, h.body);
    m.limb(p3(0, -7, 29), p3(0, 7, 29), 3, h.dark);
    m.ball(p3(lean - 7, 0, 38), 6 + b / 2, h.dark);
    if let Some(pad) = h.pads {
        m.ball(p3(lean, 10 + b, 44), 4 + b / 2, pad);
        m.ball(p3(lean, -10 - b, 44), 4 + b / 2, pad);
    }
    // head with glowing visor
    let hz = 52 + b / 2;
    let hx = lean + if pain { -3 } else { 1 };
    m.ball(p3(hx, 0, hz), 6, h.head);
    m.ball(p3(hx + 1, 0, hz - 3), 4, h.skin);
    m.limb(p3(hx + 5, -3, hz), p3(hx + 5, 3, hz), 1, h.eye);
    // arms holding the weapon forward
    let aim = matches!(frame, 4 | 5);
    let gz = if aim { 40 } else { 36 };
    let hand = p3(12, -2, gz);
    m.limb(p3(lean, -10 - b, 43), p3(6, -11 - b, 35), 3, h.body);
    m.limb(p3(6, -11 - b, 35), hand, 3, h.skin);
    m.limb(p3(lean, 10 + b, 43), p3(8, 9 + b, 36), 3, h.body);
    m.limb(p3(8, 9 + b, 36), p3(16, 0, gz), 3, h.skin);
    let tip = p3(12 + h.gun_len, -2, gz + 1);
    if h.barrels > 1 {
        for k in 0..h.barrels {
            let o = (k - h.barrels / 2) * 2;
            m.limb(p3(10, -2 + o, gz + o / 2), p3(tip.x, -2 + o, gz + 1 + o / 2), 1, h.gun);
        }
        m.ball(p3(12, -2, gz), h.gun_r + 1, h.dark);
    } else {
        m.limb(p3(8, -2, gz - 1), tip, h.gun_r, h.gun);
    }
    if frame == 5 && !dead {
        m.ball(p3(tip.x + 4, -2, gz + 1), 5, FLASH);
        m.ball(p3(tip.x + 6, -2, gz + 1), 3, col(0, 15, 15));
    }
    if frame >= 12 {
        m.ball(p3(-6, 4, 6), 5, col(4, 3, 9));
        m.ball(p3(8, -6, 4), 4, col(4, 3, 9));
    }
}

const DRONE: Human = Human {
    body: col(8, 3, 11),
    dark: col(8, 1, 6),
    skin: col(13, 5, 11),
    head: col(13, 3, 9),
    eye: BRIGHT_RED,
    gun: col(13, 2, 7),
    bulk: 0,
    gun_len: 12,
    gun_r: 2,
    barrels: 1,
    pads: None,
};

const ENFORCER: Human = Human {
    body: col(13, 2, 9),
    dark: col(13, 1, 5),
    skin: col(3, 4, 10),
    head: col(13, 2, 7),
    eye: col(6, 15, 15),
    gun: col(2, 3, 9),
    bulk: 1,
    gun_len: 10,
    gun_r: 3,
    barrels: 1,
    pads: Some(col(4, 3, 10)),
};

const HEAVY: Human = Human {
    body: col(15, 3, 11),
    dark: col(15, 1, 6),
    skin: col(12, 4, 10),
    head: col(15, 2, 8),
    eye: col(10, 15, 15),
    gun: col(13, 3, 10),
    bulk: 3,
    gun_len: 16,
    gun_r: 3,
    barrels: 4,
    pads: Some(col(13, 3, 9)),
};

const PLAYER: Human = Human {
    body: col(7, 2, 9),
    dark: col(13, 1, 6),
    skin: col(3, 5, 11),
    head: col(7, 3, 10),
    eye: col(10, 14, 14),
    gun: col(13, 2, 8),
    bulk: 1,
    gun_len: 10,
    gun_r: 2,
    barrels: 1,
    pads: Some(col(13, 4, 10)),
};

/// Fiend: hunched, spined, tailed biped that hurls fire.
fn fiend(m: &mut Model, frame: u8) {
    let walk = if frame < 4 { [0, 1, 0, -1][frame as usize] } else { 0 };
    death_pose(m, frame);
    let skin = col(14, 2, 10);
    let belly = col(2, 3, 9);
    let bone = col(3, 7, 14);
    let s = walk * 5;
    for side in [-1, 1] {
        let sw = s * side;
        // digitigrade legs: knee bends backwards
        m.limb(p3(-2, 6 * side, 26), p3(4, 7 * side, 17), 4, skin);
        m.limb(p3(4, 7 * side, 17), p3(-2 + sw, 6 * side, 8), 3, skin);
        m.limb(p3(-2 + sw, 6 * side, 8), p3(2 + sw, 6 * side, 2), 2, skin);
        m.ball(p3(4 + sw, 6 * side, 2), 2, bone);
    }
    // hunched torso + belly
    m.limb(p3(-2, 0, 28), p3(6, 0, 42), 9, skin);
    m.ball(p3(5, 0, 32), 6, belly);
    // spines down the back
    for k in 0..5 {
        m.ball(p3(-6 - k, 0, 46 - k * 5), 3 - k / 2, bone);
    }
    // tail
    let sway = [0, 3, 0, -3][(frame & 3) as usize];
    m.limb(p3(-6, 0, 26), p3(-16, sway, 18), 4, skin);
    m.limb(p3(-16, sway, 18), p3(-24, sway * 2, 10), 2, skin);
    // head low and forward, glowing eyes
    m.ball(p3(12, 0, 46), 6, skin);
    m.limb(p3(14, 0, 43), p3(19, 0, 41), 3, skin);
    m.ball(p3(16, -3, 48), 1, col(5, 15, 15));
    m.ball(p3(16, 3, 48), 1, col(5, 15, 15));
    m.ball(p3(11, -4, 53), 2, bone);
    m.ball(p3(11, 4, 53), 2, bone);
    // long arms with claws; attack raises the right arm with a fireball
    let attack = matches!(frame, 4 | 5);
    m.limb(p3(6, -9, 42), p3(12, -12, 32), 3, skin);
    m.limb(p3(12, -12, 32), p3(18, -10, 26), 2, skin);
    m.limb(p3(18, -10, 26), p3(22, -9, 22), 1, bone);
    if attack {
        let hz = if frame == 4 { 58 } else { 50 };
        m.limb(p3(6, 9, 42), p3(4, 12, 54), 3, skin);
        m.limb(p3(4, 12, 54), p3(10, 10, hz), 2, skin);
        m.ball(p3(12, 9, hz + 2), if frame == 5 { 6 } else { 4 }, FLASH);
    } else {
        m.limb(p3(6, 9, 42), p3(12, 12, 32), 3, skin);
        m.limb(p3(12, 12, 32), p3(18, 10, 26), 2, skin);
        m.limb(p3(18, 10, 26), p3(22, 9, 22), 1, bone);
    }
    if frame >= 12 {
        m.ball(p3(0, 0, 4), 6, col(4, 2, 8));
    }
}

/// Ripper: low, heavy quadruped with an armoured snout and tusks.
fn ripper(m: &mut Model, frame: u8) {
    let walk = if frame < 4 { [0, 1, 0, -1][frame as usize] } else { 0 };
    death_pose(m, frame);
    let hide = col(11, 2, 9);
    let plate = col(13, 3, 10);
    let bone = col(3, 8, 15);
    let s = walk * 6;
    // four legs
    for (lx, side, ph) in [(10, -1, 1), (10, 1, -1), (-12, -1, -1), (-12, 1, 1)] {
        let sw = s * ph;
        m.limb(p3(lx, 10 * side, 26), p3(lx + sw / 2, 11 * side, 13), 5, hide);
        m.limb(p3(lx + sw / 2, 11 * side, 13), p3(lx + sw, 11 * side, 3), 4, hide);
        m.ball(p3(lx + sw + 2, 11 * side, 2), 3, plate);
    }
    // body: long barrel with armour plates along the spine
    m.limb(p3(-14, 0, 28), p3(10, 0, 32), 13, hide);
    for k in 0..4 {
        m.ball(p3(-10 + k * 7, 0, 42 + k % 2), 6, plate);
    }
    // head and jaws; the lower jaw drops when biting
    let bite = matches!(frame, 4 | 5);
    let jaw = if frame == 5 { 12 } else if bite { 6 } else { 2 };
    m.limb(p3(16, 0, 34), p3(30, 0, 34), 8, hide);
    m.limb(p3(20, 0, 40), p3(32, 0, 37), 5, plate);
    m.limb(p3(18, 0, 28), p3(31, 0, 28 - jaw), 6, hide);
    for side in [-1, 1] {
        m.limb(p3(28, 5 * side, 30), p3(35, 7 * side, 38), 2, bone);
        m.ball(p3(26, 6 * side, 40), 2, col(7, 15, 15));
    }
    if bite {
        for k in 0..3 {
            m.ball(p3(30 + k * 2, 0, 31 - jaw / 2), 1, bone);
        }
    }
    if frame >= 12 {
        m.ball(p3(0, 0, 6), 8, col(4, 2, 8));
    }
}

/// Gazer: a floating bell with a glowing core, a ring of eyes and tentacles.
fn gazer(m: &mut Model, frame: u8, anim: u16) {
    let dead = death_pose(m, frame);
    let bell = col(10, 2, 11);
    let rim = col(11, 3, 11);
    let t = (anim / 3) as i32;
    if !dead {
        m.lift = 0;
    }
    m.ball(p3(0, 0, 42), 17, bell);
    m.limb(p3(0, -14, 30), p3(0, 14, 30), 5, rim);
    m.limb(p3(-14, 0, 30), p3(14, 0, 30), 5, rim);
    // core glows hot while attacking or hurt
    let core = match frame {
        5 => col(6, 15, 15),
        4 => col(6, 12, 15),
        6 => col(4, 12, 15),
        _ => col(11, 10, 15),
    };
    m.ball(p3(8, 0, 38), 7, core);
    // ring of eyes
    for k in 0..8 {
        let (s, c) = sc(k * 32 + 8);
        m.ball(p3(c * 15 / 256, s * 15 / 256, 46), 2, if k % 2 == 0 { col(4, 14, 15) } else { col(6, 13, 15) });
    }
    // tentacles swaying
    for k in 0..6 {
        let (s, c) = sc(k * 43);
        let (bx, by) = (c * 10 / 256, s * 10 / 256);
        let (ws, _) = sc(t * 8 + k * 40);
        let sway = ws * 5 / 256;
        m.limb(p3(bx, by, 28), p3(bx + sway, by - sway / 2, 16), 2, rim);
        m.limb(p3(bx + sway, by - sway / 2, 16), p3(bx - sway / 2, by + sway, 4), 1, bell);
    }
}

/// Juggernaut: a hulking stone golem with a molten core.
fn juggernaut(m: &mut Model, frame: u8) {
    let walk = if frame < 4 { [0, 1, 0, -1][frame as usize] } else { 0 };
    death_pose(m, frame);
    let stone = col(1, 2, 11);
    let dark = col(1, 1, 6);
    let s = walk * 7;
    for side in [-1, 1] {
        let sw = s * side;
        m.limb(p3(0, 9 * side, 30), p3(2 + sw / 2, 10 * side, 15), 7, stone);
        m.limb(p3(2 + sw / 2, 10 * side, 15), p3(sw, 10 * side, 5), 6, stone);
        m.ball(p3(sw + 3, 10 * side, 4), 5, dark);
    }
    m.ball(p3(0, 0, 36), 12, stone);
    m.ball(p3(0, 0, 50), 15, stone);
    m.ball(p3(11, 0, 50), 5, col(5, 11, 15));
    m.ball(p3(13, 0, 50), 3, col(6, 15, 15));
    // cracks glowing on the chest
    m.limb(p3(10, -8, 56), p3(12, -3, 44), 1, col(5, 9, 13));
    m.limb(p3(10, 8, 42), p3(13, 4, 54), 1, col(5, 9, 13));
    for side in [-1, 1] {
        m.ball(p3(0, 16 * side, 58), 8, stone);
    }
    m.ball(p3(4, 0, 64), 6, dark);
    m.ball(p3(9, -2, 65), 1, col(5, 15, 15));
    m.ball(p3(9, 2, 65), 1, col(5, 15, 15));
    let throw = matches!(frame, 4 | 5);
    // left arm swings with the walk
    m.limb(p3(0, -18, 56), p3(4 - s, -20, 40), 6, stone);
    m.ball(p3(8 - s, -20, 30), 7, dark);
    if throw {
        let hz = if frame == 4 { 76 } else { 62 };
        m.limb(p3(0, 18, 56), p3(4, 20, 68), 6, stone);
        m.ball(p3(8, 18, hz), 7, dark);
        m.ball(p3(10, 16, hz + 5), if frame == 5 { 7 } else { 5 }, col(10, 13, 15));
    } else {
        m.limb(p3(0, 18, 56), p3(4 + s, 20, 40), 6, stone);
        m.ball(p3(8 + s, 20, 30), 7, dark);
    }
}

// ------------------------------------------------------------------ 2D drawing helpers

fn rect(cv: &mut Canvas, x0: i32, y0: i32, x1: i32, y1: i32, c: u8) {
    for y in y0..=y1 {
        for x in x0..=x1 {
            cv.plot(x, y, c);
        }
    }
}

/// Filled rect with a vertical light-to-dark gradient over the ramp.
fn grad_rect(cv: &mut Canvas, x0: i32, y0: i32, x1: i32, y1: i32, ramp: u8, hi: i32, lo: i32) {
    let h = (y1 - y0).max(1);
    for y in y0..=y1 {
        let s = hi + (lo - hi) * (y - y0) / h;
        for x in x0..=x1 {
            cv.plot(x, y, ramp * 16 + s.clamp(1, 15) as u8);
        }
    }
}

/// Horizontal cylinder look: bright in the middle row band.
fn tube_h(cv: &mut Canvas, x0: i32, y0: i32, x1: i32, y1: i32, ramp: u8, lo: i32, hi: i32) {
    let h = (y1 - y0).max(1);
    for y in y0..=y1 {
        let t = (y - y0) * 256 / h;
        let s = lo + (hi - lo) * (256 - (t - 90).abs() * 2).max(0) / 256;
        for x in x0..=x1 {
            cv.plot(x, y, ramp * 16 + s.clamp(1, 15) as u8);
        }
    }
}

fn tube_v(cv: &mut Canvas, x0: i32, y0: i32, x1: i32, y1: i32, ramp: u8, lo: i32, hi: i32) {
    let w = (x1 - x0).max(1);
    for x in x0..=x1 {
        let t = (x - x0) * 256 / w;
        let s = lo + (hi - lo) * (256 - (t - 90).abs() * 2).max(0) / 256;
        for y in y0..=y1 {
            cv.plot(x, y, ramp * 16 + s.clamp(1, 15) as u8);
        }
    }
}

fn disc(cv: &mut Canvas, cx: i32, cy: i32, r: i32, ramp: u8, lo: i32, hi: i32) {
    for y in -r..=r {
        for x in -r..=r {
            let d2 = x * x + y * y;
            if d2 > r * r {
                continue;
            }
            let z = isqrt(r * r - d2);
            let dot = (-110 * x - 150 * y + 180 * z) / r.max(1);
            let i = (80 + 176 * dot.max(0) / 256).min(256);
            cv.plot(cx + x, cy + y, ramp * 16 + (lo + (hi - lo) * i / 256).clamp(1, 15) as u8);
        }
    }
}

fn glow(cv: &mut Canvas, cx: i32, cy: i32, r: i32, ramp: u8) {
    for y in -r..=r {
        for x in -r..=r {
            let d2 = x * x + y * y;
            if d2 > r * r {
                continue;
            }
            let s = 15 - (isqrt(d2) * 8 / r.max(1));
            cv.plot(cx + x, cy + y, ramp * 16 + s.clamp(4, 15) as u8);
        }
    }
}

fn line(cv: &mut Canvas, x0: i32, y0: i32, x1: i32, y1: i32, c: u8) {
    let n = (x1 - x0).abs().max((y1 - y0).abs()).max(1);
    for i in 0..=n {
        cv.plot(x0 + (x1 - x0) * i / n, y0 + (y1 - y0) * i / n, c);
    }
}

fn tri(cv: &mut Canvas, a: (i32, i32), b: (i32, i32), c: (i32, i32), col: u8) {
    let (x0, x1) = (a.0.min(b.0).min(c.0), a.0.max(b.0).max(c.0));
    let (y0, y1) = (a.1.min(b.1).min(c.1), a.1.max(b.1).max(c.1));
    let edge = |p: (i32, i32), q: (i32, i32), x: i32, y: i32| (q.0 - p.0) * (y - p.1) - (q.1 - p.1) * (x - p.0);
    for y in y0..=y1 {
        for x in x0..=x1 {
            let e0 = edge(a, b, x, y);
            let e1 = edge(b, c, x, y);
            let e2 = edge(c, a, x, y);
            if (e0 >= 0 && e1 >= 0 && e2 >= 0) || (e0 <= 0 && e1 <= 0 && e2 <= 0) {
                cv.plot(x, y, col);
            }
        }
    }
}

const fn sh(ramp: u8, s: u8) -> u8 {
    ramp * 16 + s
}

// ------------------------------------------------------------------ item designs (2D, front facing)

fn keycard(cv: &mut Canvas, ramp: u8, bright: bool) {
    grad_rect(cv, 2, 2, 13, 17, 0, 12, 7);
    rect(cv, 2, 5, 13, 9, sh(ramp, if bright { 14 } else { 10 }));
    rect(cv, 4, 12, 11, 12, sh(0, 4));
    rect(cv, 4, 14, 9, 14, sh(0, 4));
    rect(cv, 3, 3, 4, 3, sh(0, 15));
}

fn draw_item(cv: &mut Canvas, spr: Spr, frame: u8, anim: u16) -> Dims {
    let d = dims(spr, frame);
    cv.begin(d.w as usize, d.h as usize);
    match spr {
        Spr::Clip => {
            grad_rect(cv, 3, 4, 8, 15, 13, 9, 3);
            for x in [4, 6] {
                rect(cv, x, 1, x + 1, 4, sh(6, 13));
                cv.plot(x, 0, sh(15, 12));
            }
        }
        Spr::AmmoBox => {
            grad_rect(cv, 1, 4, 26, 15, 8, 10, 3);
            rect(cv, 1, 4, 26, 5, sh(8, 12));
            rect(cv, 8, 8, 19, 11, sh(6, 12));
            for x in (3..24).step_by(3) {
                rect(cv, x, 1, x + 1, 3, sh(6, 13));
            }
        }
        Spr::Shells => {
            for (i, x) in [1, 5, 9, 13].iter().enumerate() {
                let y = (i as i32 % 2) * 2;
                tube_v(cv, *x, 2 + y, x + 2, 10 + y, 4, 5, 13);
                rect(cv, *x, 11 + y, x + 2, 13 + y, sh(6, 12));
            }
        }
        Spr::ShellBox => {
            grad_rect(cv, 1, 6, 30, 15, 4, 8, 2);
            rect(cv, 1, 6, 30, 7, sh(4, 11));
            for x in (3..28).step_by(4) {
                tube_v(cv, x, 1, x + 2, 6, 4, 6, 13);
            }
            rect(cv, 10, 10, 21, 12, sh(6, 13));
        }
        Spr::RocketAmmo => {
            tube_v(cv, 4, 6, 9, 26, 13, 4, 12);
            tri(cv, (4, 6), (9, 6), (6, 0), sh(4, 12));
            tri(cv, (2, 26), (4, 20), (4, 26), sh(13, 6));
            tri(cv, (11, 26), (9, 20), (9, 26), sh(13, 6));
        }
        Spr::RocketBox => {
            grad_rect(cv, 1, 10, 30, 22, 8, 9, 3);
            rect(cv, 1, 10, 30, 11, sh(8, 12));
            for x in (3..28).step_by(6) {
                tube_v(cv, x, 3, x + 3, 10, 13, 5, 12);
                tri(cv, (x, 3), (x + 3, 3), (x + 1, 0), sh(4, 12));
            }
        }
        Spr::Cell => {
            grad_rect(cv, 2, 3, 13, 12, 13, 8, 3);
            rect(cv, 4, 5, 11, 10, sh(10, 13));
            rect(cv, 6, 1, 9, 2, sh(13, 10));
        }
        Spr::CellPack => {
            grad_rect(cv, 1, 5, 26, 22, 13, 9, 2);
            rect(cv, 3, 8, 24, 19, sh(10, 11));
            for y in (9..19).step_by(3) {
                rect(cv, 4, y, 23, y, sh(10, 15));
            }
            rect(cv, 9, 2, 18, 4, sh(13, 11));
        }
        Spr::Backpack => {
            disc(cv, 11, 12, 10, 2, 3, 11);
            grad_rect(cv, 5, 8, 17, 18, 2, 10, 4);
            rect(cv, 7, 3, 8, 8, sh(2, 5));
            rect(cv, 14, 3, 15, 8, sh(2, 5));
            rect(cv, 8, 12, 14, 13, sh(6, 11));
        }
        Spr::Stim => {
            tube_v(cv, 3, 3, 8, 16, 0, 8, 15);
            rect(cv, 4, 7, 7, 15, sh(4, 11));
            rect(cv, 3, 1, 8, 2, sh(13, 9));
        }
        Spr::Medkit => {
            grad_rect(cv, 1, 5, 26, 19, 0, 14, 8);
            rect(cv, 1, 5, 26, 6, sh(0, 15));
            rect(cv, 10, 2, 17, 4, sh(13, 7));
            rect(cv, 12, 8, 15, 17, sh(7, 12));
            rect(cv, 9, 11, 18, 14, sh(7, 12));
        }
        Spr::HealthBonus => {
            let r = 4 + (frame as i32 & 1);
            glow(cv, 7, 7, r, 4);
            cv.plot(6, 5, sh(0, 15));
        }
        Spr::ArmorBonus => {
            let s = [11u8, 13, 15][frame as usize % 3];
            tri(cv, (2, 3), (12, 3), (7, 14), sh(13, 8));
            tri(cv, (4, 4), (10, 4), (7, 11), sh(10, s));
            rect(cv, 2, 2, 12, 3, sh(13, 11));
        }
        Spr::ArmorGreen | Spr::ArmorBlue => {
            let (r, lit) = if spr == Spr::ArmorGreen { (8, 7) } else { (9, 10) };
            disc(cv, 8, 9, 7, r, 2, 11);
            disc(cv, 23, 9, 7, r, 2, 11);
            grad_rect(cv, 5, 9, 26, 24, r, 11, 3);
            rect(cv, 12, 2, 19, 7, 0);
            rect(cv, 13, 12, 18, 14, sh(lit, if frame == 1 { 15 } else { 11 }));
            rect(cv, 15, 15, 16, 22, sh(r, 12));
        }
        Spr::VitalOrb => {
            let r = 9 + [0, 1, 1, 0][frame as usize & 3];
            disc(cv, 11, 11, r, 4, 3, 15);
            let (s, c) = sc(anim as i32 * 4);
            glow(cv, 11 + c * 4 / 256, 11 + s * 4 / 256, 3, 12);
            cv.plot(8, 7, sh(0, 15));
        }
        Spr::KeyRed => keycard(cv, 4, frame == 1),
        Spr::KeyBlue => keycard(cv, 9, frame == 1),
        Spr::KeyYellow => keycard(cv, 6, frame == 1),
        Spr::PShotgun => {
            tube_h(cv, 0, 3, 40, 5, 13, 3, 11);
            grad_rect(cv, 30, 5, 50, 8, 2, 10, 4);
            tri(cv, (44, 5), (60, 6), (48, 12), sh(2, 6));
            rect(cv, 20, 6, 28, 7, sh(2, 9));
        }
        Spr::PChaingun => {
            for y in [2, 4, 6] {
                tube_h(cv, 0, y, 34, y + 1, 13, 3, 12);
            }
            grad_rect(cv, 28, 1, 50, 10, 13, 10, 3);
            rect(cv, 38, 10, 42, 15, sh(13, 5));
        }
        Spr::PLauncher => {
            tube_h(cv, 0, 1, 50, 9, 8, 3, 11);
            rect(cv, 0, 2, 2, 8, sh(0, 2));
            rect(cv, 30, 9, 34, 15, sh(13, 4));
            rect(cv, 16, 0, 26, 1, sh(13, 9));
        }
        Spr::PPlasma => {
            tube_h(cv, 0, 2, 44, 10, 13, 3, 12);
            for x in (6..36).step_by(6) {
                rect(cv, x, 4, x + 2, 8, sh(10, 14));
            }
            rect(cv, 34, 10, 38, 15, sh(13, 5));
        }
        Spr::PArc => {
            tube_h(cv, 0, 2, 52, 16, 11, 2, 11);
            glow(cv, 8, 9, 5, 10);
            for x in (16..48).step_by(8) {
                rect(cv, x, 3, x + 3, 15, sh(13, 10));
            }
        }
        Spr::PDrill => {
            tri(cv, (0, 6), (16, 2), (16, 10), sh(13, 12));
            for x in (2..16).step_by(4) {
                line(cv, x, 3 + x / 5, x + 2, 9 - x / 5, sh(13, 6));
            }
            grad_rect(cv, 16, 0, 38, 12, 6, 12, 5);
            rect(cv, 28, 12, 32, 18, sh(13, 5));
        }
        Spr::Aegis => {
            let s = if frame == 0 { 11 } else { 15 };
            for a in 0..64 {
                let (sn, cs) = sc(a * 4);
                cv.plot(11 + cs * 9 / 256, 11 + sn * 9 / 256, sh(6, s));
                cv.plot(11 + cs * 8 / 256, 11 + sn * 8 / 256, sh(6, s - 3));
            }
            glow(cv, 11, 11, 5, 6);
        }
        Spr::Hazmat => {
            grad_rect(cv, 4, 6, 18, 30, 6, 13, 6);
            disc(cv, 11, 5, 5, 6, 5, 13);
            rect(cv, 8, 3, 14, 6, sh(10, 12));
            rect(cv, 1, 8, 4, 22, sh(6, 9));
            rect(cv, 18, 8, 21, 22, sh(6, 9));
        }
        Spr::SurveyMap => {
            grad_rect(cv, 1, 1, 20, 15, 13, 7, 3);
            rect(cv, 3, 3, 18, 13, sh(7, 3));
            let s = if frame == 0 { 11 } else { 14 };
            line(cv, 4, 11, 9, 5, sh(7, s));
            line(cv, 9, 5, 14, 9, sh(7, s));
            line(cv, 14, 9, 17, 4, sh(7, s));
            rect(cv, 10, 10, 11, 11, sh(4, 14));
        }
        Spr::Adrenaline => {
            tube_h(cv, 2, 4, 16, 8, 0, 8, 15);
            rect(cv, 5, 5, 13, 7, sh(4, 12));
            line(cv, 16, 6, 22, 6, sh(13, 12));
            rect(cv, 0, 3, 2, 9, sh(13, 8));
        }
        Spr::Barrel => {
            if frame < 2 {
                tube_v(cv, 1, 2, 20, 41, 13, 3, 12);
                rect(cv, 1, 16, 20, 21, sh(6, if frame == 0 { 11 } else { 13 }));
                for y in [2, 41] {
                    rect(cv, 1, y, 20, y, sh(13, 5));
                }
                rect(cv, 5, 0, 16, 2, sh(7, 8 + frame));
            } else {
                boom(cv, frame - 2, 32, 30);
            }
        }
        Spr::Lamp => {
            rect(cv, 10, 20, 12, 47, sh(13, 7));
            rect(cv, 6, 45, 16, 47, sh(13, 5));
            glow(cv, 11, 12, 9, 6);
            disc(cv, 11, 12, 5, 0, 12, 15);
        }
        Spr::Pillar => {
            tube_v(cv, 3, 6, 28, 51, 13, 2, 11);
            rect(cv, 1, 0, 30, 6, sh(13, 9));
            rect(cv, 1, 46, 30, 51, sh(13, 6));
            for y in (14..44).step_by(10) {
                rect(cv, 13, y, 18, y + 3, sh(10, 14));
            }
        }
        Spr::Torch => {
            rect(cv, 7, 30, 12, 63, sh(2, 5));
            rect(cv, 4, 28, 15, 32, sh(13, 6));
            let h = [0, 3, 1, 4][frame as usize & 3];
            tri(cv, (3, 30), (16, 30), (9 + h / 2, 6 + h), sh(5, 12));
            tri(cv, (6, 30), (13, 30), (10 - h / 3, 14 + h), sh(6, 15));
        }
        _ => {}
    }
    d
}

/// Explosion puff: `f` 0..4.
fn boom(cv: &mut Canvas, f: u8, w: i32, h: i32) {
    let (cx, cy) = (w / 2, h / 2);
    let r = 5 + f as i32 * 3;
    glow(cv, cx, cy, r.min(w / 2 - 1), 5);
    if f < 3 {
        glow(cv, cx, cy, (r / 2).max(2), 6);
    }
    for k in 0..6 {
        let (s, c) = sc(k * 42 + f as i32 * 9);
        let d = r + 2;
        glow(cv, cx + c * d / 256, cy + s * d / 256, 2 + (f as i32 % 2), if f > 2 { 14 } else { 5 });
    }
}

/// A rocket in flight seen from view `rot` (0 = from behind, 8 = nose first):
/// the body lies along the flight line with the nose ahead, so it never
/// travels sideways; the exhaust shows behind it, or glows round the body
/// when it flies straight away from the viewer.
fn rocket(cv: &mut Canvas, rot: u8, flick: i32, cx: i32, cy: i32) {
    let cam = Cam::new(rot, cx, cy);
    let (tail, _, tail_depth) = cam.proj(p3(-7, 0, 0));
    let (base, _, _) = cam.proj(p3(5, 0, 0));
    let (tip, _, tip_depth) = cam.proj(p3(9, 0, 0));
    let away = tip_depth > tail_depth; // the nose is the far end
    if !away {
        glow(cv, tail, cy, 3 + flick, 5); // exhaust behind the body
    }
    if (base - tail).abs() <= 2 {
        disc(cv, (base + tail) / 2, cy, 2, 13, 4, 12); // end on: round
    } else {
        tube_h(cv, tail.min(base), cy - 2, tail.max(base), cy + 2, 13, 4, 12);
    }
    if (tip - base).abs() >= 2 {
        tri(cv, (base, cy - 2), (base, cy + 2), (tip, cy), sh(4, 12));
    } else if !away {
        disc(cv, tip, cy, 1, 4, 8, 13); // nose cone, head on
    }
    if away {
        glow(cv, tail, cy, 3 + flick, 5); // exhaust in front of the body
    }
}

fn draw_effect(cv: &mut Canvas, spr: Spr, frame: u8, rot: u8, anim: u16) -> Dims {
    let d = dims(spr, frame);
    cv.begin(d.w as usize, d.h as usize);
    let (w, h) = (d.w, d.h);
    let (cx, cy) = (w / 2, h / 2);
    let flick = (anim as i32 / 2) & 1;
    match spr {
        Spr::FiendBall | Spr::GazerBall | Spr::JuggBall => {
            let ramp = match spr {
                Spr::FiendBall => 5,
                Spr::GazerBall => 11,
                _ => 7,
            };
            if frame < 2 {
                glow(cv, cx, cy, 6 + (frame as i32 ^ flick), ramp);
                glow(cv, cx, cy, 3, if ramp == 5 { 6 } else { 0 });
            } else {
                let f = frame as i32 - 2;
                glow(cv, cx, cy, 5 + f * 3, ramp);
                for k in 0..5 {
                    let (s, c) = sc(k * 51 + f * 20);
                    glow(cv, cx + c * (6 + f * 3) / 256, cy + s * (6 + f * 3) / 256, 2, ramp);
                }
            }
        }
        Spr::Rocket => {
            if frame == 0 {
                rocket(cv, rot, flick, cx, cy);
            } else {
                boom(cv, frame - 1, w, h);
            }
        }
        Spr::Plasma => {
            if frame < 2 {
                glow(cv, cx, cy, 6 + frame as i32, 10);
                glow(cv, cx, cy, 3, 0);
            } else {
                let f = frame as i32 - 2;
                for k in 0..8 {
                    let (s, c) = sc(k * 32 + f * 10);
                    glow(cv, cx + c * (4 + f * 3) / 256, cy + s * (4 + f * 3) / 256, 3 - f / 2, 10);
                }
            }
        }
        Spr::ArcBall => {
            if frame < 2 {
                glow(cv, cx, cy, 11 + frame as i32, 11);
                glow(cv, cx, cy, 6, 0);
                for k in 0..6 {
                    let (s, c) = sc(k * 43 + anim as i32 * 13);
                    line(cv, cx, cy, cx + c * 15 / 256, cy + s * 15 / 256, sh(10, 15));
                }
            } else {
                let f = frame as i32 - 2;
                glow(cv, cx, cy, (8 + f * 5).min(cx - 1), 10);
                for k in 0..10 {
                    let (s, c) = sc(k * 26 + f * 7);
                    let d = 10 + f * 5;
                    line(cv, cx, cy, cx + c * d / 256, cy + s * d / 256, sh(11, 15));
                }
            }
        }
        Spr::ArcTrace => {
            glow(cv, cx, cy, 4 - frame as i32, 10);
            line(cv, cx - 4, cy, cx + 4, cy, sh(0, 15));
        }
        Spr::Puff => {
            let r = 2 + frame as i32;
            glow(cv, cx, cy - frame as i32, r, if frame == 0 { 6 } else { 0 });
            if frame == 0 {
                cv.plot(cx, cy, sh(0, 15));
            }
        }
        Spr::Blood => {
            let r = 1 + frame as i32;
            glow(cv, cx, cy, r, 4);
            cv.plot(cx - 2, cy + 3, sh(4, 8));
            cv.plot(cx + 2, cy + 2, sh(4, 6));
        }
        Spr::Fog => {
            let f = frame as i32;
            for k in 0..10 {
                let hsh = hash(k as u32 * 13 + 7);
                let x = (hsh % w as u32) as i32;
                let y = h - 1 - ((hsh >> 8) % 16) as i32 - f * 6;
                glow(cv, x, y, 2 + (k % 2), 10);
            }
            if f < 3 {
                glow(cv, cx, h - 12 - f * 4, 6 - f, 7);
            }
        }
        _ => {}
    }
    d
}

// ------------------------------------------------------------------ first-person weapons

fn hand(cv: &mut Canvas, x: i32, y: i32) {
    disc(cv, x, y, 9, 3, 4, 12);
    rect(cv, x - 8, y + 4, x + 8, y + 16, sh(8, 5));
    grad_rect(cv, x - 8, y + 4, x + 8, y + 30, 8, 8, 3);
}

fn draw_weapon(cv: &mut Canvas, spr: Spr, frame: u8, anim: u16) -> Dims {
    let d = dims(spr, frame);
    cv.begin(d.w as usize, d.h as usize);
    let (w, h) = (d.w, d.h);
    let flick = anim as i32 & 1;
    match spr {
        Spr::WFist => {
            // gauntleted fist; frames 1-3 punch forward
            let (x, y) = match frame {
                1 => (50, 34),
                2 => (60, 14),
                3 => (64, 6),
                _ => (44, 44),
            };
            grad_rect(cv, x - 16, y + 12, x + 16, h - 1, 13, 10, 3);
            disc(cv, x, y + 8, 17, 13, 3, 12);
            for k in 0..4 {
                disc(cv, x - 11 + k * 7, y - 2, 5, 13, 4, 13);
            }
            rect(cv, x - 13, y + 18, x + 13, y + 20, sh(4, 9));
        }
        Spr::WDrill => {
            let jolt = if frame >= 2 { (anim as i32 & 1) * 3 } else { flick };
            let bx = w / 2 + jolt;
            grad_rect(cv, bx - 22, 40, bx + 22, h - 1, 6, 12, 4);
            rect(cv, bx - 22, 40, bx + 22, 44, sh(0, 2));
            tri(cv, (bx - 12, 42), (bx + 12, 42), (bx, 2 + jolt), sh(13, 11));
            let ph = (anim as i32 * 3) & 7;
            for y in (6..40).step_by(8) {
                let yy = y + ph;
                let half = (yy - 2) * 12 / 40;
                line(cv, bx - half, yy, bx + half, yy + 4, sh(13, 5));
            }
            hand(cv, bx + 30, h - 34);
        }
        Spr::WPistol => {
            let recoil = if frame == 1 || frame == 3 { 6 } else if frame == 2 { 3 } else { 0 };
            let (x, y) = (w / 2, 34 + recoil);
            if frame == 3 {
                glow(cv, x, y - 20, 13, 5);
                glow(cv, x, y - 20, 7, 6);
            }
            grad_rect(cv, x - 8, y - 12, x + 8, y + 14, 13, 11, 4);
            rect(cv, x - 3, y - 14, x + 3, y - 11, sh(0, 2));
            rect(cv, x - 8, y - 12, x - 7, y + 14, sh(13, 13));
            hand(cv, x, y + 22);
        }
        Spr::WShotgun => {
            let pump = match frame {
                1 => 8,
                2 => 16,
                3 => 22,
                _ => 0,
            };
            let (x, y) = (w / 2, 22);
            if frame >= 4 {
                glow(cv, x, y - 12, 18 - (frame as i32 - 4) * 5, 5);
                glow(cv, x, y - 12, 9, 6);
            }
            tube_v(cv, x - 10, y - 6, x - 1, y + 40, 13, 3, 12);
            tube_v(cv, x + 1, y - 6, x + 10, y + 40, 13, 3, 12);
            rect(cv, x - 8, y - 7, x - 3, y - 4, sh(0, 1));
            rect(cv, x + 3, y - 7, x + 8, y - 4, sh(0, 1));
            grad_rect(cv, x - 13, y + 30 + pump, x + 13, y + 48 + pump, 2, 11, 4);
            grad_rect(cv, x - 15, y + 50, x + 15, h - 1, 2, 9, 3);
            hand(cv, x + 22, h - 26);
        }
        Spr::WChaingun => {
            let spin = if frame == 1 { 1 } else { 0 };
            let (x, y) = (w / 2, 20);
            if frame >= 2 {
                glow(cv, x, y - 10, 14 + (frame as i32 - 2) * 3, 5);
                glow(cv, x, y - 10, 7, 6);
            }
            for k in 0..4 {
                let o = ((k * 2 + spin) % 8) - 4;
                let bx = x + o * 3;
                tube_v(cv, bx - 2, y, bx + 2, y + 44, 13, 3, 11 + (o == 0) as i32 * 3);
            }
            grad_rect(cv, x - 16, y + 30, x + 16, h - 1, 13, 10, 3);
            rect(cv, x - 16, y + 36, x + 16, y + 38, sh(15, 10));
            hand(cv, x + 26, h - 28);
        }
        Spr::WLauncher => {
            let (x, y) = (w / 2, 18);
            if frame >= 2 {
                glow(cv, x, y - 6, 12 + (frame as i32 - 2) * 3, 5);
            }
            tube_v(cv, x - 16, y, x + 16, h - 1, 8, 3, 11);
            disc(cv, x, y + 6, 11, 0, 1, 3);
            rect(cv, x - 16, y + 30, x + 16, y + 32, sh(6, 11));
            if frame == 1 {
                glow(cv, x, y + 6, 5, 5);
            }
            hand(cv, x + 26, h - 26);
        }
        Spr::WPlasma => {
            let (x, y) = (w / 2, 20);
            if frame >= 2 {
                glow(cv, x, y - 8, 14, 10);
                glow(cv, x, y - 8, 7, 0);
            }
            grad_rect(cv, x - 14, y, x + 14, h - 1, 13, 11, 3);
            rect(cv, x - 8, y - 4, x + 8, y + 2, sh(13, 8));
            for k in 0..5 {
                let lit = if frame == 1 { 8 } else { 12 + ((k + anim as i32 / 3) % 4) };
                rect(cv, x - 10, y + 10 + k * 8, x + 10, y + 12 + k * 8, sh(10, lit.clamp(1, 15) as u8));
            }
            hand(cv, x + 24, h - 24);
        }
        Spr::WArc => {
            let (x, y) = (w / 2, 24);
            let charge = if frame == 0 { 8 } else { 14 };
            if frame >= 2 {
                glow(cv, x, y - 6, 22 - (frame as i32 - 2) * 6, 11);
                glow(cv, x, y - 6, 10, 0);
            }
            tube_v(cv, x - 24, y, x + 24, h - 1, 11, 2, 10);
            disc(cv, x, y + 10, 13, 13, 2, 8);
            glow(cv, x, y + 10, 9, 10);
            for k in 0..4 {
                let (s, c) = sc(k * 64 + anim as i32 * 8);
                line(cv, x, y + 10, x + c * 12 / 256, y + 10 + s * 12 / 256, sh(10, charge));
            }
            rect(cv, x - 24, y + 36, x + 24, y + 38, sh(6, 12));
            hand(cv, x + 32, h - 24);
        }
        _ => {}
    }
    d
}

// ------------------------------------------------------------------ public entry points

/// Does this frame look different from different sides? (Then the renderer
/// picks one of 16 views by the angle between the thing and the viewer.)
pub fn rotates(spr: Spr, frame: u8) -> bool {
    matches!(
        spr,
        Spr::Player | Spr::Drone | Spr::Enforcer | Spr::Heavy | Spr::Fiend | Spr::Ripper | Spr::Gazer | Spr::Juggernaut | Spr::Corpse
    ) || (spr == Spr::Rocket && frame == 0)
}

/// Canvas size and anchor for a sprite frame (cheap; used when projecting).
pub fn dims(spr: Spr, frame: u8) -> Dims {
    let lying = matches!(frame, 9..=16);
    let (w, h, top) = match spr {
        Spr::Player | Spr::Drone | Spr::Enforcer | Spr::Heavy | Spr::Fiend => {
            if lying { (112, 40, 40) } else { (72, 66, 64) }
        }
        Spr::Corpse => (112, 40, 40),
        Spr::Ripper => {
            if lying { (96, 64, 64) } else { (84, 56, 54) }
        }
        Spr::Gazer => (80, 72, 70),
        Spr::Juggernaut => {
            if lying { (124, 90, 90) } else { (96, 96, 94) }
        }
        Spr::FiendBall | Spr::GazerBall | Spr::JuggBall => (28, 28, 22),
        Spr::Rocket => {
            if frame == 0 { (22, 12, 10) } else { (48, 48, 34) }
        }
        Spr::Plasma => (24, 24, 20),
        Spr::ArcBall => (56, 56, 40),
        Spr::ArcTrace => (10, 10, 20),
        Spr::Puff => (12, 12, 10),
        Spr::Blood => (10, 10, 8),
        Spr::Fog => (36, 56, 56),
        Spr::Barrel => {
            if frame < 2 { (22, 42, 42) } else { (32, 30, 40) }
        }
        Spr::Lamp => (23, 48, 48),
        Spr::Pillar => (32, 52, 52),
        Spr::Torch => (20, 64, 64),
        Spr::Clip => (12, 16, 16),
        Spr::AmmoBox => (28, 16, 16),
        Spr::Shells => (16, 16, 16),
        Spr::ShellBox => (32, 16, 16),
        Spr::RocketAmmo => (14, 27, 27),
        Spr::RocketBox => (32, 23, 23),
        Spr::Cell => (16, 13, 13),
        Spr::CellPack => (28, 23, 23),
        Spr::Backpack => (22, 22, 22),
        Spr::Stim => (12, 17, 17),
        Spr::Medkit => (28, 20, 20),
        Spr::HealthBonus => (14, 14, 14),
        Spr::ArmorBonus => (15, 15, 15),
        Spr::ArmorGreen | Spr::ArmorBlue => (32, 25, 25),
        Spr::VitalOrb => (23, 23, 36),
        Spr::KeyRed | Spr::KeyBlue | Spr::KeyYellow => (16, 20, 20),
        Spr::PShotgun => (61, 13, 13),
        Spr::PChaingun => (51, 16, 16),
        Spr::PLauncher => (51, 16, 16),
        Spr::PPlasma => (45, 16, 16),
        Spr::PArc => (53, 18, 18),
        Spr::PDrill => (39, 19, 19),
        Spr::Aegis => (23, 23, 34),
        Spr::Hazmat => (23, 31, 31),
        Spr::SurveyMap => (22, 17, 17),
        Spr::Adrenaline => (23, 12, 12),
        Spr::WFist => (110, 90, 90),
        Spr::WDrill => (100, 100, 100),
        Spr::WPistol => (64, 90, 90),
        Spr::WShotgun => (72, 96, 96),
        Spr::WChaingun => (84, 96, 96),
        Spr::WLauncher => (80, 90, 90),
        Spr::WPlasma => (80, 90, 90),
        Spr::WArc => (100, 90, 90),
        Spr::None => (1, 1, 1),
    };
    Dims { w, h, left: w / 2, top }
}

/// Renders a sprite frame into the canvas and returns its dimensions.
pub fn draw(cv: &mut Canvas, spr: Spr, frame: u8, rot: u8, anim: u16) -> Dims {
    match spr {
        Spr::Player | Spr::Drone | Spr::Enforcer | Spr::Heavy | Spr::Fiend | Spr::Ripper | Spr::Gazer | Spr::Juggernaut | Spr::Corpse => {
            let d = dims(spr, frame);
            cv.begin(d.w as usize, d.h as usize);
            let mut m = Model::new();
            match spr {
                Spr::Player => humanoid(&mut m, &PLAYER, frame),
                Spr::Drone => humanoid(&mut m, &DRONE, frame),
                Spr::Corpse => humanoid(&mut m, &DRONE, 11),
                Spr::Enforcer => humanoid(&mut m, &ENFORCER, frame),
                Spr::Heavy => humanoid(&mut m, &HEAVY, frame),
                Spr::Fiend => fiend(&mut m, frame),
                Spr::Ripper => ripper(&mut m, frame),
                Spr::Gazer => gazer(&mut m, frame, anim),
                _ => juggernaut(&mut m, frame),
            }
            let cam = Cam::new(rot, d.left, d.top);
            render_model(cv, &m, &cam);
            d
        }
        Spr::WFist | Spr::WDrill | Spr::WPistol | Spr::WShotgun | Spr::WChaingun | Spr::WLauncher | Spr::WPlasma | Spr::WArc => {
            draw_weapon(cv, spr, frame, anim)
        }
        Spr::FiendBall
        | Spr::GazerBall
        | Spr::JuggBall
        | Spr::Rocket
        | Spr::Plasma
        | Spr::ArcBall
        | Spr::ArcTrace
        | Spr::Puff
        | Spr::Blood
        | Spr::Fog => draw_effect(cv, spr, frame, rot, anim),
        _ => draw_item(cv, spr, frame, anim),
    }
}
