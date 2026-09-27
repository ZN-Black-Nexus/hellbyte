//! The player: movement, view, weapons, pickups — plus damage and death
//! for every shootable thing.

use crate::fixed::*;
use crate::game::{Game, Skill};
use crate::info::*;
use crate::level::*;
use crate::map::{MELEERANGE, MISSILERANGE};
use crate::render::PSpriteView;

pub const VIEWHEIGHT: Fixed = 41 * FRACUNIT;
pub const MAXHEALTH: i32 = 100;
const MAXBOB: Fixed = 16 * FRACUNIT;
const WEAPONTOP: Fixed = 0;
const WEAPONBOTTOM: Fixed = 100 * FRACUNIT;
const RAISESPEED: Fixed = 6 * FRACUNIT;
const LOWERSPEED: Fixed = 6 * FRACUNIT;
const BONUSADD: i32 = 6;

pub const CARD_RED: usize = 0;
pub const CARD_BLUE: usize = 1;
pub const CARD_YELLOW: usize = 2;

pub const PW_INVULN: usize = 0;
pub const PW_STRENGTH: usize = 1;
pub const PW_IRONFEET: usize = 2;
pub const PW_ALLMAP: usize = 3;
pub const NUMPOWERS: usize = 4;
const INVULNTICS: i32 = 30 * 35;
const IRONTICS: i32 = 60 * 35;

pub const CHEAT_GOD: u32 = 1;
pub const CHEAT_NOCLIP: u32 = 2;

pub const BT_ATTACK: u8 = 1;
pub const BT_USE: u8 = 2;
pub const BT_CHANGE: u8 = 4;
pub const BT_WEAPONSHIFT: u8 = 3;
pub const BT_WEAPONMASK: u8 = 7 << 3;

pub const AM_BULLETS: usize = 0;
pub const AM_SHELLS: usize = 1;
pub const AM_CELLS: usize = 2;
pub const AM_ROCKETS: usize = 3;
pub const AM_NONE: usize = 4;
const CLIPAMMO: [i32; 4] = [10, 4, 20, 1];
pub const MAXAMMO: [i32; 4] = [200, 50, 300, 50];

pub const NUMWEAPONS: usize = 8;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Weapon {
    Fist,
    Pistol,
    Shotgun,
    Chaingun,
    Launcher,
    Plasma,
    Arc,
    Drill,
}

impl Weapon {
    pub fn from_u8(v: u8) -> Option<Weapon> {
        Some(match v {
            0 => Weapon::Fist,
            1 => Weapon::Pistol,
            2 => Weapon::Shotgun,
            3 => Weapon::Chaingun,
            4 => Weapon::Launcher,
            5 => Weapon::Plasma,
            6 => Weapon::Arc,
            7 => Weapon::Drill,
            _ => return None,
        })
    }
}

pub struct WeaponInfo {
    pub ammo: usize,
    pub up: S,
    pub down: S,
    pub ready: S,
    pub attack: S,
    pub flash: S,
    pub per_shot: i32,
}

