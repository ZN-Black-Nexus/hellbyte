//! The Hellbyte palette: 16 colour ramps x 16 shades (index = ramp*16 + shade,
//! shade 0 darkest). Index 0 is pure black and doubles as "transparent" in
//! sprite canvases. Light-diminishing colormaps are generated from it.
use std::fmt::Write;

/// (dark, mid, bright) control colours for each ramp.
pub const RAMPS: [[(u8, u8, u8); 3]; 16] = [
    [(0, 0, 0), (115, 115, 115), (255, 255, 255)],    // 0 gray
    [(10, 9, 8), (104, 96, 86), (216, 208, 194)],     // 1 stone
    [(12, 7, 3), (104, 68, 38), (206, 160, 110)],     // 2 brown
    [(18, 12, 6), (150, 114, 78), (255, 226, 184)],   // 3 tan / skin
    [(18, 0, 0), (160, 24, 18), (255, 130, 105)],     // 4 red
    [(36, 4, 0), (224, 96, 8), (255, 248, 170)],      // 5 fire
    [(20, 14, 0), (176, 140, 24), (255, 250, 160)],   // 6 gold
    [(0, 14, 0), (44, 152, 32), (190, 255, 130)],     // 7 green
    [(8, 10, 4), (84, 92, 44), (186, 194, 128)],      // 8 olive
    [(0, 3, 20), (36, 64, 172), (160, 196, 255)],     // 9 blue
    [(0, 14, 16), (22, 140, 148), (176, 255, 250)],   // 10 teal
    [(10, 0, 16), (112, 34, 142), (234, 168, 255)],   // 11 purple
    [(20, 5, 6), (172, 82, 92), (255, 196, 196)],     // 12 flesh
    [(6, 8, 12), (84, 94, 114), (196, 210, 230)],     // 13 steel
    [(12, 2, 2), (96, 32, 22), (198, 116, 86)],       // 14 maroon
    [(14, 5, 1), (136, 64, 26), (246, 176, 116)],     // 15 copper
];

pub const NUM_COLORMAPS: usize = 32;

pub fn build() -> Vec<[u8; 3]> {
    let mut pal = Vec::with_capacity(256);
    for r in RAMPS.iter() {
        for s in 0..16 {
            let t = s as f64 / 15.0;
            let (a, b, u) = if t < 0.5 { (r[0], r[1], t * 2.0) } else { (r[1], r[2], (t - 0.5) * 2.0) };
            let lerp = |x: u8, y: u8| (x as f64 + (y as f64 - x as f64) * u).round() as u8;
            pal.push([lerp(a.0, b.0), lerp(a.1, b.1), lerp(a.2, b.2)]);
        }
    }
    pal
}

pub fn nearest(pal: &[[u8; 3]], r: f64, g: f64, b: f64) -> u8 {
    let mut best = (f64::MAX, 0usize);
    for (i, c) in pal.iter().enumerate() {
        let (dr, dg, db) = (c[0] as f64 - r, c[1] as f64 - g, c[2] as f64 - b);
        // Weighted for human perception.
        let d = dr * dr * 0.30 + dg * dg * 0.59 + db * db * 0.11;
        if d < best.0 {
            best = (d, i);
        }
    }
    best.1 as u8
}

/// 32 light levels (0 = full bright) followed by the "invulnerable" inverse map.
pub fn colormaps(pal: &[[u8; 3]]) -> Vec<u8> {
    let mut maps = Vec::with_capacity((NUM_COLORMAPS + 1) * 256);
    for l in 0..NUM_COLORMAPS {
        let f = (NUM_COLORMAPS - l) as f64 / NUM_COLORMAPS as f64;
        for c in pal {
            // Keep ramp identity where possible: search nearest colour.
            maps.push(nearest(pal, c[0] as f64 * f, c[1] as f64 * f, c[2] as f64 * f));
        }
    }
    for c in pal {
        let lum = 0.30 * c[0] as f64 + 0.59 * c[1] as f64 + 0.11 * c[2] as f64;
        let v = 255.0 - lum;
        maps.push(nearest(&pal[..16], v, v, v));
    }
    maps
}

pub fn emit(out: &mut String, pal: &[[u8; 3]]) {
    out.push_str("pub static PALETTE: [u8; 768] = [");
    for c in pal {
        write!(out, "{},{},{},", c[0], c[1], c[2]).unwrap();
    }
    out.push_str("];\n");
    let maps = colormaps(pal);
    writeln!(out, "pub const NUM_COLORMAPS: usize = {NUM_COLORMAPS};").unwrap();
    write!(out, "pub static COLORMAPS: [u8; {}] = [", maps.len()).unwrap();
    for m in maps {
        write!(out, "{m},").unwrap();
    }
    out.push_str("];\n");
}
