//! Movement clipping, traces and hit detection against the level geometry.

use crate::fixed::*;
use crate::game::Game;
use crate::info::*;
use crate::level::*;

pub const MAXRADIUS: Fixed = 32 * FRACUNIT;
pub const MELEERANGE: Fixed = 64 * FRACUNIT;
pub const MISSILERANGE: Fixed = 32 * 64 * FRACUNIT;
pub const USERANGE: Fixed = 64 * FRACUNIT;
pub const MAXSTEP: Fixed = 24 * FRACUNIT;

const MAX_INTERCEPTS: usize = 128;
pub const PT_ADDLINES: u8 = 1;
pub const PT_ADDTHINGS: u8 = 2;
pub const PT_EARLYOUT: u8 = 4;

#[derive(Clone, Copy)]
struct Intercept {
    frac: Fixed,
    line: i32,
    thing: MRef,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Traverse {
    Slide,
    Aim,
    Shoot,
    Use,
    Sight,
}

fn blockshift(v: Fixed, origin: i32) -> i32 {
    (v - fx(origin)) >> (FRACBITS + 7)
}

/// Fraction along divline `a` where it crosses divline `b`.
fn intercept_vector(ax: Fixed, ay: Fixed, adx: Fixed, ady: Fixed, bx: Fixed, by: Fixed, bdx: Fixed, bdy: Fixed) -> Fixed {
    let den = ((bdy as i64) * (adx as i64) - (bdx as i64) * (ady as i64)) >> 16;
    if den == 0 {
        return 0;
    }
    let num = (((bx - ax) as i64) * (bdy as i64) + ((ay - by) as i64) * (bdx as i64)) >> 16;
    ((num << 16) / den).clamp(i32::MIN as i64, i32::MAX as i64) as Fixed
}

impl Game {
    // ================================================================ position checks

    /// Can `m` stand at (x, y)? Fills `tm` with the floor/ceiling found there.
    pub fn check_position(&mut self, m: MRef, x: Fixed, y: Fixed) -> bool {
        let mo = self.mobjs[m as usize];
        self.tm.thing = m;
        self.tm.flags = mo.flags;
        self.tm.x = x;
        self.tm.y = y;
        self.tm.bbox = [y + mo.radius, y - mo.radius, x - mo.radius, x + mo.radius];
        let sec = self.lv.sector_at(x, y);
        self.tm.ceilingline = -1;
        self.tm.floorz = self.lv.sectors[sec].floor;
        self.tm.dropoffz = self.tm.floorz;
        self.tm.ceilingz = self.lv.sectors[sec].ceil;
        self.lv.validcount = self.lv.validcount.wrapping_add(1);
        self.tm.numspechit = 0;
        if mo.flags & MF_NOCLIP != 0 {
            return true;
        }
        let bm = &self.lv.map.blockmap;
        let (ox, oy) = (bm.x, bm.y);
        let b = self.tm.bbox;
        let xl = blockshift(b[BOXLEFT] - MAXRADIUS, ox);
        let xh = blockshift(b[BOXRIGHT] + MAXRADIUS, ox);
        let yl = blockshift(b[BOXBOTTOM] - MAXRADIUS, oy);
        let yh = blockshift(b[BOXTOP] + MAXRADIUS, oy);
        for bx in xl..=xh {
            for by in yl..=yh {
                if !self.block_things(bx, by, &mut |g, t| g.pit_check_thing(t)) {
                    return false;
                }
            }
        }
        let xl = blockshift(b[BOXLEFT], ox);
        let xh = blockshift(b[BOXRIGHT], ox);
        let yl = blockshift(b[BOXBOTTOM], oy);
        let yh = blockshift(b[BOXTOP], oy);
        for bx in xl..=xh {
            for by in yl..=yh {
                if !self.block_lines(bx, by, &mut |g, l| g.pit_check_line(l)) {
                    return false;
                }
            }
        }
        true
    }

    /// Iterate the mobjs linked into a blockmap cell.
    pub fn block_things(&mut self, bx: i32, by: i32, f: &mut dyn FnMut(&mut Game, MRef) -> bool) -> bool {
        let Some(cell) = self.lv.block_index(bx, by) else { return true };
        let mut m = self.lv.blocklinks[cell];
        let mut guard = 0;
        while m != NONE && guard < crate::limits::MAX_MOBJS {
            let next = self.mobjs[m as usize].bnext;
            if !f(self, m) {
                return false;
            }
            m = next;
            guard += 1;
        }
        true
    }

