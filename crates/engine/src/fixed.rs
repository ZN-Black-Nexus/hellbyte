//! 16.16 fixed-point numbers and 32-bit binary angles.
//!
//! The whole engine is integer-only so it runs at full speed on CPUs without
//! an FPU (most router SoCs, old ARM/MIPS parts, microcontrollers).

use crate::data::{FINESINE_Q, FINETAN_H, TANTOANGLE};

pub type Fixed = i32;
pub const FRACBITS: u32 = 16;
pub const FRACUNIT: Fixed = 1 << FRACBITS;

#[inline(always)]
pub fn fmul(a: Fixed, b: Fixed) -> Fixed {
    ((a as i64 * b as i64) >> FRACBITS) as Fixed
}

/// Fixed-point division that saturates instead of overflowing.
#[inline]
pub fn fdiv(a: Fixed, b: Fixed) -> Fixed {
    if (a.unsigned_abs() >> 14) >= b.unsigned_abs() {
        if (a ^ b) < 0 { i32::MIN } else { i32::MAX }
    } else {
        (((a as i64) << FRACBITS) / b as i64) as Fixed
    }
}

#[inline(always)]
pub const fn fx(units: i32) -> Fixed {
    units << FRACBITS
}

/// A full turn is 2^32.
pub type Angle = u32;
pub const ANG45: Angle = 0x2000_0000;
pub const ANG90: Angle = 0x4000_0000;
pub const ANG180: Angle = 0x8000_0000;
pub const ANG270: Angle = 0xc000_0000;
pub const ANG1: Angle = ANG45 / 45;

pub const FINEANGLES: usize = 8192;
pub const FINEMASK: usize = FINEANGLES - 1;
pub const ANGLETOFINESHIFT: u32 = 19;

#[inline]
pub fn finesine(i: usize) -> Fixed {
    let i = i & FINEMASK;
    let j = i & 2047;
    match i >> 11 {
        0 => FINESINE_Q[j],
        1 => FINESINE_Q[2047 - j],
        2 => -FINESINE_Q[j],
        _ => -FINESINE_Q[2047 - j],
    }
}

#[inline]
pub fn finecosine(i: usize) -> Fixed {
    finesine(i + FINEANGLES / 4)
}

/// Tangent over half a turn: index 0..4096 maps -90..+90 degrees.
#[inline]
pub fn finetangent(i: usize) -> Fixed {
    let i = i & 4095;
    if i >= 2048 { FINETAN_H[i - 2048] } else { -FINETAN_H[2047 - i] }
}

#[inline]
pub fn sin_a(a: Angle) -> Fixed {
    finesine((a >> ANGLETOFINESHIFT) as usize)
}

#[inline]
pub fn cos_a(a: Angle) -> Fixed {
    finecosine((a >> ANGLETOFINESHIFT) as usize)
}

#[inline]
fn slope(num: u32, den: u32) -> usize {
    if den == 0 {
        return 2048;
    }
    (((num as u64) << 11) / den as u64).min(2048) as usize
}

/// Angle of the vector (dx, dy), measured counter-clockwise from +x.
pub fn point_to_angle(dx: Fixed, dy: Fixed) -> Angle {
    if dx == 0 && dy == 0 {
        return 0;
    }
    let (ax, ay) = (dx.unsigned_abs(), dy.unsigned_abs());
    if dx >= 0 {
        if dy >= 0 {
            if ax > ay { TANTOANGLE[slope(ay, ax)] } else { ANG90 - 1 - TANTOANGLE[slope(ax, ay)] }
        } else if ax > ay {
            0u32.wrapping_sub(TANTOANGLE[slope(ay, ax)])
        } else {
            ANG270 + TANTOANGLE[slope(ax, ay)]
        }
    } else if dy >= 0 {
        if ax > ay { ANG180 - 1 - TANTOANGLE[slope(ay, ax)] } else { ANG90 + TANTOANGLE[slope(ax, ay)] }
    } else if ax > ay {
        ANG180 + TANTOANGLE[slope(ay, ax)]
    } else {
        ANG270 - 1 - TANTOANGLE[slope(ax, ay)]
    }
}

/// Cheap distance estimate (octagonal norm), good to within ~8%.
#[inline]
pub fn approx_dist(dx: Fixed, dy: Fixed) -> Fixed {
    let dx = dx.wrapping_abs();
    let dy = dy.wrapping_abs();
    if dx < dy { dx + dy - (dx >> 1) } else { dx + dy - (dy >> 1) }
}

/// Exact-ish Euclidean distance (used where precision matters, e.g. the renderer).
pub fn point_dist(dx: Fixed, dy: Fixed) -> Fixed {
    let d = (dx as i64) * (dx as i64) + (dy as i64) * (dy as i64);
    isqrt64(d as u64) as Fixed
}

pub fn isqrt64(v: u64) -> u64 {
    if v < 2 {
        return v;
    }
    // Newton's method from an over-estimate converges monotonically.
    let bits = 64 - v.leading_zeros();
    let mut x = 1u64 << bits.div_ceil(2);
    loop {
        let y = (x + v / x) / 2;
        if y >= x {
            return x;
        }
        x = y;
    }
}
