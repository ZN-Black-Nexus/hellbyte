//! Map objects ("mobjs"): the player, monsters, projectiles, items, effects.
//! They live in a fixed pool and refer to each other by index.

use crate::fixed::*;
use crate::info::*;
use crate::level::{MRef, NONE};

#[derive(Clone, Copy)]
pub struct SpawnPoint {
    pub x: i16,
    pub y: i16,
    pub angle: u16,
    pub kind: u8,
    pub flags: u8,
}

#[derive(Clone, Copy)]
pub struct Mobj {
    pub x: Fixed,
    pub y: Fixed,
    pub z: Fixed,
    pub momx: Fixed,
    pub momy: Fixed,
    pub momz: Fixed,
    pub angle: Angle,
    pub floorz: Fixed,
    pub ceilingz: Fixed,
    pub radius: Fixed,
    pub height: Fixed,
    pub flags: u32,
    pub health: i32,
    pub state: S,
    pub tics: i16,
    pub kind: ThingKind,
    pub sprite: Spr,
    pub frame: u8,
    pub movedir: u8,
    pub movecount: i16,
    pub reactiontime: i16,
    pub threshold: i16,
    pub target: MRef,
    pub tracer: MRef,
    pub subsector: u16,
    pub sector: u16,
    pub snext: MRef,
    pub sprev: MRef,
    pub bnext: MRef,
    pub bprev: MRef,
    pub spawn: SpawnPoint,
    pub lastlook: u8,
    pub is_player: bool,
    pub in_use: bool,
}

impl Mobj {
    pub const EMPTY: Mobj = Mobj {
        x: 0,
        y: 0,
        z: 0,
        momx: 0,
        momy: 0,
        momz: 0,
        angle: 0,
        floorz: 0,
        ceilingz: 0,
        radius: 0,
        height: 0,
        flags: 0,
        health: 0,
        state: S::Null,
        tics: 0,
        kind: ThingKind::Puff,
        sprite: Spr::None,
        frame: 0,
        movedir: 0,
        movecount: 0,
        reactiontime: 0,
        threshold: 0,
        target: NONE,
        tracer: NONE,
        subsector: 0,
        sector: 0,
        snext: NONE,
        sprev: NONE,
        bnext: NONE,
        bprev: NONE,
        spawn: SpawnPoint { x: 0, y: 0, angle: 0, kind: 0, flags: 0 },
        lastlook: 0,
        is_player: false,
        in_use: false,
    };

    pub fn info(&self) -> &'static MobjInfo {
        info(self.kind)
    }
}

// ====================================================================== behaviour

use crate::data::SKY_FLAT;
use crate::game::Game;
use crate::limits::MAX_MOBJS;

pub const ONFLOORZ: Fixed = i32::MIN;
pub const ONCEILINGZ: Fixed = i32::MAX;
pub const GRAVITY: Fixed = FRACUNIT;
pub const MAXMOVE: Fixed = 30 * FRACUNIT;
pub const STOPSPEED: Fixed = 0x1000;
pub const FRICTION: Fixed = 0xe800;
pub const FLOATSPEED: Fixed = 4 * FRACUNIT;

impl Game {
    fn alloc_mobj(&mut self) -> MRef {
        for k in 0..MAX_MOBJS {
            let i = (self.mobj_hint + k) % MAX_MOBJS;
            if !self.mobjs[i].in_use {
                self.mobj_hint = (i + 1) % MAX_MOBJS;
                return i as MRef;
            }
        }
        NONE
    }

    pub fn spawn_mobj(&mut self, x: Fixed, y: Fixed, z: Fixed, kind: ThingKind) -> MRef {
        let m = self.alloc_mobj();
        if m == NONE {
            return NONE;
        }
        let inf = info(kind);
        let st = state(inf.spawn);
        let lastlook = (self.rnd() & 1) as u8;
        let mo = &mut self.mobjs[m as usize];
        *mo = Mobj::EMPTY;
        mo.in_use = true;
        mo.kind = kind;
        mo.x = x;
        mo.y = y;
        mo.radius = inf.radius;
        mo.height = inf.height;
        mo.flags = inf.flags;
        mo.health = inf.health;
        mo.reactiontime = inf.reaction;
        mo.lastlook = lastlook;
        mo.state = inf.spawn;
        mo.tics = st.tics;
        mo.sprite = st.sprite;
        mo.frame = st.frame;
        if self.skill == crate::game::Skill::Inferno {
            mo.reactiontime = 0;
        }
        self.set_thing_position(m);
        let sec = self.lv.sectors[self.mo(m).sector as usize];
        let mo = &mut self.mobjs[m as usize];
        mo.floorz = sec.floor;
        mo.ceilingz = sec.ceil;
        mo.z = if z == ONFLOORZ {
            sec.floor
        } else if z == ONCEILINGZ {
            sec.ceil - mo.height
        } else {
            z
        };
        m
    }