    pub fn block_lines(&mut self, bx: i32, by: i32, f: &mut dyn FnMut(&mut Game, usize) -> bool) -> bool {
        let Some(cell) = self.lv.block_index(bx, by) else { return true };
        let bm = &self.lv.map.blockmap;
        let mut i = bm.offsets[cell] as usize;
        loop {
            let li = self.lv.map.blockmap.lines[i];
            if li == 0xffff {
                return true;
            }
            i += 1;
            let li = li as usize;
            if self.lv.lines[li].validcount == self.lv.validcount {
                continue;
            }
            self.lv.lines[li].validcount = self.lv.validcount;
            if !f(self, li) {
                return false;
            }
        }
    }

    fn pit_check_line(&mut self, ld: usize) -> bool {
        let lb = self.lv.line_bbox(ld);
        let b = self.tm.bbox;
        if b[BOXRIGHT] <= lb[BOXLEFT] || b[BOXLEFT] >= lb[BOXRIGHT] || b[BOXTOP] <= lb[BOXBOTTOM] || b[BOXBOTTOM] >= lb[BOXTOP] {
            return true;
        }
        if self.lv.box_on_line_side(&b, ld) != -1 {
            return true;
        }
        let Some((opentop, openbottom, _, lowfloor)) = self.lv.line_opening(ld) else {
            return false; // one-sided: solid wall
        };
        let flags = self.lv.lines[ld].flags;
        if self.tm.flags & MF_MISSILE == 0 {
            if flags & crate::data::ML_BLOCKING != 0 {
                return false;
            }
            let is_player = self.mobjs[self.tm.thing as usize].is_player;
            if !is_player && flags & crate::data::ML_BLOCKMONSTERS != 0 {
                return false;
            }
        }
        if opentop < self.tm.ceilingz {
            self.tm.ceilingz = opentop;
            self.tm.ceilingline = ld as i32;
        }
        if openbottom > self.tm.floorz {
            self.tm.floorz = openbottom;
        }
        if lowfloor < self.tm.dropoffz {
            self.tm.dropoffz = lowfloor;
        }
        if self.lv.lines[ld].special != LineSpecial::None && self.tm.numspechit < self.tm.spechit.len() {
            self.tm.spechit[self.tm.numspechit] = ld as u16;
            self.tm.numspechit += 1;
        }
        true
    }

    fn pit_check_thing(&mut self, t: MRef) -> bool {
        let th = self.mobjs[t as usize];
        if th.flags & (MF_SOLID | MF_SPECIAL | MF_SHOOTABLE) == 0 {
            return true;
        }
        let me = self.mobjs[self.tm.thing as usize];
        let blockdist = th.radius + me.radius;
        if (th.x - self.tm.x).abs() >= blockdist || (th.y - self.tm.y).abs() >= blockdist {
            return true;
        }
        if t == self.tm.thing {
            return true;
        }
        if self.tm.flags & MF_MISSILE != 0 {
            if me.z > th.z + th.height || me.z + me.height < th.z {
                return true; // flew over or under
            }
            if me.target != NONE {
                let src = self.mobjs[me.target as usize];
                if src.kind == th.kind || (src.kind == ThingKind::Fiend && th.kind == ThingKind::Fiend) {
                    if t == me.target {
                        return true; // don't hit yourself
                    }
                    if !th.is_player {
                        return false; // same species: explode harmlessly
                    }
                }
            }
            if th.flags & MF_SHOOTABLE == 0 {
                return th.flags & MF_SOLID == 0;
            }
            let damage = ((self.rnd() % 8) + 1) * me.info().damage;
            let src = me.target;
            self.damage_mobj(t, self.tm.thing, src, damage);
            return false;
        }
        if th.flags & MF_SPECIAL != 0 {
            let solid = th.flags & MF_SOLID != 0;
            if self.tm.flags & MF_PICKUP != 0 {
                self.touch_special(t, self.tm.thing);
            }
            return !solid;
        }
        th.flags & MF_SOLID == 0
    }

    /// Attempt to move to (x, y); crosses special lines on success.
    pub fn try_move(&mut self, m: MRef, x: Fixed, y: Fixed) -> bool {
        self.tm.floatok = false;
        if !self.check_position(m, x, y) {
            return false;
        }
        let mo = self.mobjs[m as usize];
        if mo.flags & MF_NOCLIP == 0 {
            if self.tm.ceilingz - self.tm.floorz < mo.height {
                return false;
            }
            self.tm.floatok = true;
            if mo.flags & MF_TELEPORT == 0 && self.tm.ceilingz - mo.z < mo.height {
                return false;
            }
            if mo.flags & MF_TELEPORT == 0 && self.tm.floorz - mo.z > MAXSTEP {
                return false;
            }
            if mo.flags & (MF_DROPOFF | MF_FLOAT) == 0 && self.tm.floorz - self.tm.dropoffz > MAXSTEP {
                return false;
            }
        }
        self.unset_thing_position(m);
        let (oldx, oldy) = (mo.x, mo.y);
        {
            let mo = &mut self.mobjs[m as usize];
            mo.floorz = self.tm.floorz;
            mo.ceilingz = self.tm.ceilingz;
            mo.x = x;
            mo.y = y;
        }
        self.set_thing_position(m);
        if mo.flags & (MF_TELEPORT | MF_NOCLIP) == 0 {
            let hits = self.tm.spechit;
            let mut n = self.tm.numspechit;
            while n > 0 {
                n -= 1;
                let ld = hits[n] as usize;
                let side = self.lv.point_on_line_side(x, y, ld);
                let oldside = self.lv.point_on_line_side(oldx, oldy, ld);
                if side != oldside && self.lv.lines[ld].special != LineSpecial::None {
                    self.cross_special_line(ld, oldside, m);
                    if !self.mobjs[m as usize].in_use {
                        return true;
                    }
                }
            }
        }
        true
    }