pub static WEAPONINFO: [WeaponInfo; NUMWEAPONS] = [
    WeaponInfo { ammo: AM_NONE, up: S::FistUp, down: S::FistDown, ready: S::FistReady, attack: S::FistAtk1, flash: S::Null, per_shot: 0 },
    WeaponInfo {
        ammo: AM_BULLETS,
        up: S::PistolUp,
        down: S::PistolDown,
        ready: S::PistolReady,
        attack: S::PistolAtk1,
        flash: S::PistolFlash,
        per_shot: 1,
    },
    WeaponInfo { ammo: AM_SHELLS, up: S::SgUp, down: S::SgDown, ready: S::SgReady, attack: S::SgAtk1, flash: S::SgFlash1, per_shot: 1 },
    WeaponInfo { ammo: AM_BULLETS, up: S::CgUp, down: S::CgDown, ready: S::CgReady, attack: S::CgAtk1, flash: S::CgFlash1, per_shot: 1 },
    WeaponInfo { ammo: AM_ROCKETS, up: S::RlUp, down: S::RlDown, ready: S::RlReady, attack: S::RlAtk1, flash: S::RlFlash1, per_shot: 1 },
    WeaponInfo { ammo: AM_CELLS, up: S::PlUp, down: S::PlDown, ready: S::PlReady, attack: S::PlAtk1, flash: S::PlFlash1, per_shot: 1 },
    WeaponInfo { ammo: AM_CELLS, up: S::ArcUp, down: S::ArcDown, ready: S::ArcReady, attack: S::ArcAtk1, flash: S::ArcFlash1, per_shot: 40 },
    WeaponInfo {
        ammo: AM_NONE,
        up: S::DrillUp,
        down: S::DrillDown,
        ready: S::DrillReady,
        attack: S::DrillAtk1,
        flash: S::Null,
        per_shot: 0,
    },
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum PState {
    Live,
    Dead,
    Reborn,
}

#[derive(Clone, Copy, Default, Debug)]
pub struct TicCmd {
    pub forwardmove: i8,
    pub sidemove: i8,
    pub angleturn: i16,
    pub buttons: u8,
}

#[derive(Clone, Copy)]
pub struct PSprite {
    pub state: S,
    pub tics: i16,
    pub sx: Fixed,
    pub sy: Fixed,
}

pub const PS_WEAPON: usize = 0;
pub const PS_FLASH: usize = 1;

pub struct Player {
    pub mo: MRef,
    pub state: PState,
    pub cmd: TicCmd,
    pub viewz: Fixed,
    pub viewheight: Fixed,
    pub deltaviewheight: Fixed,
    pub bob: Fixed,
    pub health: i32,
    pub armorpoints: i32,
    pub armortype: u8,
    pub powers: [i32; NUMPOWERS],
    pub cards: [bool; 3],
    pub backpack: bool,
    pub weaponowned: [bool; NUMWEAPONS],
    pub ammo: [i32; 4],
    pub maxammo: [i32; 4],
    pub readyweapon: Weapon,
    pub pendingweapon: Option<Weapon>,
    pub attackdown: bool,
    pub usedown: bool,
    pub refire: i32,
    pub killcount: i32,
    pub itemcount: i32,
    pub secretcount: i32,
    pub damagecount: i32,
    pub bonuscount: i32,
    pub attacker: MRef,
    pub extralight: i32,
    pub fixedcolormap: i32,
    pub psprites: [PSprite; 2],
    pub cheats: u32,
}

impl Player {
    pub fn reset_inventory(&mut self) {
        self.health = MAXHEALTH;
        self.armorpoints = 0;
        self.armortype = 0;
        self.backpack = false;
        self.weaponowned = [false; NUMWEAPONS];
        self.weaponowned[Weapon::Fist as usize] = true;
        self.weaponowned[Weapon::Pistol as usize] = true;
        self.ammo = [50, 0, 0, 0];
        self.maxammo = MAXAMMO;
        self.readyweapon = Weapon::Pistol;
        self.pendingweapon = None;
    }

    pub fn reset_level_stats(&mut self) {
        self.killcount = 0;
        self.itemcount = 0;
        self.secretcount = 0;
        self.cards = [false; 3];
        self.powers = [0; NUMPOWERS];
        self.damagecount = 0;
        self.bonuscount = 0;
        self.extralight = 0;
        self.fixedcolormap = -1;
        self.attacker = NONE;
        self.refire = 0;
    }

    pub fn reborn_mobj_state(&mut self) {
        self.state = PState::Live;
        self.viewheight = VIEWHEIGHT;
        self.deltaviewheight = 0;
        self.attackdown = false;
        self.usedown = false;
        self.pendingweapon = None;
    }

    pub fn psprite_views(&self, leveltime: u32) -> [Option<PSpriteView>; 2] {
        let mut out = [None, None];
        for (i, p) in self.psprites.iter().enumerate() {
            if p.state != S::Null {
                let st = state(p.state);
                out[i] = Some(PSpriteView { sprite: st.sprite, frame: st.frame, sx: p.sx, sy: p.sy, anim: (leveltime & 0xffff) as u16 });
            }
        }
        out
    }
}

impl Game {
    // ================================================================ per-tic player logic

    pub fn player_think(&mut self) {
        let m = self.player.mo;
        if m == NONE || !self.mobjs[m as usize].in_use {
            return;
        }
        if self.player.cheats & CHEAT_NOCLIP != 0 {
            self.mobjs[m as usize].flags |= MF_NOCLIP;
        } else {
            self.mobjs[m as usize].flags &= !MF_NOCLIP;
        }
        if self.mobjs[m as usize].flags & MF_JUSTATTACKED != 0 {
            // The drill pulls you along towards what it's chewing.
            self.player.cmd.angleturn = 0;
            self.player.cmd.forwardmove = 100;
            self.player.cmd.sidemove = 0;
            self.mobjs[m as usize].flags &= !MF_JUSTATTACKED;
        }
        if self.player.state == PState::Dead {
            self.death_think();
            return;
        }
        if self.mobjs[m as usize].reactiontime > 0 {
            self.mobjs[m as usize].reactiontime -= 1;
        } else {
            self.move_player();
        }
        self.calc_height();
        if self.lv.sectors[self.mobjs[m as usize].sector as usize].special != SectorSpecial::None {
            self.player_in_special_sector();
        }
        let cmd = self.player.cmd;
        if cmd.buttons & BT_CHANGE != 0 {
            let mut w = Weapon::from_u8((cmd.buttons & BT_WEAPONMASK) >> BT_WEAPONSHIFT).unwrap_or(Weapon::Fist);
            if w == Weapon::Fist
                && self.player.weaponowned[Weapon::Drill as usize]
                && !(self.player.readyweapon == Weapon::Drill && self.player.powers[PW_STRENGTH] > 0)
            {
                w = Weapon::Drill;
            }
            if self.player.weaponowned[w as usize] && w != self.player.readyweapon {
                self.player.pendingweapon = Some(w);
            }
        }
        if cmd.buttons & BT_USE != 0 {
            if !self.player.usedown {
                self.use_lines(m);
                self.player.usedown = true;
            }
        } else {
            self.player.usedown = false;
        }
        self.move_psprites();
        let p = &mut self.player;
        if p.powers[PW_STRENGTH] > 0 {
            p.powers[PW_STRENGTH] += 1;
        }
        if p.powers[PW_INVULN] > 0 {
            p.powers[PW_INVULN] -= 1;
        }
        if p.powers[PW_IRONFEET] > 0 {
            p.powers[PW_IRONFEET] -= 1;
        }
        if p.damagecount > 0 {
            p.damagecount -= 1;
        }
        if p.bonuscount > 0 {
            p.bonuscount -= 1;
        }
        p.fixedcolormap = if p.powers[PW_INVULN] > 0 {
            if p.powers[PW_INVULN] > 4 * 32 || p.powers[PW_INVULN] & 8 != 0 { crate::data::NUM_COLORMAPS as i32 } else { -1 }
        } else {
            -1
        };
    }

    fn thrust(&mut self, angle: Angle, mv: Fixed) {
        let mo = &mut self.mobjs[self.player.mo as usize];
        mo.momx += fmul(mv, cos_a(angle));
        mo.momy += fmul(mv, sin_a(angle));
    }

    fn move_player(&mut self) {
        let m = self.player.mo;
        let cmd = self.player.cmd;
        {
            let mo = &mut self.mobjs[m as usize];
            mo.angle = mo.angle.wrapping_add((cmd.angleturn as i32 as u32) << 16);
        }
        let mo = self.mobjs[m as usize];
        let onground = mo.z <= mo.floorz;
        if cmd.forwardmove != 0 && onground {
            self.thrust(mo.angle, cmd.forwardmove as i32 * 2048);
        }
        if cmd.sidemove != 0 && onground {
            self.thrust(mo.angle.wrapping_sub(ANG90), cmd.sidemove as i32 * 2048);
        }
        if (cmd.forwardmove != 0 || cmd.sidemove != 0) && self.mobjs[m as usize].state == S::PlayStand {
            self.set_state(m, S::PlayRun1);
        }
    }

    fn calc_height(&mut self) {
        let m = self.player.mo;
        let mo = self.mobjs[m as usize];
        let onground = mo.z <= mo.floorz;
        let mut bob = (fmul(mo.momx, mo.momx) + fmul(mo.momy, mo.momy)) >> 2;
        if bob > MAXBOB {
            bob = MAXBOB;
        }
        self.player.bob = bob;
        if !onground {
            self.player.viewz = (mo.z + VIEWHEIGHT).min(mo.ceilingz - 4 * FRACUNIT);
            return;
        }
        let angle = (FINEANGLES / 20 * self.leveltime as usize) & FINEMASK;
        let bobv = fmul(bob / 2, finesine(angle));
        let p = &mut self.player;
        if p.state == PState::Live {
            p.viewheight += p.deltaviewheight;
            if p.viewheight > VIEWHEIGHT {
                p.viewheight = VIEWHEIGHT;
                p.deltaviewheight = 0;
            }
            if p.viewheight < VIEWHEIGHT / 2 {
                p.viewheight = VIEWHEIGHT / 2;
                if p.deltaviewheight <= 0 {
                    p.deltaviewheight = 1;
                }
            }
            if p.deltaviewheight != 0 {
                p.deltaviewheight += FRACUNIT / 4;
                if p.deltaviewheight == 0 {
                    p.deltaviewheight = 1;
                }
            }
        }
        p.viewz = (mo.z + p.viewheight + bobv).min(mo.ceilingz - 4 * FRACUNIT);
    }

    fn death_think(&mut self) {
        self.move_psprites();
        let m = self.player.mo;
        if self.player.viewheight > 6 * FRACUNIT {
            self.player.viewheight -= FRACUNIT;
        }
        if self.player.viewheight < 6 * FRACUNIT {
            self.player.viewheight = 6 * FRACUNIT;
        }
        self.player.deltaviewheight = 0;
        self.calc_height();
        let a = self.player.attacker;
        if a != NONE && a != m && self.mobjs[a as usize].in_use {
            let (mo, at) = (self.mobjs[m as usize], self.mobjs[a as usize]);
            let angle = point_to_angle(at.x - mo.x, at.y - mo.y);
            let delta = angle.wrapping_sub(mo.angle);
            let ang5 = ANG90 / 18;
            if delta < ang5 || delta > 0u32.wrapping_sub(ang5) {
                self.mobjs[m as usize].angle = angle;
                if self.player.damagecount > 0 {
                    self.player.damagecount -= 1;
                }
            } else if delta < ANG180 {
                self.mobjs[m as usize].angle = mo.angle.wrapping_add(ang5);
            } else {
                self.mobjs[m as usize].angle = mo.angle.wrapping_sub(ang5);
            }
        } else if self.player.damagecount > 0 {
            self.player.damagecount -= 1;
        }
        if self.player.cmd.buttons & BT_USE != 0 && self.leveltime > 0 {
            self.player.state = PState::Reborn;
        }
    }

    // ================================================================ weapon overlay

    pub fn setup_psprites(&mut self) {
        for p in self.player.psprites.iter_mut() {
            p.state = S::Null;
            p.tics = 0;
            p.sx = 0;
            p.sy = WEAPONBOTTOM;
        }
        self.player.pendingweapon = Some(self.player.readyweapon);
        self.bring_up_weapon();
    }

    fn set_psprite(&mut self, pos: usize, mut s: S) {
        let mut guard = 0;
        loop {
            if s == S::Null {
                self.player.psprites[pos].state = S::Null;
                return;
            }
            let st = state(s);
            self.player.psprites[pos].state = s;
            self.player.psprites[pos].tics = st.tics;
            if st.action != A::None {
                self.weapon_action(pos, st.action);
                if self.player.psprites[pos].state == S::Null {
                    return;
                }
            }
            // An action may have switched state; continue from wherever we are.
            let cur = self.player.psprites[pos].state;
            if self.player.psprites[pos].tics != 0 || guard > 16 {
                return;
            }
            s = if cur == s { st.next } else { state(cur).next };
            guard += 1;
        }
    }

    fn move_psprites(&mut self) {
        for pos in 0..2 {
            let p = self.player.psprites[pos];
            if p.state != S::Null && p.tics != -1 {
                self.player.psprites[pos].tics -= 1;
                if self.player.psprites[pos].tics <= 0 {
                    self.set_psprite(pos, state(p.state).next);
                }
            }
        }
        self.player.psprites[PS_FLASH].sx = self.player.psprites[PS_WEAPON].sx;
        self.player.psprites[PS_FLASH].sy = self.player.psprites[PS_WEAPON].sy;
    }

    fn bring_up_weapon(&mut self) {
        let w = self.player.pendingweapon.unwrap_or(self.player.readyweapon);
        self.player.pendingweapon = None;
        self.player.psprites[PS_WEAPON].sy = WEAPONBOTTOM;
        self.set_psprite(PS_WEAPON, WEAPONINFO[w as usize].up);
    }

    fn check_ammo(&mut self) -> bool {
        let p = &self.player;
        let wi = &WEAPONINFO[p.readyweapon as usize];
        if wi.ammo == AM_NONE || p.ammo[wi.ammo] >= wi.per_shot {
            return true;
        }
        // Out of ammo: switch to the best weapon that can still fire.
        let own = |w: Weapon| p.weaponowned[w as usize];
        let next = if own(Weapon::Plasma) && p.ammo[AM_CELLS] > 0 {
            Weapon::Plasma
        } else if own(Weapon::Chaingun) && p.ammo[AM_BULLETS] > 0 {
            Weapon::Chaingun
        } else if own(Weapon::Shotgun) && p.ammo[AM_SHELLS] > 0 {
            Weapon::Shotgun
        } else if p.ammo[AM_BULLETS] > 0 {
            Weapon::Pistol
        } else if own(Weapon::Drill) {
            Weapon::Drill
        } else if own(Weapon::Launcher) && p.ammo[AM_ROCKETS] > 0 {
            Weapon::Launcher
        } else if own(Weapon::Arc) && p.ammo[AM_CELLS] >= 40 {
            Weapon::Arc
        } else {
            Weapon::Fist
        };
        self.player.pendingweapon = Some(next);
        let down = WEAPONINFO[self.player.readyweapon as usize].down;
        self.set_psprite(PS_WEAPON, down);
        false
    }

    fn fire_weapon(&mut self) {
        if !self.check_ammo() {
            return;
        }
        let m = self.player.mo;
        self.set_state(m, S::PlayAtk1);
        let atk = WEAPONINFO[self.player.readyweapon as usize].attack;
        self.set_psprite(PS_WEAPON, atk);
        self.noise_alert(m, m);
    }

    fn use_ammo(&mut self, n: i32) {
        let a = WEAPONINFO[self.player.readyweapon as usize].ammo;
        if a != AM_NONE {
            self.player.ammo[a] = (self.player.ammo[a] - n).max(0);
        }
    }

    fn bullet_slope(&mut self) -> Fixed {
        let m = self.player.mo;
        let a = self.mobjs[m as usize].angle;
        let range = 16 * 64 * FRACUNIT;
        let mut slope = self.aim_line_attack(m, a, range);
        if self.tm.linetarget == NONE {
            slope = self.aim_line_attack(m, a.wrapping_add(1 << 26), range);
            if self.tm.linetarget == NONE {
                slope = self.aim_line_attack(m, a.wrapping_sub(1 << 26), range);
            }
        }
        slope
    }

    fn gun_shot(&mut self, accurate: bool, slope: Fixed) {
        let m = self.player.mo;
        let damage = 5 * (self.rnd() % 3 + 1);
        let mut a = self.mobjs[m as usize].angle;
        if !accurate {
            a = a.wrapping_add((self.rnd_sub() << 18) as u32);
        }
        self.line_attack(m, a, MISSILERANGE, slope, damage);
    }

    fn weapon_action(&mut self, pos: usize, a: A) {
        let m = self.player.mo;
        match a {
            A::WeaponReady => {
                let st = self.mobjs[m as usize].state;
                if st == S::PlayAtk1 || st == S::PlayAtk2 {
                    self.set_state(m, S::PlayStand);
                }
                if self.player.pendingweapon.is_some() || self.player.health <= 0 {
                    let down = WEAPONINFO[self.player.readyweapon as usize].down;
                    self.set_psprite(PS_WEAPON, down);
                    return;
                }
                if self.player.cmd.buttons & BT_ATTACK != 0 {
                    let w = self.player.readyweapon;
                    if !self.player.attackdown || (w != Weapon::Launcher && w != Weapon::Arc) {
                        self.player.attackdown = true;
                        self.fire_weapon();
                        return;
                    }
                } else {
                    self.player.attackdown = false;
                }
                let bob = self.player.bob;
                let angle = (128 * self.leveltime as usize) & FINEMASK;
                let ps = &mut self.player.psprites[pos];
                ps.sx = fmul(bob, finecosine(angle));
                ps.sy = WEAPONTOP + fmul(bob, finesine(angle & (FINEANGLES / 2 - 1)));
            }
            A::ReFire => {
                if self.player.cmd.buttons & BT_ATTACK != 0 && self.player.pendingweapon.is_none() && self.player.health > 0 {
                    self.player.refire += 1;
                    self.fire_weapon();
                } else {
                    self.player.refire = 0;
                    self.check_ammo();
                }
            }
            A::Lower => {
                self.player.psprites[pos].sy += LOWERSPEED;
                if self.player.psprites[pos].sy < WEAPONBOTTOM {
                    return;
                }
                if self.player.state == PState::Dead {
                    self.player.psprites[pos].sy = WEAPONBOTTOM;
                    return;
                }
                if self.player.health <= 0 {
                    self.set_psprite(PS_WEAPON, S::Null);
                    return;
                }
                if let Some(w) = self.player.pendingweapon {
                    self.player.readyweapon = w;
                }
                self.bring_up_weapon();
            }
            A::Raise => {
                self.player.psprites[pos].sy -= RAISESPEED;
                if self.player.psprites[pos].sy > WEAPONTOP {
                    return;
                }
                self.player.psprites[pos].sy = WEAPONTOP;
                let ready = WEAPONINFO[self.player.readyweapon as usize].ready;
                self.set_psprite(PS_WEAPON, ready);
            }
            A::GunFlash => {
                self.set_state(m, S::PlayAtk2);
                let f = WEAPONINFO[self.player.readyweapon as usize].flash;
                self.set_psprite(PS_FLASH, f);
            }
            A::Light0 => self.player.extralight = 0,
            A::Light1 => self.player.extralight = 1,
            A::Light2 => self.player.extralight = 2,
            A::FirePunch => {
                let mut damage = (self.rnd() % 10 + 1) << 1;
                if self.player.powers[PW_STRENGTH] > 0 {
                    damage *= 10;
                }
                let a = self.mobjs[m as usize].angle.wrapping_add((self.rnd_sub() << 18) as u32);
                let slope = self.aim_line_attack(m, a, MELEERANGE);
                self.line_attack(m, a, MELEERANGE, slope, damage);
                let t = self.tm.linetarget;
                if t != NONE {
                    let (mo, tt) = (self.mobjs[m as usize], self.mobjs[t as usize]);
                    self.mobjs[m as usize].angle = point_to_angle(tt.x - mo.x, tt.y - mo.y);
                }
            }
            A::FireDrill => {
                let damage = 2 * (self.rnd() % 10 + 1);
                let a = self.mobjs[m as usize].angle.wrapping_add((self.rnd_sub() << 18) as u32);
                let slope = self.aim_line_attack(m, a, MELEERANGE + 1);
                self.line_attack(m, a, MELEERANGE + 1, slope, damage);
                let t = self.tm.linetarget;
                if t == NONE {
                    return;
                }
                let (mo, tt) = (self.mobjs[m as usize], self.mobjs[t as usize]);
                let angle = point_to_angle(tt.x - mo.x, tt.y - mo.y);
                let d = angle.wrapping_sub(mo.angle);
                let step = ANG90 / 20;
                let new = if d > ANG180 {
                    if (d as i32) < -(step as i32) { angle.wrapping_add(ANG90 / 21) } else { mo.angle.wrapping_sub(step) }
                } else if d > step {
                    angle.wrapping_sub(ANG90 / 21)
                } else {
                    mo.angle.wrapping_add(step)
                };
                let mo = &mut self.mobjs[m as usize];
                mo.angle = new;
                mo.flags |= MF_JUSTATTACKED;
            }
            A::FirePistol => {
                self.set_state(m, S::PlayAtk2);
                self.use_ammo(1);
                self.set_psprite(PS_FLASH, S::PistolFlash);
                let slope = self.bullet_slope();
                let accurate = self.player.refire == 0;
                self.gun_shot(accurate, slope);
            }
            A::FireShotgun => {
                self.set_state(m, S::PlayAtk2);
                self.use_ammo(1);
                self.set_psprite(PS_FLASH, S::SgFlash1);
                let slope = self.bullet_slope();
                for _ in 0..7 {
                    self.gun_shot(false, slope);
                }
            }
            A::FireChaingun => {
                if self.player.ammo[AM_BULLETS] <= 0 {
                    return;
                }
                self.set_state(m, S::PlayAtk2);
                self.use_ammo(1);
                let flash = if self.player.psprites[PS_WEAPON].state == S::CgAtk2 { S::CgFlash2 } else { S::CgFlash1 };
                self.set_psprite(PS_FLASH, flash);
                let slope = self.bullet_slope();
                let accurate = self.player.refire == 0;
                self.gun_shot(accurate, slope);
            }
            A::FireLauncher => {
                self.use_ammo(1);
                self.spawn_player_missile(m, ThingKind::Rocket);
            }
            A::FirePlasma => {
                self.use_ammo(1);
                let flash = if self.rnd() & 1 != 0 { S::PlFlash2 } else { S::PlFlash1 };
                self.set_psprite(PS_FLASH, flash);
                self.spawn_player_missile(m, ThingKind::PlasmaShot);
            }
            A::FireArc => {
                self.use_ammo(40);
                self.spawn_player_missile(m, ThingKind::ArcShot);
            }
            _ => {}
        }
    }

    /// Arc cannon blast: forty invisible tracers fan out from the shooter's
    /// facing and burn whatever they touch.
    pub fn a_arc_spray(&mut self, m: MRef) {
        let shooter = self.mobjs[m as usize].target;
        if shooter == NONE || !self.mobjs[shooter as usize].in_use {
            return;
        }
        let base = self.mobjs[m as usize].angle;
        for i in 0..40u32 {
            let an = base.wrapping_sub(ANG90 / 2).wrapping_add(ANG90 / 40 * i);
            let save = self.mobjs[shooter as usize].angle;
            self.aim_line_attack(shooter, an, 16 * 64 * FRACUNIT);
            self.mobjs[shooter as usize].angle = save;
            let t = self.tm.linetarget;
            if t == NONE {
                continue;
            }
            let tt = self.mobjs[t as usize];
            self.spawn_mobj(tt.x, tt.y, tt.z + (tt.height >> 2), ThingKind::ArcTrace);
            let mut damage = 0;
            for _ in 0..15 {
                damage += (self.rnd() & 7) + 1;
            }
            self.damage_mobj(t, shooter, shooter, damage);
        }
    }

    // ================================================================ pickups

    fn give_ammo(&mut self, ammo: usize, clips: i32) -> bool {
        let p = &mut self.player;
        if ammo == AM_NONE || p.ammo[ammo] >= p.maxammo[ammo] {
            return false;
        }
        let mut num = if clips > 0 { clips * CLIPAMMO[ammo] } else { CLIPAMMO[ammo] / 2 };
        if matches!(self.skill, Skill::Stroll | Skill::Inferno) {
            num <<= 1;
        }
        let old = p.ammo[ammo];
        p.ammo[ammo] = (p.ammo[ammo] + num).min(p.maxammo[ammo]);
        if old != 0 {
            return true;
        }
        // Had none: switch to something that uses it, like players expect.
        let w = p.readyweapon;
        let own = |x: Weapon| p.weaponowned[x as usize];
        match ammo {
            AM_BULLETS if w == Weapon::Fist => {
                p.pendingweapon = Some(if own(Weapon::Chaingun) { Weapon::Chaingun } else { Weapon::Pistol });
            }
            AM_SHELLS if (w == Weapon::Fist || w == Weapon::Pistol) && own(Weapon::Shotgun) => {
                p.pendingweapon = Some(Weapon::Shotgun);
            }
            AM_CELLS if (w == Weapon::Fist || w == Weapon::Pistol) && own(Weapon::Plasma) => {
                p.pendingweapon = Some(Weapon::Plasma);
            }
            AM_ROCKETS if w == Weapon::Fist && own(Weapon::Launcher) => {
                p.pendingweapon = Some(Weapon::Launcher);
            }
            _ => {}
        }
        true
    }

    fn give_weapon(&mut self, w: Weapon, dropped: bool) -> bool {
        let ammo = WEAPONINFO[w as usize].ammo;
        let gave_ammo = ammo != AM_NONE && self.give_ammo(ammo, if dropped { 1 } else { 2 });
        let mut gave_weapon = false;
        if !self.player.weaponowned[w as usize] {
            self.player.weaponowned[w as usize] = true;
            self.player.pendingweapon = Some(w);
            gave_weapon = true;
        }
        gave_weapon || gave_ammo
    }

    fn give_body(&mut self, num: i32) -> bool {
        if self.player.health >= MAXHEALTH {
            return false;
        }
        self.player.health = (self.player.health + num).min(MAXHEALTH);
        let m = self.player.mo;
        self.mobjs[m as usize].health = self.player.health;
        true
    }

    fn give_armor(&mut self, kind: u8) -> bool {
        let hits = kind as i32 * 100;
        if self.player.armorpoints >= hits {
            return false;
        }
        self.player.armortype = kind;
        self.player.armorpoints = hits;
        true
    }

    fn give_card(&mut self, c: usize) {
        if !self.player.cards[c] {
            self.player.bonuscount = BONUSADD;
            self.player.cards[c] = true;
        }
    }

    pub fn touch_special(&mut self, special: MRef, toucher: MRef) {
        let sp = self.mobjs[special as usize];
        let to = self.mobjs[toucher as usize];
        let delta = sp.z - to.z;
        if delta > to.height || delta < -8 * FRACUNIT || to.health <= 0 || !to.is_player {
            return;
        }
        let dropped = sp.flags & MF_DROPPED != 0;
        let msg: &'static str = match sp.kind {
            ThingKind::ArmorGreen => {
                if !self.give_armor(1) {
                    return;
                }
                "Picked up light armor."
            }
            ThingKind::ArmorBlue => {
                if !self.give_armor(2) {
                    return;
                }
                "Picked up heavy armor!"
            }
            ThingKind::HealthBonus => {
                self.player.health = (self.player.health + 1).min(200);
                self.mobjs[toucher as usize].health = self.player.health;
                "Picked up a vital shard."
            }
            ThingKind::ArmorBonus => {
                self.player.armorpoints = (self.player.armorpoints + 1).min(200);
                if self.player.armortype == 0 {
                    self.player.armortype = 1;
                }
                "Picked up an armor plate."
            }
            ThingKind::VitalOrb => {
                self.player.health = (self.player.health + 100).min(200);
                self.mobjs[toucher as usize].health = self.player.health;
                "Vital orb! Health surges!"
            }
            ThingKind::KeyRed => {
                self.give_card(CARD_RED);
                "Picked up a red keycard."
            }
            ThingKind::KeyBlue => {
                self.give_card(CARD_BLUE);
                "Picked up a blue keycard."
            }
            ThingKind::KeyYellow => {
                self.give_card(CARD_YELLOW);
                "Picked up a yellow keycard."
            }
            ThingKind::Stimpack => {
                if !self.give_body(10) {
                    return;
                }
                "Picked up a stim vial."
            }
            ThingKind::Medkit => {
                let low = self.player.health < 25;
                if !self.give_body(25) {
                    return;
                }
                if low { "Picked up a med case. Just in time." } else { "Picked up a med case." }
            }
            ThingKind::Aegis => {
                self.player.powers[PW_INVULN] = INVULNTICS;
                "Aegis! Nothing can touch you."
            }
            ThingKind::Adrenaline => {
                self.player.health = self.player.health.max(MAXHEALTH);
                self.mobjs[toucher as usize].health = self.player.health;
                self.player.powers[PW_STRENGTH] = 1;
                if self.player.readyweapon != Weapon::Fist {
                    self.player.pendingweapon = Some(Weapon::Fist);
                }
                "Adrenaline rush! Fists of fury!"
            }
            ThingKind::Hazmat => {
                self.player.powers[PW_IRONFEET] = IRONTICS;
                "Hazmat suit. The sludge can't hurt you now."
            }
            ThingKind::SurveyMap => {
                self.player.powers[PW_ALLMAP] = 1;
                "Survey chip: the area is mapped."
            }
            ThingKind::Clip => {
                if !self.give_ammo(AM_BULLETS, if dropped { 0 } else { 1 }) {
                    return;
                }
                "Picked up a clip."
            }
            ThingKind::AmmoBox => {
                if !self.give_ammo(AM_BULLETS, 5) {
                    return;
                }
                "Picked up a box of bullets."
            }
            ThingKind::Shells => {
                if !self.give_ammo(AM_SHELLS, 1) {
                    return;
                }
                "Picked up 4 shells."
            }
            ThingKind::ShellBox => {
                if !self.give_ammo(AM_SHELLS, 5) {
                    return;
                }
                "Picked up a box of shells."
            }
            ThingKind::Rockets => {
                if !self.give_ammo(AM_ROCKETS, 1) {
                    return;
                }
                "Picked up a rocket."
            }
            ThingKind::RocketBox => {
                if !self.give_ammo(AM_ROCKETS, 5) {
                    return;
                }
                "Picked up a crate of rockets."
            }
            ThingKind::Cell => {
                if !self.give_ammo(AM_CELLS, 1) {
                    return;
                }
                "Picked up an energy cell."
            }
            ThingKind::CellPack => {
                if !self.give_ammo(AM_CELLS, 5) {
                    return;
                }
                "Picked up a cell pack."
            }
            ThingKind::Backpack => {
                if !self.player.backpack {
                    for i in 0..4 {
                        self.player.maxammo[i] *= 2;
                    }
                    self.player.backpack = true;
                }
                for a in 0..4 {
                    self.give_ammo(a, 1);
                }
                "Picked up an ammo pack!"
            }
            ThingKind::Shotgun => {
                if !self.give_weapon(Weapon::Shotgun, dropped) {
                    return;
                }
                "You got the scattergun!"
            }
            ThingKind::Chaingun => {
                if !self.give_weapon(Weapon::Chaingun, dropped) {
                    return;
                }
                "You got the rotary cannon!"
            }
            ThingKind::Launcher => {
                if !self.give_weapon(Weapon::Launcher, dropped) {
                    return;
                }
                "You got the rocket launcher!"
            }
            ThingKind::Plasma => {
                if !self.give_weapon(Weapon::Plasma, dropped) {
                    return;
                }
                "You got the pulse rifle!"
            }
            ThingKind::ArcCannon => {
                if !self.give_weapon(Weapon::Arc, dropped) {
                    return;
                }
                "You got the ARC CANNON! Oh yes."
            }
            ThingKind::Drill => {
                if !self.give_weapon(Weapon::Drill, dropped) {
                    return;
                }
                "A power drill! Time to dig in."
            }
            _ => return,
        };
        if sp.flags & MF_COUNTITEM != 0 {
            self.player.itemcount += 1;
        }
        self.remove_mobj(special);
        self.player.bonuscount += BONUSADD;
        self.msg(msg);
    }

    // ================================================================ damage

    pub fn damage_mobj(&mut self, target: MRef, inflictor: MRef, source: MRef, damage: i32) {
        let t = self.mobjs[target as usize];
        if t.flags & MF_SHOOTABLE == 0 || t.health <= 0 {
            return;
        }
        let mut damage = damage;
        if t.is_player && self.skill == Skill::Stroll {
            damage >>= 1;
        }
        // Knockback, unless it's the drill (it holds you in place).
        let drill = source != NONE && self.mobjs[source as usize].is_player && self.player.readyweapon == Weapon::Drill;
        if inflictor != NONE && t.flags & MF_NOCLIP == 0 && !drill {
            let inf = self.mobjs[inflictor as usize];
            let mut ang = point_to_angle(t.x - inf.x, t.y - inf.y);
            let mut thrust = (damage as i64 * (FRACUNIT as i64 >> 3) * 100 / t.info().mass.max(1) as i64) as i32;
            if damage < 40 && damage > t.health && t.z - inf.z > 64 * FRACUNIT && self.rnd() & 1 != 0 {
                ang = ang.wrapping_add(ANG180);
                thrust *= 4;
            }
            let mo = &mut self.mobjs[target as usize];
            mo.momx += fmul(thrust, cos_a(ang));
            mo.momy += fmul(thrust, sin_a(ang));
        }
        if t.is_player {
            if self.lv.sectors[t.sector as usize].special == SectorSpecial::ExitDamage && damage >= t.health {
                damage = t.health - 1;
            }
            if damage < 1000 && (self.player.cheats & CHEAT_GOD != 0 || self.player.powers[PW_INVULN] > 0) {
                return;
            }
            if self.player.armortype != 0 {
                let mut saved = if self.player.armortype == 1 { damage / 3 } else { damage / 2 };
                if self.player.armorpoints <= saved {
                    saved = self.player.armorpoints;
                    self.player.armortype = 0;
                }
                self.player.armorpoints -= saved;
                damage -= saved;
            }
            self.player.health = (self.player.health - damage).max(0);
            self.player.attacker = source;
            self.player.damagecount = (self.player.damagecount + damage).min(100);
        }
        self.mobjs[target as usize].health -= damage;
        if self.mobjs[target as usize].health <= 0 {
            self.kill_mobj(source, target);
            return;
        }
        let pain = t.info().pain;
        if self.rnd() < t.info().painchance as i32 && pain != S::Null {
            self.mobjs[target as usize].flags |= MF_JUSTHIT;
            self.set_state(target, pain);
            if !self.mobjs[target as usize].in_use {
                return;
            }
        }
        self.mobjs[target as usize].reactiontime = 0;
        let t = self.mobjs[target as usize];
        if t.threshold == 0 && source != NONE && source != target && !t.is_player {
            // Infighting: turn on whoever hurt us.
            let mo = &mut self.mobjs[target as usize];
            mo.target = source;
            mo.threshold = crate::enemy::BASETHRESHOLD;
            let inf = t.info();
            if t.state == inf.spawn && inf.see != S::Null {
                self.set_state(target, inf.see);
            }
        }
    }

    pub fn kill_mobj(&mut self, source: MRef, target: MRef) {
        let t = self.mobjs[target as usize];
        {
            let mo = &mut self.mobjs[target as usize];
            mo.flags &= !(MF_SHOOTABLE | MF_FLOAT);
            mo.flags &= !MF_NOGRAVITY;
            mo.flags |= MF_CORPSE | MF_DROPOFF;
            mo.height >>= 2;
        }
        if t.flags & MF_COUNTKILL != 0 {
            self.player.killcount += 1;
        }
        let _ = source;
        if t.is_player {
            self.mobjs[target as usize].flags &= !MF_SOLID;
            self.player.state = PState::Dead;
            let down = WEAPONINFO[self.player.readyweapon as usize].down;
            self.set_psprite(PS_WEAPON, down);
        }
        let inf = t.info();
        // Overkill turns the body into gibs.
        let s = if t.health < -inf.health && inf.xdeath != S::Null { inf.xdeath } else { inf.death };
        if !self.set_state(target, s) {
            return;
        }
        let r = (self.rnd() & 3) as i16;
        {
            let mo = &mut self.mobjs[target as usize];
            mo.tics = (mo.tics - r).max(1);
        }
        let drop = match t.kind {
            ThingKind::Drone => Some(ThingKind::Clip),
            ThingKind::Enforcer => Some(ThingKind::Shotgun),
            ThingKind::Heavy => Some(ThingKind::Chaingun),
            _ => None,
        };
        if let Some(k) = drop {
            let d = self.spawn_mobj(t.x, t.y, crate::mobj::ONFLOORZ, k);
            if d != NONE {
                self.mobjs[d as usize].flags |= MF_DROPPED;
            }
        }
    }
}