    pub fn remove_mobj(&mut self, m: MRef) {
        if m == NONE || !self.mobjs[m as usize].in_use {
            return;
        }
        self.unset_thing_position(m);
        self.mobjs[m as usize].in_use = false;
        // Nothing may keep pointing at a freed slot.
        for o in self.mobjs.iter_mut() {
            if o.target == m {
                o.target = NONE;
            }
            if o.tracer == m {
                o.tracer = NONE;
            }
        }
        if self.player.attacker == m {
            self.player.attacker = NONE;
        }
        for s in self.lv.sectors.iter_mut() {
            if s.soundtarget == m {
                s.soundtarget = NONE;
            }
        }
    }

    /// Enter state `s`, running its action. Zero-tic states chain immediately.
    /// Returns false if the mobj was removed.
    pub fn set_state(&mut self, m: MRef, mut s: S) -> bool {
        let mut guard = 0;
        loop {
            if s == S::Null {
                self.mobjs[m as usize].state = S::Null;
                self.remove_mobj(m);
                return false;
            }
            let st = state(s);
            {
                let mo = &mut self.mobjs[m as usize];
                mo.state = s;
                mo.tics = st.tics;
                mo.sprite = st.sprite;
                mo.frame = st.frame;
            }
            if st.action != A::None {
                self.call_action(m, st.action);
                if !self.mobjs[m as usize].in_use {
                    return false;
                }
            }
            s = st.next;
            guard += 1;
            if self.mobjs[m as usize].tics != 0 || guard > 32 {
                return true;
            }
        }
    }

    fn call_action(&mut self, m: MRef, a: A) {
        match a {
            A::Look => self.a_look(m),
            A::Chase => self.a_chase(m),
            A::FaceTarget => self.a_face_target(m),
            A::DroneShoot => self.a_drone_shoot(m),
            A::EnforcerShoot => self.a_enforcer_shoot(m),
            A::HeavyShoot => self.a_drone_shoot(m),
            A::HeavyRefire => self.a_heavy_refire(m),
            A::FiendAttack => self.a_fiend_attack(m),
            A::RipperBite => self.a_ripper_bite(m),
            A::GazerAttack => self.a_gazer_attack(m),
            A::JuggAttack => self.a_jugg_attack(m),
            A::Fall => self.mobjs[m as usize].flags &= !MF_SOLID,
            A::Explode => {
                let src = self.mo(m).target;
                self.radius_attack(m, src, 128);
            }
            A::BossDeath => self.a_boss_death(m),
            A::ArcSpray => self.a_arc_spray(m),
            _ => {}
        }
    }

    /// Link into the sector thing list and the blockmap.
    pub fn set_thing_position(&mut self, m: MRef) {
        let (x, y, flags) = {
            let mo = &self.mobjs[m as usize];
            (mo.x, mo.y, mo.flags)
        };
        let ss = self.lv.subsector_at(x, y);
        let sec = self.lv.map.subsectors[ss].sector;
        {
            let mo = &mut self.mobjs[m as usize];
            mo.subsector = ss as u16;
            mo.sector = sec;
        }
        if flags & MF_NOSECTOR == 0 {
            let head = self.lv.sectors[sec as usize].thinglist;
            let mo = &mut self.mobjs[m as usize];
            mo.sprev = NONE;
            mo.snext = head;
            if head != NONE {
                self.mobjs[head as usize].sprev = m;
            }
            self.lv.sectors[sec as usize].thinglist = m;
        }
        if flags & MF_NOBLOCKMAP == 0 {
            let (bx, by) = self.lv.block_coords(x, y);
            match self.lv.block_index(bx, by) {
                Some(b) => {
                    let head = self.lv.blocklinks[b];
                    let mo = &mut self.mobjs[m as usize];
                    mo.bprev = NONE;
                    mo.bnext = head;
                    if head != NONE {
                        self.mobjs[head as usize].bprev = m;
                    }
                    self.lv.blocklinks[b] = m;
                }
                None => {
                    let mo = &mut self.mobjs[m as usize];
                    mo.bnext = NONE;
                    mo.bprev = NONE;
                }
            }
        }
    }