    /// Re-check floor/ceiling for a thing whose sector moved. False if it no longer fits.
    pub fn thing_height_clip(&mut self, m: MRef) -> bool {
        let mo = self.mobjs[m as usize];
        let onfloor = mo.z == mo.floorz;
        self.check_position(m, mo.x, mo.y);
        let mo = &mut self.mobjs[m as usize];
        mo.floorz = self.tm.floorz;
        mo.ceilingz = self.tm.ceilingz;
        if onfloor {
            mo.z = mo.floorz;
        } else if mo.z + mo.height > mo.ceilingz {
            mo.z = mo.ceilingz - mo.height;
        }
        mo.ceilingz - mo.floorz >= mo.height
    }

    // ================================================================ sliding

    pub fn slide_move(&mut self, m: MRef) {
        self.tm.slidemo = m;
        let mut hitcount = 0;
        loop {
            hitcount += 1;
            let mo = self.mobjs[m as usize];
            if hitcount == 3 {
                self.stairstep(m);
                return;
            }
            let (leadx, trailx) = if mo.momx > 0 { (mo.x + mo.radius, mo.x - mo.radius) } else { (mo.x - mo.radius, mo.x + mo.radius) };
            let (leady, traily) = if mo.momy > 0 { (mo.y + mo.radius, mo.y - mo.radius) } else { (mo.y - mo.radius, mo.y + mo.radius) };
            self.tm.bestslidefrac = FRACUNIT + 1;
            self.tm.bestslideline = -1;
            self.path_traverse(leadx, leady, leadx + mo.momx, leady + mo.momy, PT_ADDLINES, Traverse::Slide);
            self.path_traverse(trailx, leady, trailx + mo.momx, leady + mo.momy, PT_ADDLINES, Traverse::Slide);
            self.path_traverse(leadx, traily, leadx + mo.momx, traily + mo.momy, PT_ADDLINES, Traverse::Slide);
            if self.tm.bestslidefrac == FRACUNIT + 1 {
                self.stairstep(m);
                return;
            }
            self.tm.bestslidefrac -= 0x800;
            if self.tm.bestslidefrac > 0 {
                let newx = fmul(mo.momx, self.tm.bestslidefrac);
                let newy = fmul(mo.momy, self.tm.bestslidefrac);
                if !self.try_move(m, mo.x + newx, mo.y + newy) {
                    self.stairstep(m);
                    return;
                }
            }
            let mut frac = FRACUNIT - (self.tm.bestslidefrac + 0x800);
            if frac > FRACUNIT {
                frac = FRACUNIT;
            }
            if frac <= 0 {
                return;
            }
            let mo = self.mobjs[m as usize];
            self.tm.xmove = fmul(mo.momx, frac);
            self.tm.ymove = fmul(mo.momy, frac);
            if self.tm.bestslideline >= 0 {
                self.hit_slide_line(self.tm.bestslideline as usize);
            }
            {
                let mo = &mut self.mobjs[m as usize];
                mo.momx = self.tm.xmove;
                mo.momy = self.tm.ymove;
            }
            let mo = self.mobjs[m as usize];
            if self.try_move(m, mo.x + self.tm.xmove, mo.y + self.tm.ymove) {
                return;
            }
        }
    }

    fn stairstep(&mut self, m: MRef) {
        let mo = self.mobjs[m as usize];
        if !self.try_move(m, mo.x, mo.y + mo.momy) {
            self.try_move(m, mo.x + mo.momx, mo.y);
        }
    }

