//! Monster behaviour: waking up, chasing, attacking, noise propagation.

use crate::data::ML_SOUNDBLOCK;
use crate::fixed::*;
use crate::game::{Game, Skill};
use crate::info::*;
use crate::level::*;
use crate::map::{MELEERANGE, MISSILERANGE};

const NODIR: u8 = 8;
const DI_EAST: u8 = 0;
const DI_NORTH: u8 = 2;
const DI_WEST: u8 = 4;
const DI_SOUTH: u8 = 6;
const OPPOSITE: [u8; 9] = [4, 5, 6, 7, 0, 1, 2, 3, NODIR];
const DIAGS: [u8; 4] = [3, 1, 5, 7]; // NW, NE, SW, SE
const XSPEED: [Fixed; 8] = [FRACUNIT, 47000, 0, -47000, -FRACUNIT, -47000, 0, 47000];
const YSPEED: [Fixed; 8] = [0, 47000, FRACUNIT, 47000, 0, -47000, -FRACUNIT, -47000];

pub const BASETHRESHOLD: i16 = 100;

impl Game {
    // ================================================================ noise

    /// Wake up monsters that can "hear" `emitter` (gunfire), spreading
    /// through open two-sided lines; sound-blocking lines halve the range.
    pub fn noise_alert(&mut self, target: MRef, emitter: MRef) {
        self.lv.validcount = self.lv.validcount.wrapping_add(1);
        let sec = self.mobjs[emitter as usize].sector as usize;
        self.recursive_sound(sec, 0, target, 0);
    }

    fn recursive_sound(&mut self, sec: usize, soundblocks: u8, target: MRef, depth: u32) {
        if depth > 256 {
            return;
        }
        let s = &self.lv.sectors[sec];
        if s.validcount == self.lv.validcount && s.soundtraversed <= soundblocks + 1 {
            return;
        }
        let vc = self.lv.validcount;
        let s = &mut self.lv.sectors[sec];
        s.validcount = vc;
        s.soundtraversed = soundblocks + 1;
        s.soundtarget = target;
        for &l in self.lv.sector_lines(sec) {
            let l = l as usize;
            let Some((_, _, range, _)) = self.lv.line_opening(l) else { continue };
            if range <= 0 {
                continue;
            }
            let Some(other) = self.lv.other_sector(l, sec) else { continue };
            if self.lv.lines[l].flags & ML_SOUNDBLOCK != 0 {
                if soundblocks == 0 {
                    self.recursive_sound(other, 1, target, depth + 1);
                }
            } else {
                self.recursive_sound(other, soundblocks, target, depth + 1);
            }
        }
    }

    // ================================================================ perception

    fn check_melee_range(&mut self, m: MRef) -> bool {
        let mo = self.mobjs[m as usize];
        if mo.target == NONE {
            return false;
        }
        let t = self.mobjs[mo.target as usize];
        let dist = approx_dist(t.x - mo.x, t.y - mo.y);
        if dist >= MELEERANGE - 20 * FRACUNIT + t.radius {
            return false;
        }
        self.check_sight(m, mo.target)
    }

    fn check_missile_range(&mut self, m: MRef) -> bool {
        let mo = self.mobjs[m as usize];
        if !self.check_sight(m, mo.target) {
            return false;
        }
        let mo = &mut self.mobjs[m as usize];
        if mo.flags & MF_JUSTHIT != 0 {
            mo.flags &= !MF_JUSTHIT; // just got hit: fight back
            return true;
        }
        if mo.reactiontime != 0 {
            return false;
        }
        let t = self.mobjs[mo.target as usize];
        let mo = self.mobjs[m as usize];
        let mut dist = approx_dist(mo.x - t.x, mo.y - t.y) - 64 * FRACUNIT;
        if mo.info().melee == S::Null {
            dist -= 128 * FRACUNIT; // no melee: fire more
        }
        let mut dist = dist >> FRACBITS;
        if mo.kind == ThingKind::Juggernaut {
            dist >>= 1;
        }
        if dist > 200 {
            dist = 200;
        }
        self.rnd() >= dist
    }