    pub fn unset_thing_position(&mut self, m: MRef) {
        let mo = self.mobjs[m as usize];
        if mo.flags & MF_NOSECTOR == 0 {
            if mo.snext != NONE {
                self.mobjs[mo.snext as usize].sprev = mo.sprev;
            }
            if mo.sprev != NONE {
                self.mobjs[mo.sprev as usize].snext = mo.snext;
            } else if self.lv.sectors[mo.sector as usize].thinglist == m {
                self.lv.sectors[mo.sector as usize].thinglist = mo.snext;
            }
        }
        if mo.flags & MF_NOBLOCKMAP == 0 {
            if mo.bnext != NONE {
                self.mobjs[mo.bnext as usize].bprev = mo.bprev;
            }
            if mo.bprev != NONE {
                self.mobjs[mo.bprev as usize].bnext = mo.bnext;
            } else {
                let (bx, by) = self.lv.block_coords(mo.x, mo.y);
                if let Some(b) = self.lv.block_index(bx, by) {
                    if self.lv.blocklinks[b] == m {
                        self.lv.blocklinks[b] = mo.bnext;
                    }
                }
            }
        }
        let mo = &mut self.mobjs[m as usize];
        mo.snext = NONE;
        mo.sprev = NONE;
        mo.bnext = NONE;
        mo.bprev = NONE;
    }

    pub fn mobj_think(&mut self, m: MRef) {
        let mo = self.mobjs[m as usize];
        if mo.momx != 0 || mo.momy != 0 {
            self.xy_movement(m);
            if !self.mobjs[m as usize].in_use {
                return;
            }
        }
        let mo = self.mobjs[m as usize];
        if mo.z != mo.floorz || mo.momz != 0 {
            self.z_movement(m);
            if !self.mobjs[m as usize].in_use {
                return;
            }
        }
        let mo = &mut self.mobjs[m as usize];
        if mo.tics != -1 {
            mo.tics -= 1;
            if mo.tics <= 0 {
                let next = state(mo.state).next;
                self.set_state(m, next);
            }
        }
    }

    fn xy_movement(&mut self, m: MRef) {
        {
            let mo = &mut self.mobjs[m as usize];
            mo.momx = mo.momx.clamp(-MAXMOVE, MAXMOVE);
            mo.momy = mo.momy.clamp(-MAXMOVE, MAXMOVE);
        }
        let (mut xmove, mut ymove) = (self.mo(m).momx, self.mo(m).momy);
        loop {
            let mo = self.mobjs[m as usize];
            let (tx, ty);
            if xmove > MAXMOVE / 2 || ymove > MAXMOVE / 2 || xmove < -MAXMOVE / 2 || ymove < -MAXMOVE / 2 {
                tx = mo.x + xmove / 2;
                ty = mo.y + ymove / 2;
                xmove >>= 1;
                ymove >>= 1;
            } else {
                tx = mo.x + xmove;
                ty = mo.y + ymove;
                xmove = 0;
                ymove = 0;
            }
            if !self.try_move(m, tx, ty) {
                let mo = self.mobjs[m as usize];
                if mo.is_player {
                    self.slide_move(m);
                } else if mo.flags & MF_MISSILE != 0 {
                    // Missiles flying into the sky just vanish.
                    if self.tm.ceilingline >= 0 {
                        if let Some(b) = self.lv.back_sector(self.tm.ceilingline as usize) {
                            if self.lv.sectors[b].ceilpic == SKY_FLAT {
                                self.remove_mobj(m);
                                return;
                            }
                        }
                    }
                    self.explode_missile(m);
                    return;
                } else {
                    let mo = &mut self.mobjs[m as usize];
                    mo.momx = 0;
                    mo.momy = 0;
                }
            }
            if !self.mobjs[m as usize].in_use {
                return;
            }
            if xmove == 0 && ymove == 0 {
                break;
            }
        }
        let mo = self.mobjs[m as usize];
        if mo.flags & MF_MISSILE != 0 || mo.z > mo.floorz {
            return;
        }
        if mo.flags & MF_CORPSE != 0
            && (mo.momx > FRACUNIT / 4 || mo.momx < -FRACUNIT / 4 || mo.momy > FRACUNIT / 4 || mo.momy < -FRACUNIT / 4)
            && mo.floorz != self.lv.sectors[mo.sector as usize].floor
        {
            return; // corpses slide off ledges
        }
        let idle = mo.is_player && self.player.cmd.forwardmove == 0 && self.player.cmd.sidemove == 0;
        let mo = &mut self.mobjs[m as usize];
        if mo.momx > -STOPSPEED && mo.momx < STOPSPEED && mo.momy > -STOPSPEED && mo.momy < STOPSPEED && (!mo.is_player || idle) {
            if mo.is_player && matches!(mo.state, S::PlayRun1 | S::PlayRun2 | S::PlayRun3 | S::PlayRun4) {
                self.set_state(m, S::PlayStand);
            }
            let mo = &mut self.mobjs[m as usize];
            mo.momx = 0;
            mo.momy = 0;
        } else {
            mo.momx = fmul(mo.momx, FRICTION);
            mo.momy = fmul(mo.momy, FRICTION);
        }
    }