    fn hit_slide_line(&mut self, ld: usize) {
        match self.lv.line_slope(ld) {
            Slope::Horizontal => {
                self.tm.ymove = 0;
                return;
            }
            Slope::Vertical => {
                self.tm.xmove = 0;
                return;
            }
            _ => {}
        }
        let mo = self.mobjs[self.tm.slidemo as usize];
        let side = self.lv.point_on_line_side(mo.x, mo.y, ld);
        let (dx, dy) = self.lv.line_dxdy(ld);
        let mut lineangle = point_to_angle(dx, dy);
        if side == 1 {
            lineangle = lineangle.wrapping_add(ANG180);
        }
        let moveangle = point_to_angle(self.tm.xmove, self.tm.ymove);
        let mut delta = moveangle.wrapping_sub(lineangle);
        if delta > ANG180 {
            delta = delta.wrapping_add(ANG180);
        }
        let movelen = approx_dist(self.tm.xmove, self.tm.ymove);
        let newlen = fmul(movelen, cos_a(delta));
        self.tm.xmove = fmul(newlen, cos_a(lineangle));
        self.tm.ymove = fmul(newlen, sin_a(lineangle));
    }

    fn ptr_slide(&mut self, ic: &Intercept) -> bool {
        if ic.line < 0 {
            return true;
        }
        let li = ic.line as usize;
        let mo = self.mobjs[self.tm.slidemo as usize];
        let blocking = match self.lv.line_opening(li) {
            None => {
                if self.lv.point_on_line_side(mo.x, mo.y, li) == 1 {
                    return true; // back side of a one-sided line
                }
                true
            }
            Some((opentop, openbottom, openrange, _)) => {
                openrange < mo.height || opentop - mo.z < mo.height || openbottom - mo.z > MAXSTEP
            }
        };
        if !blocking {
            return true;
        }
        if ic.frac < self.tm.bestslidefrac {
            self.tm.bestslidefrac = ic.frac;
            self.tm.bestslideline = li as i32;
        }
        false
    }

    // ================================================================ path traversal

    /// Walk the blockmap along a line, collecting line/thing intercepts,
    /// then visit them nearest first. Returns false if a visitor stopped it.
    pub fn path_traverse(&mut self, mut x1: Fixed, mut y1: Fixed, x2: Fixed, y2: Fixed, flags: u8, kind: Traverse) -> bool {
        self.lv.validcount = self.lv.validcount.wrapping_add(1);
        let bm = &self.lv.map.blockmap;
        let (ox, oy) = (fx(bm.x), fx(bm.y));
        // Avoid starting exactly on a block boundary.
        if ((x1 - ox) & (fx(128) - 1)) == 0 {
            x1 += FRACUNIT;
        }
        if ((y1 - oy) & (fx(128) - 1)) == 0 {
            y1 += FRACUNIT;
        }
        self.tm.dlx = x1;
        self.tm.dly = y1;
        self.tm.dx = x2 - x1;
        self.tm.dy = y2 - y1;
        let mut ics = [Intercept { frac: 0, line: -1, thing: NONE }; MAX_INTERCEPTS];
        let mut n = 0usize;

        // Grid walk (Amanatides & Woo) over 128-unit cells.
        let fx1 = (x1 - ox) as i64;
        let fy1 = (y1 - oy) as i64;
        let fx2 = (x2 - ox) as i64;
        let fy2 = (y2 - oy) as i64;
        let cell = 128i64 << 16;
        let (mut cx, mut cy) = (fx1.div_euclid(cell), fy1.div_euclid(cell));
        let (ex, ey) = (fx2.div_euclid(cell), fy2.div_euclid(cell));
        let (ddx, ddy) = (fx2 - fx1, fy2 - fy1);
        let stepx = if ddx > 0 { 1 } else if ddx < 0 { -1 } else { 0 };
        let stepy = if ddy > 0 { 1 } else if ddy < 0 { -1 } else { 0 };
        // t values scaled by 2^16 over the whole trace
        let tdelta = |d: i64| if d == 0 { i64::MAX } else { (cell << 16) / d.abs() };
        let tnext = |p: i64, c: i64, d: i64, step: i64| -> i64 {
            if d == 0 {
                return i64::MAX;
            }
            let boundary = if step > 0 { (c + 1) * cell } else { c * cell };
            ((boundary - p).abs() << 16) / d.abs()
        };
        let (tdx, tdy) = (tdelta(ddx), tdelta(ddy));
        let (mut tmx, mut tmy) = (tnext(fx1, cx, ddx, stepx), tnext(fy1, cy, ddy, stepy));
        let mut early = false;
        for _ in 0..256 {
            if flags & PT_ADDLINES != 0 && !self.add_line_intercepts(cx as i32, cy as i32, flags, &mut ics, &mut n) {
                early = true;
                break;
            }
            if flags & PT_ADDTHINGS != 0 {
                self.add_thing_intercepts(cx as i32, cy as i32, &mut ics, &mut n);
            }
            if cx == ex && cy == ey {
                break;
            }
            if tmx < tmy {
                cx += stepx;
                tmx = tmx.saturating_add(tdx);
            } else {
                cy += stepy;
                tmy = tmy.saturating_add(tdy);
            }
            if tmx.min(tmy) > (1 << 16) + 2 && !(cx == ex && cy == ey) {
                // Past the end of the trace; still visit the final cell.
                if (cx - ex).abs() + (cy - ey).abs() > 1 {
                    break;
                }
            }
        }
        let _ = early;
        // Visit nearest first.
        ics[..n].sort_unstable_by_key(|i| i.frac);
        for ic in ics[..n].iter() {
            if ic.frac > FRACUNIT {
                return true;
            }
            let go = match kind {
                Traverse::Slide => self.ptr_slide(ic),
                Traverse::Aim => self.ptr_aim(ic),
                Traverse::Shoot => self.ptr_shoot(ic),
                Traverse::Use => self.ptr_use(ic),
                Traverse::Sight => self.ptr_sight(ic),
            };
            if !go {
                return false;
            }
        }
        !early
    }