    fn look_for_player(&mut self, m: MRef, allaround: bool) -> bool {
        let p = self.player.mo;
        if p == NONE || !self.mobjs[p as usize].in_use || self.player.health <= 0 {
            return false;
        }
        if !self.check_sight(m, p) {
            return false;
        }
        if !allaround {
            let mo = self.mobjs[m as usize];
            let pm = self.mobjs[p as usize];
            let an = point_to_angle(pm.x - mo.x, pm.y - mo.y).wrapping_sub(mo.angle);
            if an > ANG90 && an < ANG270 {
                // Behind: only notice if very close.
                if approx_dist(pm.x - mo.x, pm.y - mo.y) > MELEERANGE {
                    return false;
                }
            }
        }
        self.mobjs[m as usize].target = p;
        true
    }

    // ================================================================ movement

    fn monster_move(&mut self, m: MRef) -> bool {
        let mo = self.mobjs[m as usize];
        if mo.movedir == NODIR {
            return false;
        }
        let speed = mo.info().speed;
        let d = mo.movedir as usize;
        let tx = mo.x + fmul(speed, XSPEED[d]);
        let ty = mo.y + fmul(speed, YSPEED[d]);
        if !self.try_move(m, tx, ty) {
            let mo = self.mobjs[m as usize];
            if mo.flags & MF_FLOAT != 0 && self.tm.floatok {
                let mo = &mut self.mobjs[m as usize];
                if mo.z < self.tm.floorz {
                    mo.z += crate::mobj::FLOATSPEED;
                } else {
                    mo.z -= crate::mobj::FLOATSPEED;
                }
                mo.flags |= MF_INFLOAT;
                return true;
            }
            if self.tm.numspechit == 0 {
                return false;
            }
            self.mobjs[m as usize].movedir = NODIR;
            let hits = self.tm.spechit;
            let n = self.tm.numspechit;
            let mut good = false;
            for &l in hits[..n].iter().rev() {
                // Monsters can open some doors by walking into them.
                if self.use_special_line(m, l as usize, 0) {
                    good = true;
                }
            }
            return good;
        }
        let mo = &mut self.mobjs[m as usize];
        mo.flags &= !MF_INFLOAT;
        if mo.flags & MF_FLOAT == 0 {
            mo.z = mo.floorz;
        }
        true
    }

    fn try_walk(&mut self, m: MRef) -> bool {
        if !self.monster_move(m) {
            return false;
        }
        let r = self.rnd() & 15;
        self.mobjs[m as usize].movecount = r as i16;
        true
    }

    fn new_chase_dir(&mut self, m: MRef) {
        let mo = self.mobjs[m as usize];
        if mo.target == NONE {
            return;
        }
        let t = self.mobjs[mo.target as usize];
        let olddir = mo.movedir;
        let turnaround = OPPOSITE[olddir.min(8) as usize];
        let deltax = t.x - mo.x;
        let deltay = t.y - mo.y;
        let ten = 10 * FRACUNIT;
        let mut d1 = if deltax > ten {
            DI_EAST
        } else if deltax < -ten {
            DI_WEST
        } else {
            NODIR
        };
        let mut d2 = if deltay < -ten {
            DI_SOUTH
        } else if deltay > ten {
            DI_NORTH
        } else {
            NODIR
        };
        if d1 != NODIR && d2 != NODIR {
            let dir = DIAGS[(((deltay < 0) as usize) << 1) + (deltax > 0) as usize];
            self.mobjs[m as usize].movedir = dir;
            if dir != turnaround && self.try_walk(m) {
                return;
            }
        }
        if self.rnd() > 200 || deltay.abs() > deltax.abs() {
            core::mem::swap(&mut d1, &mut d2);
        }
        if d1 == turnaround {
            d1 = NODIR;
        }
        if d2 == turnaround {
            d2 = NODIR;
        }
        for d in [d1, d2] {
            if d != NODIR {
                self.mobjs[m as usize].movedir = d;
                if self.try_walk(m) {
                    return;
                }
            }
        }
        if olddir != NODIR {
            self.mobjs[m as usize].movedir = olddir;
            if self.try_walk(m) {
                return;
            }
        }
        if self.rnd() & 1 != 0 {
            for d in 0..8u8 {
                if d != turnaround {
                    self.mobjs[m as usize].movedir = d;
                    if self.try_walk(m) {
                        return;
                    }
                }
            }
        } else {
            for d in (0..8u8).rev() {
                if d != turnaround {
                    self.mobjs[m as usize].movedir = d;
                    if self.try_walk(m) {
                        return;
                    }
                }
            }
        }
        if turnaround != NODIR {
            self.mobjs[m as usize].movedir = turnaround;
            if self.try_walk(m) {
                return;
            }
        }
        self.mobjs[m as usize].movedir = NODIR;
    }