    fn z_movement(&mut self, m: MRef) {
        let is_player = self.mo(m).is_player;
        if is_player {
            let mo = self.mobjs[m as usize];
            if mo.z < mo.floorz {
                // Smooth the view when stepping up.
                self.player.viewheight -= mo.floorz - mo.z;
                self.player.deltaviewheight = (crate::player::VIEWHEIGHT - self.player.viewheight) >> 3;
            }
        }
        {
            let mo = &mut self.mobjs[m as usize];
            mo.z += mo.momz;
        }
        let mo = self.mobjs[m as usize];
        if mo.flags & MF_FLOAT != 0 && mo.target != NONE && mo.flags & MF_INFLOAT == 0 {
            let t = self.mobjs[mo.target as usize];
            let dist = approx_dist(mo.x - t.x, mo.y - t.y);
            let delta = (t.z + (mo.height >> 1)) - mo.z;
            let mo = &mut self.mobjs[m as usize];
            if delta < 0 && dist < -(delta * 3) {
                mo.z -= FLOATSPEED;
            } else if delta > 0 && dist < delta * 3 {
                mo.z += FLOATSPEED;
            }
        }
        let mo = self.mobjs[m as usize];
        if mo.z <= mo.floorz {
            if mo.momz < 0 {
                if is_player && mo.momz < -GRAVITY * 8 {
                    // Landing hard squashes the view a little.
                    self.player.deltaviewheight = mo.momz >> 3;
                }
                self.mobjs[m as usize].momz = 0;
            }
            self.mobjs[m as usize].z = mo.floorz;
            if mo.flags & MF_MISSILE != 0 && mo.flags & MF_NOCLIP == 0 {
                self.explode_missile(m);
                return;
            }
        } else if mo.flags & MF_NOGRAVITY == 0 {
            let mo = &mut self.mobjs[m as usize];
            if mo.momz == 0 {
                mo.momz = -GRAVITY * 2;
            } else {
                mo.momz -= GRAVITY;
            }
        }
        let mo = self.mobjs[m as usize];
        if mo.z + mo.height > mo.ceilingz {
            let mo = &mut self.mobjs[m as usize];
            if mo.momz > 0 {
                mo.momz = 0;
            }
            mo.z = mo.ceilingz - mo.height;
            if mo.flags & MF_MISSILE != 0 && mo.flags & MF_NOCLIP == 0 {
                if self.lv.sectors[mo.sector as usize].ceilpic == SKY_FLAT {
                    self.remove_mobj(m);
                } else {
                    self.explode_missile(m);
                }
            }
        }
    }

    pub fn explode_missile(&mut self, m: MRef) {
        let death = self.mo(m).info().death;
        {
            let mo = &mut self.mobjs[m as usize];
            mo.momx = 0;
            mo.momy = 0;
            mo.momz = 0;
        }
        if !self.set_state(m, death) {
            return;
        }
        let r = (self.rnd() & 3) as i16;
        let mo = &mut self.mobjs[m as usize];
        mo.tics = (mo.tics - r).max(1);
        mo.flags &= !MF_MISSILE;
    }