    fn add_line_intercepts(&mut self, bx: i32, by: i32, flags: u8, ics: &mut [Intercept; MAX_INTERCEPTS], n: &mut usize) -> bool {
        let Some(cell) = self.lv.block_index(bx, by) else { return true };
        let mut i = self.lv.map.blockmap.offsets[cell] as usize;
        loop {
            let li = self.lv.map.blockmap.lines[i];
            if li == 0xffff {
                return true;
            }
            i += 1;
            let ld = li as usize;
            if self.lv.lines[ld].validcount == self.lv.validcount {
                continue;
            }
            self.lv.lines[ld].validcount = self.lv.validcount;
            let l = self.lv.line(ld);
            let v1 = self.lv.vert(l.v1);
            let v2 = self.lv.vert(l.v2);
            let (tx, ty, tdx, tdy) = (self.tm.dlx, self.tm.dly, self.tm.dx, self.tm.dy);
            let s1 = point_on_side(v1.x, v1.y, tx, ty, tdx, tdy);
            let s2 = point_on_side(v2.x, v2.y, tx, ty, tdx, tdy);
            if s1 == s2 {
                continue;
            }
            let frac = intercept_vector(tx, ty, tdx, tdy, v1.x, v1.y, v2.x - v1.x, v2.y - v1.y);
            if frac < 0 {
                continue;
            }
            if flags & PT_EARLYOUT != 0 && frac < FRACUNIT && self.lv.back_sector(ld).is_none() {
                return false;
            }
            if *n < MAX_INTERCEPTS {
                ics[*n] = Intercept { frac, line: ld as i32, thing: NONE };
                *n += 1;
            }
        }
    }

    fn add_thing_intercepts(&mut self, bx: i32, by: i32, ics: &mut [Intercept; MAX_INTERCEPTS], n: &mut usize) {
        let Some(cell) = self.lv.block_index(bx, by) else { return };
        let mut m = self.lv.blocklinks[cell];
        let (tx, ty, tdx, tdy) = (self.tm.dlx, self.tm.dly, self.tm.dx, self.tm.dy);
        let positive = (tdx ^ tdy) > 0;
        let mut guard = 0;
        while m != NONE && guard < crate::limits::MAX_MOBJS {
            let t = self.mobjs[m as usize];
            // Use the thing's diagonal that crosses the trace.
            let (x1, y1, x2, y2) = if positive {
                (t.x - t.radius, t.y + t.radius, t.x + t.radius, t.y - t.radius)
            } else {
                (t.x - t.radius, t.y - t.radius, t.x + t.radius, t.y + t.radius)
            };
            let s1 = point_on_side(x1, y1, tx, ty, tdx, tdy);
            let s2 = point_on_side(x2, y2, tx, ty, tdx, tdy);
            if s1 != s2 {
                let frac = intercept_vector(tx, ty, tdx, tdy, x1, y1, x2 - x1, y2 - y1);
                if frac >= 0 && *n < MAX_INTERCEPTS {
                    ics[*n] = Intercept { frac, line: -1, thing: m };
                    *n += 1;
                }
            }
            m = t.bnext;
            guard += 1;
        }
    }

    // ================================================================ aiming & shooting

    /// Find a vertical slope that hits something along `angle`. Sets `tm.linetarget`.
    pub fn aim_line_attack(&mut self, t1: MRef, angle: Angle, distance: Fixed) -> Fixed {
        let mo = self.mobjs[t1 as usize];
        let x2 = mo.x + (distance >> FRACBITS) * (cos_a(angle) >> 0);
        let y2 = mo.y + (distance >> FRACBITS) * sin_a(angle);
        self.tm.shootthing = t1;
        self.tm.shootz = mo.z + (mo.height >> 1) + 8 * FRACUNIT;
        self.tm.topslope = 100 * FRACUNIT / 160;
        self.tm.bottomslope = -100 * FRACUNIT / 160;
        self.tm.attackrange = distance;
        self.tm.linetarget = NONE;
        self.path_traverse(mo.x, mo.y, x2, y2, PT_ADDLINES | PT_ADDTHINGS, Traverse::Aim);
        if self.tm.linetarget != NONE { self.tm.aimslope } else { 0 }
    }

