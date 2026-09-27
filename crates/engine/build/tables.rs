//! Fixed-point trig tables. Angles are 32-bit binary angles; the "fine"
//! tables have 8192 steps per turn and sample at half-step offsets so no
//! entry is exactly zero (avoids divide-by-zero in the renderer).
use std::f64::consts::PI;
use std::fmt::Write;

pub fn emit(out: &mut String) {
    let step = 2.0 * PI / 8192.0;
    // Quarter sine wave: sin((i + 0.5) * step), i in 0..2048
    out.push_str("pub static FINESINE_Q: [i32; 2048] = [");
    for i in 0..2048 {
        let v = ((i as f64 + 0.5) * step).sin() * 65536.0;
        write!(out, "{},", v.round() as i32).unwrap();
    }
    out.push_str("];\n");
    // Positive half of the tangent: tan((i + 0.5) * step), i in 0..2048
    out.push_str("pub static FINETAN_H: [i32; 2048] = [");
    for i in 0..2048 {
        let v = ((i as f64 + 0.5) * step).tan() * 65536.0;
        write!(out, "{},", v.round().min(i32::MAX as f64) as i32).unwrap();
    }
    out.push_str("];\n");
    // atan(i / 2048) as a binary angle, i in 0..=2048
    out.push_str("pub static TANTOANGLE: [u32; 2049] = [");
    for i in 0..=2048 {
        let a = (i as f64 / 2048.0).atan() / (2.0 * PI) * 4294967296.0;
        write!(out, "{},", a.round() as u32).unwrap();
    }
    out.push_str("];\n");
}