    pub fn spawn_puff(&mut self, x: Fixed, y: Fixed, z: Fixed) {
        let z = z + (self.rnd_sub() << 10);
        let th = self.spawn_mobj(x, y, z, ThingKind::Puff);
        if th == NONE {
            return;
        }
        let r = (self.rnd() & 3) as i16;
        let mo = &mut self.mobjs[th as usize];
        mo.momz = FRACUNIT;
        mo.tics = (mo.tics - r).max(1);
        if self.tm.attackrange == crate::map::MELEERANGE {
            self.set_state(th, S::Puff3);
        }
    }

    pub fn spawn_blood(&mut self, x: Fixed, y: Fixed, z: Fixed, damage: i32) {
        let z = z + (self.rnd_sub() << 10);
        let th = self.spawn_mobj(x, y, z, ThingKind::Blood);
        if th == NONE {
            return;
        }
        let r = (self.rnd() & 3) as i16;
        let mo = &mut self.mobjs[th as usize];
        mo.momz = FRACUNIT * 2;
        mo.tics = (mo.tics - r).max(1);
        if damage <= 12 && damage >= 9 {
            self.set_state(th, S::Blood2);
        } else if damage < 9 {
            self.set_state(th, S::Blood3);
        }
    }

    /// Launch a monster projectile at `dest`.
    pub fn spawn_missile(&mut self, source: MRef, dest: MRef, kind: ThingKind) -> MRef {
        let s = self.mobjs[source as usize];
        let th = self.spawn_mobj(s.x, s.y, s.z + 32 * FRACUNIT, kind);
        if th == NONE || dest == NONE {
            return th;
        }
        let d = self.mobjs[dest as usize];
        let mut an = point_to_angle(d.x - s.x, d.y - s.y);
        if d.flags & MF_SHADOW != 0 {
            an = an.wrapping_add((self.rnd_sub() << 20) as u32);
        }
        let speed = info(kind).speed;
        let dist = (approx_dist(d.x - s.x, d.y - s.y) / speed.max(1)).max(1);
        {
            let mo = &mut self.mobjs[th as usize];
            mo.target = source;
            mo.angle = an;
            mo.momx = fmul(speed, cos_a(an));
            mo.momy = fmul(speed, sin_a(an));
            mo.momz = (d.z - s.z) / dist;
        }
        self.check_missile_spawn(th);
        th
    }

    pub fn check_missile_spawn(&mut self, th: MRef) {
        let r = (self.rnd() & 3) as i16;
        let (x, y) = {
            let mo = &mut self.mobjs[th as usize];
            mo.tics = (mo.tics - r).max(1);
            // Nudge forward so an immediate explosion still has a direction.
            mo.x += mo.momx >> 1;
            mo.y += mo.momy >> 1;
            mo.z += mo.momz >> 1;
            (mo.x, mo.y)
        };
        if !self.try_move(th, x, y) {
            self.explode_missile(th);
        }
    }

    /// Player projectile with vertical auto-aim.
    pub fn spawn_player_missile(&mut self, source: MRef, kind: ThingKind) {
        let s = self.mobjs[source as usize];
        let mut an = s.angle;
        let range = 16 * 64 * FRACUNIT;
        let mut slope = self.aim_line_attack(source, an, range);
        if self.tm.linetarget == NONE {
            an = an.wrapping_add(1 << 26);
            slope = self.aim_line_attack(source, an, range);
            if self.tm.linetarget == NONE {
                an = an.wrapping_sub(2 << 26);
                slope = self.aim_line_attack(source, an, range);
            }
            if self.tm.linetarget == NONE {
                an = s.angle;
                slope = 0;
            }
        }
        let th = self.spawn_mobj(s.x, s.y, s.z + 32 * FRACUNIT, kind);
        if th == NONE {
            return;
        }
        let speed = info(kind).speed;
        {
            let mo = &mut self.mobjs[th as usize];
            mo.target = source;
            mo.angle = an;
            mo.momx = fmul(speed, cos_a(an));
            mo.momy = fmul(speed, sin_a(an));
            mo.momz = fmul(speed, slope);
        }
        self.check_missile_spawn(th);
    }
}