    fn ptr_aim(&mut self, ic: &Intercept) -> bool {
        if ic.line >= 0 {
            let li = ic.line as usize;
            let Some((opentop, openbottom, _, _)) = self.lv.line_opening(li) else { return false };
            if openbottom >= opentop {
                return false;
            }
            let dist = fmul(self.tm.attackrange, ic.frac).max(1);
            let f = self.lv.sectors[self.lv.front_sector(li)];
            let b = self.lv.sectors[self.lv.back_sector(li).unwrap()];
            if f.floor != b.floor {
                let slope = fdiv(openbottom - self.tm.shootz, dist);
                if slope > self.tm.bottomslope {
                    self.tm.bottomslope = slope;
                }
            }
            if f.ceil != b.ceil {
                let slope = fdiv(opentop - self.tm.shootz, dist);
                if slope < self.tm.topslope {
                    self.tm.topslope = slope;
                }
            }
            return self.tm.topslope > self.tm.bottomslope;
        }
        let t = ic.thing;
        if t == self.tm.shootthing {
            return true;
        }
        let th = self.mobjs[t as usize];
        if th.flags & MF_SHOOTABLE == 0 {
            return true;
        }
        let dist = fmul(self.tm.attackrange, ic.frac).max(1);
        let mut top = fdiv(th.z + th.height - self.tm.shootz, dist);
        if top < self.tm.bottomslope {
            return true;
        }
        let mut bottom = fdiv(th.z - self.tm.shootz, dist);
        if bottom > self.tm.topslope {
            return true;
        }
        top = top.min(self.tm.topslope);
        bottom = bottom.max(self.tm.bottomslope);
        self.tm.aimslope = (top + bottom) / 2;
        self.tm.linetarget = t;
        false
    }

    /// Hitscan attack. Spawns puffs/blood and applies damage.
    pub fn line_attack(&mut self, t1: MRef, angle: Angle, distance: Fixed, slope: Fixed, damage: i32) {
        let mo = self.mobjs[t1 as usize];
        let x2 = mo.x + (distance >> FRACBITS) * cos_a(angle);
        let y2 = mo.y + (distance >> FRACBITS) * sin_a(angle);
        self.tm.shootthing = t1;
        self.tm.la_damage = damage;
        self.tm.shootz = mo.z + (mo.height >> 1) + 8 * FRACUNIT;
        self.tm.attackrange = distance;
        self.tm.aimslope = slope;
        self.path_traverse(mo.x, mo.y, x2, y2, PT_ADDLINES | PT_ADDTHINGS, Traverse::Shoot);
    }

    fn ptr_shoot(&mut self, ic: &Intercept) -> bool {
        let range = self.tm.attackrange;
        if ic.line >= 0 {
            let li = ic.line as usize;
            if self.lv.lines[li].special != LineSpecial::None {
                self.shoot_special_line(self.tm.shootthing, li);
            }
            let mut hit = true;
            if let Some((opentop, openbottom, _, _)) = self.lv.line_opening(li) {
                let dist = fmul(range, ic.frac).max(1);
                let f = self.lv.sectors[self.lv.front_sector(li)];
                let b = self.lv.sectors[self.lv.back_sector(li).unwrap()];
                hit = (f.floor != b.floor && fdiv(openbottom - self.tm.shootz, dist) > self.tm.aimslope)
                    || (f.ceil != b.ceil && fdiv(opentop - self.tm.shootz, dist) < self.tm.aimslope);
            }
            if !hit {
                return true;
            }
            let frac = ic.frac - fdiv(4 * FRACUNIT, range);
            let x = self.tm.dlx + fmul(self.tm.dx, frac);
            let y = self.tm.dly + fmul(self.tm.dy, frac);
            let z = self.tm.shootz + fmul(self.tm.aimslope, fmul(frac, range));
            let fs = self.lv.sectors[self.lv.front_sector(li)];
            if fs.ceilpic == crate::data::SKY_FLAT {
                if z > fs.ceil {
                    return false; // shooting into the sky
                }
                if let Some(b) = self.lv.back_sector(li) {
                    if self.lv.sectors[b].ceilpic == crate::data::SKY_FLAT {
                        return false;
                    }
                }
            }
            self.spawn_puff(x, y, z);
            return false;
        }
        let t = ic.thing;
        if t == self.tm.shootthing {
            return true;
        }
        let th = self.mobjs[t as usize];
        if th.flags & MF_SHOOTABLE == 0 {
            return true;
        }
        let dist = fmul(range, ic.frac).max(1);
        if fdiv(th.z + th.height - self.tm.shootz, dist) < self.tm.aimslope {
            return true;
        }
        if fdiv(th.z - self.tm.shootz, dist) > self.tm.aimslope {
            return true;
        }
        let frac = ic.frac - fdiv(10 * FRACUNIT, range);
        let x = self.tm.dlx + fmul(self.tm.dx, frac);
        let y = self.tm.dly + fmul(self.tm.dy, frac);
        let z = self.tm.shootz + fmul(self.tm.aimslope, fmul(frac, range));
        let dmg = self.tm.la_damage;
        if th.flags & MF_NOBLOOD != 0 {
            self.spawn_puff(x, y, z);
        } else {
            self.spawn_blood(x, y, z, dmg);
        }
        if dmg > 0 {
            let s = self.tm.shootthing;
            self.damage_mobj(t, s, s, dmg);
        }
        false
    }