    // ================================================================ state actions

    pub fn a_look(&mut self, m: MRef) {
        self.mobjs[m as usize].threshold = 0;
        let sec = self.mobjs[m as usize].sector as usize;
        let targ = self.lv.sectors[sec].soundtarget;
        let mut seen = false;
        if targ != NONE && self.mobjs[targ as usize].flags & MF_SHOOTABLE != 0 {
            self.mobjs[m as usize].target = targ;
            if self.mobjs[m as usize].flags & MF_AMBUSH != 0 {
                seen = self.check_sight(m, targ);
            } else {
                seen = true;
            }
        }
        if !seen && !self.look_for_player(m, false) {
            return;
        }
        let see = self.mobjs[m as usize].info().see;
        self.set_state(m, see);
    }

    pub fn a_chase(&mut self, m: MRef) {
        {
            let mo = &mut self.mobjs[m as usize];
            if mo.reactiontime > 0 {
                mo.reactiontime -= 1;
            }
        }
        let mo = self.mobjs[m as usize];
        if mo.threshold > 0 {
            if mo.target == NONE || self.mobjs[mo.target as usize].health <= 0 {
                self.mobjs[m as usize].threshold = 0;
            } else {
                self.mobjs[m as usize].threshold -= 1;
            }
        }
        // Turn towards the movement direction in 45 degree steps.
        {
            let mo = &mut self.mobjs[m as usize];
            if mo.movedir < 8 {
                mo.angle &= 7 << 29;
                let delta = mo.angle.wrapping_sub((mo.movedir as u32) << 29) as i32;
                if delta > 0 {
                    mo.angle = mo.angle.wrapping_sub(ANG45);
                } else if delta < 0 {
                    mo.angle = mo.angle.wrapping_add(ANG45);
                }
            }
        }
        let mo = self.mobjs[m as usize];
        if mo.target == NONE || self.mobjs[mo.target as usize].flags & MF_SHOOTABLE == 0 {
            if self.look_for_player(m, true) {
                return;
            }
            let spawn = mo.info().spawn;
            self.set_state(m, spawn);
            return;
        }
        if mo.flags & MF_JUSTATTACKED != 0 {
            self.mobjs[m as usize].flags &= !MF_JUSTATTACKED;
            if self.skill != Skill::Inferno {
                self.new_chase_dir(m);
            }
            return;
        }
        let inf = mo.info();
        if inf.melee != S::Null && self.check_melee_range(m) {
            self.set_state(m, inf.melee);
            return;
        }
        if inf.missile != S::Null {
            let wait = self.skill != Skill::Inferno && self.mobjs[m as usize].movecount != 0;
            if !wait && self.check_missile_range(m) {
                self.set_state(m, inf.missile);
                self.mobjs[m as usize].flags |= MF_JUSTATTACKED;
                return;
            }
        }
        let mc = {
            let mo = &mut self.mobjs[m as usize];
            mo.movecount -= 1;
            mo.movecount
        };
        if mc < 0 || !self.monster_move(m) {
            self.new_chase_dir(m);
        }
    }

