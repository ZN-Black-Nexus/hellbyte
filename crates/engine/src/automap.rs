//! Overhead map of the lines the player has seen.

use crate::data::*;
use crate::draw::Surface;
use crate::fixed::*;
use crate::game::Game;

pub struct Automap {
    pub active: bool,
    /// Screen pixels per map unit, 16.16.
    pub scale: Fixed,
    pub follow: bool,
    pub cx: Fixed,
    pub cy: Fixed,
}

impl Automap {
    pub const NEW: Automap = Automap { active: false, scale: FRACUNIT / 5, follow: true, cx: 0, cy: 0 };

    pub fn zoom(&mut self, zoom_in: bool) {
        self.scale = if zoom_in { self.scale * 5 / 4 } else { self.scale * 4 / 5 };
        self.scale = self.scale.clamp(FRACUNIT / 40, FRACUNIT * 2);
    }

    pub fn pan(&mut self, dx: i32, dy: i32) {
        self.follow = false;
        let step = fdiv(fx(8), self.scale);
        self.cx += dx * step;
        self.cy += dy * step;
    }

    pub fn draw(&mut self, g: &Game, seen: &[u32], s: &mut Surface) {
        let pmo = g.mo(g.player.mo);
        if self.follow {
            self.cx = pmo.x;
            self.cy = pmo.y;
        }
        s.fill(0, 0, s.w - 1, s.h - 1, 1);
        let (hw, hh) = (s.w / 2, s.h / 2);
        let sc = self.scale;
        let to_screen = |x: Fixed, y: Fixed| -> (i32, i32) {
            (hw + (fmul(x - self.cx, sc) >> FRACBITS), hh - (fmul(y - self.cy, sc) >> FRACBITS))
        };
        let allmap = g.player.powers[crate::player::PW_ALLMAP] > 0;
        let lv = &g.lv;
        for (i, ld) in lv.map.lines.iter().enumerate() {
            let flags = lv.lines[i].flags;
            let mapped = seen[i / 32] & (1 << (i % 32)) != 0 || flags & ML_MAPPED != 0;
            if flags & ML_DONTDRAW != 0 || (!mapped && !allmap) {
                continue;
            }
            let v1 = lv.vert(ld.v1);
            let v2 = lv.vert(ld.v2);
            let (x1, y1) = to_screen(v1.x, v1.y);
            let (x2, y2) = to_screen(v2.x, v2.y);
            if (x1 < 0 && x2 < 0) || (y1 < 0 && y2 < 0) || (x1 >= s.w && x2 >= s.w) || (y1 >= s.h && y2 >= s.h) {
                continue;
            }
            let color = if !mapped {
                8 // revealed by the survey chip, never seen: grey
            } else {
                match lv.back_sector(i) {
                    None => 4 * 16 + 12,
                    Some(_) if flags & ML_SECRET != 0 => 4 * 16 + 12,
                    Some(b) => {
                        let f = &lv.sectors[lv.front_sector(i)];
                        let b = &lv.sectors[b];
                        if f.floor != b.floor {
                            2 * 16 + 12
                        } else if f.ceil != b.ceil {
                            6 * 16 + 13
                        } else {
                            continue;
                        }
                    }
                }
            };
            s.line(x1, y1, x2, y2, color);
        }
        // Player arrow.
        let a = pmo.angle;
        let len = 16 * FRACUNIT;
        let tip = (pmo.x + fmul(len, cos_a(a)), pmo.y + fmul(len, sin_a(a)));
        let l = (pmo.x + fmul(len / 2, cos_a(a.wrapping_add(ANG90 + ANG45))), pmo.y + fmul(len / 2, sin_a(a.wrapping_add(ANG90 + ANG45))));
        let r = (pmo.x + fmul(len / 2, cos_a(a.wrapping_sub(ANG90 + ANG45))), pmo.y + fmul(len / 2, sin_a(a.wrapping_sub(ANG90 + ANG45))));
        let tail = (pmo.x - fmul(len, cos_a(a)), pmo.y - fmul(len, sin_a(a)));
        let (tx, ty) = to_screen(tip.0, tip.1);
        let (lx, ly) = to_screen(l.0, l.1);
        let (rx, ry) = to_screen(r.0, r.1);
        let (bx, by) = to_screen(tail.0, tail.1);
        let c = 15;
        s.line(bx, by, tx, ty, c);
        s.line(tx, ty, lx, ly, c);
        s.line(tx, ty, rx, ry, c);
    }
}