    // ================================================================ use

    pub fn use_lines(&mut self, m: MRef) {
        let mo = self.mobjs[m as usize];
        self.tm.usething = m;
        let x2 = mo.x + (USERANGE >> FRACBITS) * cos_a(mo.angle);
        let y2 = mo.y + (USERANGE >> FRACBITS) * sin_a(mo.angle);
        self.path_traverse(mo.x, mo.y, x2, y2, PT_ADDLINES, Traverse::Use);
    }

    fn ptr_use(&mut self, ic: &Intercept) -> bool {
        if ic.line < 0 {
            return true;
        }
        let li = ic.line as usize;
        if self.lv.lines[li].special == LineSpecial::None {
            return match self.lv.line_opening(li) {
                Some((_, _, range, _)) if range > 0 => true,
                _ => false,
            };
        }
        let u = self.mobjs[self.tm.usething as usize];
        let side = self.lv.point_on_line_side(u.x, u.y, li);
        self.use_special_line(self.tm.usething, li, side);
        false
    }

    // ================================================================ sight

    pub fn check_sight(&mut self, t1: MRef, t2: MRef) -> bool {
        let a = self.mobjs[t1 as usize];
        let b = self.mobjs[t2 as usize];
        self.tm.sightzstart = a.z + a.height - (a.height >> 2);
        self.tm.topslope = (b.z + b.height) - self.tm.sightzstart;
        self.tm.bottomslope = b.z - self.tm.sightzstart;
        self.path_traverse(a.x, a.y, b.x, b.y, PT_EARLYOUT | PT_ADDLINES, Traverse::Sight)
    }

    fn ptr_sight(&mut self, ic: &Intercept) -> bool {
        let li = ic.line as usize;
        let Some((opentop, openbottom, _, _)) = self.lv.line_opening(li) else { return false };
        if openbottom >= opentop {
            return false;
        }
        let frac = ic.frac.max(1);
        let f = self.lv.sectors[self.lv.front_sector(li)];
        let b = self.lv.sectors[self.lv.back_sector(li).unwrap()];
        if f.floor != b.floor {
            let slope = fdiv(openbottom - self.tm.sightzstart, frac);
            if slope > self.tm.bottomslope {
                self.tm.bottomslope = slope;
            }
        }
        if f.ceil != b.ceil {
            let slope = fdiv(opentop - self.tm.sightzstart, frac);
            if slope < self.tm.topslope {
                self.tm.topslope = slope;
            }
        }
        self.tm.topslope > self.tm.bottomslope
    }

    // ================================================================ splash damage

    pub fn radius_attack(&mut self, spot: MRef, source: MRef, damage: i32) {
        let s = self.mobjs[spot as usize];
        let dist = fx(damage + 32);
        let bm = &self.lv.map.blockmap;
        let (ox, oy) = (bm.x, bm.y);
        let yh = blockshift(s.y + dist, oy);
        let yl = blockshift(s.y - dist, oy);
        let xh = blockshift(s.x + dist, ox);
        let xl = blockshift(s.x - dist, ox);
        self.tm.bombspot = spot;
        self.tm.bombsource = source;
        self.tm.bombdamage = damage;
        for y in yl..=yh {
            for x in xl..=xh {
                self.block_things(x, y, &mut |g, t| g.pit_radius_attack(t));
            }
        }
    }

    fn pit_radius_attack(&mut self, t: MRef) -> bool {
        let th = self.mobjs[t as usize];
        if th.flags & MF_SHOOTABLE == 0 {
            return true;
        }
        let spot = self.mobjs[self.tm.bombspot as usize];
        let dx = (th.x - spot.x).abs();
        let dy = (th.y - spot.y).abs();
        let dist = ((dx.max(dy) - th.radius) >> FRACBITS).max(0);
        if dist >= self.tm.bombdamage {
            return true;
        }
        if self.check_sight(t, self.tm.bombspot) {
            let (spot, src, dmg) = (self.tm.bombspot, self.tm.bombsource, self.tm.bombdamage);
            self.damage_mobj(t, spot, src, dmg - dist);
            self.tm.bombspot = spot;
            self.tm.bombsource = src;
            self.tm.bombdamage = dmg;
        }
        true
    }

