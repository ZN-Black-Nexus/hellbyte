//! World state and the per-tic world update.

use crate::data::*;
use crate::fixed::*;
use crate::info::*;
use crate::level::*;
use crate::limits::*;
use crate::mobj::*;
use crate::player::Player;
use crate::spec::{Button, LightFx, Mover, Scroller};

pub const TICRATE: u32 = 35;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Skill {
    Stroll,
    Skirmish,
    Brawl,
    Carnage,
    Inferno,
}

impl Skill {
    pub fn from_u8(v: u8) -> Skill {
        match v {
            0 => Skill::Stroll,
            1 => Skill::Skirmish,
            3 => Skill::Carnage,
            4 => Skill::Inferno,
            _ => Skill::Brawl,
        }
    }
}

/// Per-trace state (what the classic engine kept in globals).
pub struct Trace {
    pub thing: MRef,
    pub flags: u32,
    pub x: Fixed,
    pub y: Fixed,
    pub bbox: [Fixed; 4],
    pub floorz: Fixed,
    pub ceilingz: Fixed,
    pub dropoffz: Fixed,
    pub ceilingline: i32,
    pub spechit: [u16; 16],
    pub numspechit: usize,
    pub floatok: bool,
    // line attacks / aiming
    pub shootthing: MRef,
    pub shootz: Fixed,
    pub la_damage: i32,
    pub attackrange: Fixed,
    pub aimslope: Fixed,
    pub topslope: Fixed,
    pub bottomslope: Fixed,
    pub linetarget: MRef,
    // sliding
    pub bestslidefrac: Fixed,
    pub bestslideline: i32,
    pub slidemo: MRef,
    pub xmove: Fixed,
    pub ymove: Fixed,
    // sight
    pub sightzstart: Fixed,
    // use
    pub usething: MRef,
    // divline of the current path traversal
    pub dx: Fixed,
    pub dy: Fixed,
    pub dlx: Fixed,
    pub dly: Fixed,
    // radius attack / crushing
    pub bombspot: MRef,
    pub bombsource: MRef,
    pub bombdamage: i32,
    pub crushchange: bool,
    pub nofit: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Exit {
    None,
    Normal,
    Secret,
}

pub struct Game {
    pub lv: Level,
    pub mobjs: [Mobj; MAX_MOBJS],
    pub mobj_hint: usize,
    pub player: Player,
    pub movers: [Mover; MAX_MOVERS],
    pub lights: [LightFx; MAX_LIGHTS],
    pub buttons: [Button; MAX_BUTTONS],
    pub scrollers: [Scroller; MAX_SCROLLERS],
    pub tm: Trace,
    pub rng: u32,
    pub leveltime: u32,
    pub skill: Skill,
    pub totalkills: i32,
    pub totalitems: i32,
    pub totalsecret: i32,
    pub exit: Exit,
    /// Level to restart on player death (from the start of the level).
    pub map_index: usize,
    /// Set when the player walks through a line that ends the game.
    pub victory: bool,
    pub message: Option<&'static str>,
    pub message_tics: u32,
    /// Monsters can't wake each other up during the first tic.
    pub respawn_monsters: bool,
}

impl Game {
    /// Pseudo-random byte 0..=255 (deterministic; saved with the game).
    pub fn rnd(&mut self) -> i32 {
        self.rng ^= self.rng << 13;
        self.rng ^= self.rng >> 17;
        self.rng ^= self.rng << 5;
        ((self.rng >> 8) & 255) as i32
    }

    /// Difference of two random bytes, used for spreads.
    pub fn rnd_sub(&mut self) -> i32 {
        let a = self.rnd();
        let b = self.rnd();
        a - b
    }

    pub fn msg(&mut self, m: &'static str) {
        self.message = Some(m);
        self.message_tics = 4 * TICRATE;
    }

    #[inline]
    pub fn mo(&self, m: MRef) -> &Mobj {
        &self.mobjs[m as usize]
    }

    #[inline]
    pub fn mo_mut(&mut self, m: MRef) -> &mut Mobj {
        &mut self.mobjs[m as usize]
    }

    pub fn player_mo(&self) -> MRef {
        self.player.mo
    }

    // ---------------------------------------------------------------- level setup