    pub fn a_face_target(&mut self, m: MRef) {
        let mo = self.mobjs[m as usize];
        if mo.target == NONE {
            return;
        }
        let t = self.mobjs[mo.target as usize];
        let mut an = point_to_angle(t.x - mo.x, t.y - mo.y);
        if t.flags & MF_SHADOW != 0 {
            an = an.wrapping_add((self.rnd_sub() << 21) as u32);
        }
        let mo = &mut self.mobjs[m as usize];
        mo.flags &= !MF_AMBUSH;
        mo.angle = an;
    }

    fn hitscan(&mut self, m: MRef, pellets: i32) {
        if self.mobjs[m as usize].target == NONE {
            return;
        }
        self.a_face_target(m);
        let angle = self.mobjs[m as usize].angle;
        let slope = self.aim_line_attack(m, angle, MISSILERANGE);
        for _ in 0..pellets {
            let a = angle.wrapping_add((self.rnd_sub() << 20) as u32);
            let damage = ((self.rnd() % 5) + 1) * 3;
            self.line_attack(m, a, MISSILERANGE, slope, damage);
        }
    }

    pub fn a_drone_shoot(&mut self, m: MRef) {
        self.hitscan(m, 1);
    }

    pub fn a_enforcer_shoot(&mut self, m: MRef) {
        self.hitscan(m, 3);
    }

    pub fn a_heavy_refire(&mut self, m: MRef) {
        self.a_face_target(m);
        if self.rnd() < 40 {
            return;
        }
        let t = self.mobjs[m as usize].target;
        if t == NONE || self.mobjs[t as usize].health <= 0 || !self.check_sight(m, t) {
            let see = self.mobjs[m as usize].info().see;
            self.set_state(m, see);
        }
    }

    fn melee_or_missile(&mut self, m: MRef, melee: Option<(i32, i32)>, missile: Option<ThingKind>) {
        let t = self.mobjs[m as usize].target;
        if t == NONE {
            return;
        }
        self.a_face_target(m);
        if let Some((sides, mult)) = melee {
            if self.check_melee_range(m) {
                let damage = ((self.rnd() % sides) + 1) * mult;
                self.damage_mobj(t, m, m, damage);
                return;
            }
        }
        if let Some(k) = missile {
            self.spawn_missile(m, t, k);
        }
    }

    pub fn a_fiend_attack(&mut self, m: MRef) {
        self.melee_or_missile(m, Some((8, 3)), Some(ThingKind::FiendBall));
    }

    pub fn a_ripper_bite(&mut self, m: MRef) {
        self.melee_or_missile(m, Some((10, 4)), None);
    }

    pub fn a_gazer_attack(&mut self, m: MRef) {
        self.melee_or_missile(m, Some((6, 10)), Some(ThingKind::GazerBall));
    }

    pub fn a_jugg_attack(&mut self, m: MRef) {
        self.melee_or_missile(m, Some((8, 10)), Some(ThingKind::JuggBall));
    }

    /// When the last boss-flagged monster of a kind dies, sectors tagged 666
    /// lower their floors (level designers use this to open the exit).
    pub fn a_boss_death(&mut self, m: MRef) {
        if self.player.health <= 0 {
            return;
        }
        let kind = self.mobjs[m as usize].kind;
        let alive = self
            .mobjs
            .iter()
            .enumerate()
            .any(|(i, o)| i != m as usize && o.in_use && o.kind == kind && o.health > 0);
        if alive {
            return;
        }
        let mut s = -1isize;
        while let Some(sec) = self.lv.find_sector_from_tag(666, s) {
            s = sec as isize;
            if self.lv.sectors[sec].mover != 0 {
                continue;
            }
            let low = self.lv.lowest_floor_surrounding(sec);
            if let Some(i) = self.movers.iter().position(|mv| mv.kind == crate::spec::MoverKind::None) {
                self.movers[i] = crate::spec::Mover {
                    kind: crate::spec::MoverKind::Floor,
                    sector: sec as u16,
                    dir: -1,
                    speed: FRACUNIT,
                    low,
                    ..crate::spec::Mover::EMPTY
                };
                self.lv.sectors[sec].mover = (i + 1) as u16;
            }
        }
    }
}