    // ================================================================ teleport & crushing

    /// Move to (x, y) killing anything in the way (teleport destination).
    pub fn teleport_move(&mut self, m: MRef, x: Fixed, y: Fixed) -> bool {
        let mo = self.mobjs[m as usize];
        self.tm.thing = m;
        self.tm.flags = mo.flags;
        self.tm.x = x;
        self.tm.y = y;
        self.tm.bbox = [y + mo.radius, y - mo.radius, x - mo.radius, x + mo.radius];
        let sec = self.lv.sector_at(x, y);
        self.tm.ceilingline = -1;
        self.tm.floorz = self.lv.sectors[sec].floor;
        self.tm.dropoffz = self.tm.floorz;
        self.tm.ceilingz = self.lv.sectors[sec].ceil;
        self.lv.validcount = self.lv.validcount.wrapping_add(1);
        self.tm.numspechit = 0;
        let bm = &self.lv.map.blockmap;
        let (ox, oy) = (bm.x, bm.y);
        let b = self.tm.bbox;
        let xl = blockshift(b[BOXLEFT] - MAXRADIUS, ox);
        let xh = blockshift(b[BOXRIGHT] + MAXRADIUS, ox);
        let yl = blockshift(b[BOXBOTTOM] - MAXRADIUS, oy);
        let yh = blockshift(b[BOXTOP] + MAXRADIUS, oy);
        for bx in xl..=xh {
            for by in yl..=yh {
                if !self.block_things(bx, by, &mut |g, t| g.pit_stomp(t)) {
                    return false;
                }
            }
        }
        self.unset_thing_position(m);
        {
            let mo = &mut self.mobjs[m as usize];
            mo.floorz = self.tm.floorz;
            mo.ceilingz = self.tm.ceilingz;
            mo.x = x;
            mo.y = y;
        }
        self.set_thing_position(m);
        true
    }

    fn pit_stomp(&mut self, t: MRef) -> bool {
        let th = self.mobjs[t as usize];
        if th.flags & MF_SHOOTABLE == 0 || t == self.tm.thing {
            return true;
        }
        let me = self.mobjs[self.tm.thing as usize];
        let blockdist = th.radius + me.radius;
        if (th.x - self.tm.x).abs() >= blockdist || (th.y - self.tm.y).abs() >= blockdist {
            return true;
        }
        if !me.is_player {
            return false; // monsters don't telefrag
        }
        let s = self.tm.thing;
        self.damage_mobj(t, s, s, 10000);
        self.tm.thing = s;
        true
    }

    /// Adjust everything in a sector after its floor/ceiling moved.
    /// Returns true if something doesn't fit (the mover should stop/reverse).
    pub fn change_sector(&mut self, sec: usize, crunch: bool) -> bool {
        self.tm.nofit = false;
        self.tm.crushchange = crunch;
        let bb = self.lv.sectors[sec].blockbox;
        for x in bb[BOXLEFT] as i32..=bb[BOXRIGHT] as i32 {
            for y in bb[BOXBOTTOM] as i32..=bb[BOXTOP] as i32 {
                self.block_things(x, y, &mut |g, t| g.pit_change_sector(t));
            }
        }
        self.tm.nofit
    }

    fn pit_change_sector(&mut self, t: MRef) -> bool {
        if self.thing_height_clip(t) {
            return true;
        }
        let th = self.mobjs[t as usize];
        if th.health <= 0 && th.flags & MF_CORPSE != 0 {
            // Crushed corpses become gibs.
            if th.info().xdeath != S::Null || th.kind == ThingKind::Player1 {
                let mo = &mut self.mobjs[t as usize];
                mo.flags &= !MF_SOLID;
                mo.height = 0;
                mo.radius = 0;
            }
            return true;
        }
        if th.flags & MF_DROPPED != 0 {
            self.remove_mobj(t);
            return true;
        }
        if th.flags & MF_SHOOTABLE == 0 {
            return true;
        }
        self.tm.nofit = true;
        if self.tm.crushchange && self.leveltime & 3 == 0 {
            let nofit = self.tm.nofit;
            let crush = self.tm.crushchange;
            self.damage_mobj(t, NONE, NONE, 10);
            self.tm.nofit = nofit;
            self.tm.crushchange = crush;
            let (x, y, z, h) = (th.x, th.y, th.z, th.height);
            let b = self.spawn_mobj(x, y, z + h / 2, ThingKind::Blood);
            if b != NONE {
                let r1 = self.rnd_sub();
                let r2 = self.rnd_sub();
                let mo = &mut self.mobjs[b as usize];
                mo.momx = r1 << 12;
                mo.momy = r2 << 12;
            }
        }
        true
    }
}