    /// Load a level and populate it. Keeps the player's inventory unless `fresh`.
    pub fn setup_level(&mut self, index: usize, fresh: bool) {
        let index = index.min(MAPS.len() - 1);
        self.map_index = index;
        self.lv.init(index);
        for m in self.mobjs.iter_mut() {
            *m = Mobj::EMPTY;
        }
        for mv in self.movers.iter_mut() {
            *mv = Mover::EMPTY;
        }
        for l in self.lights.iter_mut() {
            *l = LightFx::EMPTY;
        }
        for b in self.buttons.iter_mut() {
            *b = Button::EMPTY;
        }
        for s in self.scrollers.iter_mut() {
            *s = Scroller::EMPTY;
        }
        self.mobj_hint = 0;
        self.leveltime = 0;
        self.totalkills = 0;
        self.totalitems = 0;
        self.totalsecret = 0;
        self.exit = Exit::None;
        self.message = None;
        if fresh {
            self.player.reset_inventory();
        }
        self.player.reset_level_stats();
        let skill_bit = match self.skill {
            Skill::Stroll | Skill::Skirmish => 1,
            Skill::Brawl => 2,
            _ => 4,
        };
        let map = self.lv.map.get();
        for t in map.things {
            if t.flags & 16 != 0 {
                continue; // multiplayer only
            }
            if t.kind != ThingKind::Player1 && t.flags & skill_bit == 0 {
                continue;
            }
            self.spawn_map_thing(t);
        }
        self.spawn_specials();
    }

    fn spawn_map_thing(&mut self, t: &ThingDef) {
        let x = fx(t.x as i32);
        let y = fx(t.y as i32);
        let angle = ((t.angle as u64 * ANG45 as u64) / 45) as Angle;
        if t.kind == ThingKind::Player1 {
            self.spawn_player(x, y, angle);
            return;
        }
        let info = info(t.kind);
        let z = if info.flags & MF_SPAWNCEILING != 0 { ONCEILINGZ } else { ONFLOORZ };
        let m = self.spawn_mobj(x, y, z, t.kind);
        if m == NONE {
            return;
        }
        {
            let mo = self.mo_mut(m);
            mo.angle = angle;
            mo.spawn = SpawnPoint { x: t.x, y: t.y, angle: t.angle, kind: t.kind as u8, flags: t.flags };
            if t.flags & 8 != 0 {
                mo.flags |= MF_AMBUSH;
            }
            if mo.tics > 0 {
                let r = (self.rng >> 3) as i16;
                let mo = self.mo_mut(m);
                mo.tics = 1 + r.rem_euclid(mo.tics);
            }
        }
        let flags = self.mo(m).flags;
        if flags & MF_COUNTKILL != 0 {
            self.totalkills += 1;
        }
        if flags & MF_COUNTITEM != 0 {
            self.totalitems += 1;
        }
    }

    fn spawn_player(&mut self, x: Fixed, y: Fixed, angle: Angle) {
        let m = self.spawn_mobj(x, y, ONFLOORZ, ThingKind::Player1);
        let p = &mut self.player;
        p.mo = m;
        p.reborn_mobj_state();
        let health = p.health;
        let mo = self.mo_mut(m);
        mo.angle = angle;
        mo.is_player = true;
        mo.health = health;
        self.player.viewheight = crate::player::VIEWHEIGHT;
        self.player.viewz = self.mo(m).z + crate::player::VIEWHEIGHT;
        self.setup_psprites();
    }

    // ---------------------------------------------------------------- world tic

    pub fn tick_world(&mut self) {
        self.player_think();
        // Mobjs think in index order; removal during the loop is fine.
        for i in 0..MAX_MOBJS {
            if self.mobjs[i].in_use && !self.mobjs[i].is_player {
                self.mobj_think(i as MRef);
            }
        }
        if self.mobjs[self.player.mo as usize].in_use {
            self.mobj_think(self.player.mo);
        }
        self.update_specials();
        if self.message_tics > 0 {
            self.message_tics -= 1;
            if self.message_tics == 0 {
                self.message = None;
            }
        }
        self.leveltime += 1;
    }

    pub fn count_kills(&self) -> (i32, i32, i32) {
        (self.player.killcount, self.player.itemcount, self.player.secretcount)
    }
}
