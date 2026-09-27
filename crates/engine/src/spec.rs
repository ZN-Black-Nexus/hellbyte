//! Level specials: doors, lifts, moving floors and ceilings, stairs,
//! lighting effects, switches, teleporters, damaging floors and exits.

use crate::data::*;
use crate::fixed::*;
use crate::game::{Exit, Game};
use crate::info::*;
use crate::level::*;
use crate::player::{CARD_BLUE, CARD_RED, CARD_YELLOW, PW_IRONFEET};

const VDOORSPEED: Fixed = 2 * FRACUNIT;
const VDOORWAIT: i16 = 150;
const PLATSPEED: Fixed = FRACUNIT;
const PLATWAIT: i16 = 3 * 35;
const FLOORSPEED: Fixed = FRACUNIT;
const CEILSPEED: Fixed = FRACUNIT;
const BUTTONTIME: i16 = 35;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum MoverKind {
    None,
    Door,
    Plat,
    Floor,
    Ceiling,
}

// door subtypes
pub const DOOR_NORMAL: u8 = 0;
pub const DOOR_CLOSE30_OPEN: u8 = 1;
pub const DOOR_CLOSE: u8 = 2;
pub const DOOR_OPEN: u8 = 3;
pub const DOOR_BLAZE_RAISE: u8 = 4;
pub const DOOR_BLAZE_OPEN: u8 = 5;
// ceiling subtypes
pub const CEIL_LOWER_FLOOR: u8 = 0;
pub const CEIL_CRUSH: u8 = 1;

#[derive(Clone, Copy)]
pub struct Mover {
    pub kind: MoverKind,
    pub sub: u8,
    pub sector: u16,
    pub dir: i8,
    pub olddir: i8,
    pub speed: Fixed,
    pub low: Fixed,
    pub high: Fixed,
    pub wait: i16,
    pub count: i16,
    pub crush: bool,
    pub tag: u16,
    /// Repeatable crusher (stays in the list when stopped).
    pub perpetual: bool,
}

impl Mover {
    pub const EMPTY: Mover = Mover {
        kind: MoverKind::None,
        sub: 0,
        sector: 0,
        dir: 0,
        olddir: 0,
        speed: 0,
        low: 0,
        high: 0,
        wait: 0,
        count: 0,
        crush: false,
        tag: 0,
        perpetual: false,
    };
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum LightKind {
    None,
    Flicker,
    Strobe,
    Glow,
}

#[derive(Clone, Copy)]
pub struct LightFx {
    pub kind: LightKind,
    pub sector: u16,
    pub count: i16,
    pub min: u8,
    pub max: u8,
    pub dark: i16,
    pub bright: i16,
    pub dir: i8,
}

impl LightFx {
    pub const EMPTY: LightFx = LightFx { kind: LightKind::None, sector: 0, count: 0, min: 0, max: 0, dark: 0, bright: 0, dir: 0 };
}

/// A switch that pops back after a while.
#[derive(Clone, Copy)]
pub struct Button {
    pub line: u16,
    pub part: u8,
    pub texture: u8,
    pub timer: i16,
}

impl Button {
    pub const EMPTY: Button = Button { line: 0, part: 0, texture: 0, timer: 0 };
}

#[derive(Clone, Copy)]
pub struct Scroller {
    pub side: u16,
    pub active: bool,
}

impl Scroller {
    pub const EMPTY: Scroller = Scroller { side: 0, active: false };
}

#[derive(PartialEq, Eq)]
enum MoveRes {
    Ok,
    Crushed,
    PastDest,
}

impl Game {
    // ================================================================ movers

    fn new_mover(&mut self, sec: usize, kind: MoverKind) -> Option<usize> {
        let i = self.movers.iter().position(|m| m.kind == MoverKind::None)?;
        self.movers[i] = Mover { kind, sector: sec as u16, ..Mover::EMPTY };
        self.lv.sectors[sec].mover = (i + 1) as u16;
        Some(i)
    }

    fn remove_mover(&mut self, i: usize) {
        let sec = self.movers[i].sector as usize;
        if self.lv.sectors[sec].mover == (i + 1) as u16 {
            self.lv.sectors[sec].mover = 0;
        }
        self.movers[i] = Mover::EMPTY;
    }

    fn move_plane(&mut self, sec: usize, speed: Fixed, dest: Fixed, crush: bool, ceiling: bool, dir: i8) -> MoveRes {
        let s = &mut self.lv.sectors[sec];
        let (cur, ok_past) = if ceiling { (s.ceil, dest) } else { (s.floor, dest) };
        let set = |g: &mut Game, v: Fixed| {
            if ceiling {
                g.lv.sectors[sec].ceil = v;
            } else {
                g.lv.sectors[sec].floor = v;
            }
        };
        let past = if dir < 0 { cur - speed < ok_past } else { cur + speed > ok_past };
        if past {
            set(self, dest);
            if self.change_sector(sec, crush) && !(ceiling && dir > 0) && !(!ceiling && dir < 0) {
                set(self, cur);
                self.change_sector(sec, crush);
                return MoveRes::Crushed;
            }
            return MoveRes::PastDest;
        }
        let next = if dir < 0 { cur - speed } else { cur + speed };
        set(self, next);
        let squeezing = (ceiling && dir < 0) || (!ceiling && dir > 0);
        if squeezing && self.change_sector(sec, crush) {
            if crush {
                return MoveRes::Crushed;
            }
            set(self, cur);
            self.change_sector(sec, crush);
            return MoveRes::Crushed;
        } else if !squeezing {
            self.change_sector(sec, crush);
        }
        MoveRes::Ok
    }

    fn think_mover(&mut self, i: usize) {
        let mv = self.movers[i];
        let sec = mv.sector as usize;
        match mv.kind {
            MoverKind::None => {}
            MoverKind::Door => match mv.dir {
                0 => {
                    self.movers[i].count -= 1;
                    if self.movers[i].count <= 0 {
                        self.movers[i].dir = if mv.sub == DOOR_CLOSE30_OPEN { 1 } else { -1 };
                    }
                }
                2 => {
                    self.movers[i].count -= 1;
                    if self.movers[i].count <= 0 {
                        self.movers[i].dir = 1;
                        self.movers[i].sub = DOOR_NORMAL;
                    }
                }
                -1 => {
                    let floor = self.lv.sectors[sec].floor;
                    match self.move_plane(sec, mv.speed, floor, false, true, -1) {
                        MoveRes::PastDest => {
                            if mv.sub == DOOR_CLOSE30_OPEN {
                                self.movers[i].dir = 0;
                                self.movers[i].count = 35 * 30;
                            } else {
                                self.remove_mover(i);
                            }
                        }
                        MoveRes::Crushed => {
                            if mv.sub != DOOR_CLOSE {
                                self.movers[i].dir = 1; // something's in the way: reopen
                            }
                        }
                        MoveRes::Ok => {}
                    }
                }
                _ => {
                    if self.move_plane(sec, mv.speed, mv.high, false, true, 1) == MoveRes::PastDest {
                        match mv.sub {
                            DOOR_NORMAL | DOOR_BLAZE_RAISE => {
                                self.movers[i].dir = 0;
                                self.movers[i].count = mv.wait;
                            }
                            _ => self.remove_mover(i),
                        }
                    }
                }
            },
            MoverKind::Plat => match mv.dir {
                1 => match self.move_plane(sec, mv.speed, mv.high, false, false, 1) {
                    MoveRes::Crushed => {
                        self.movers[i].count = mv.wait;
                        self.movers[i].dir = -1;
                    }
                    MoveRes::PastDest => self.remove_mover(i),
                    MoveRes::Ok => {}
                },
                -1 => {
                    if self.move_plane(sec, mv.speed, mv.low, false, false, -1) == MoveRes::PastDest {
                        self.movers[i].count = mv.wait;
                        self.movers[i].dir = 0;
                    }
                }
                _ => {
                    self.movers[i].count -= 1;
                    if self.movers[i].count <= 0 {
                        self.movers[i].dir = if self.lv.sectors[sec].floor == mv.low { 1 } else { -1 };
                    }
                }
            },
            MoverKind::Floor => {
                let dest = if mv.dir > 0 { mv.high } else { mv.low };
                if self.move_plane(sec, mv.speed, dest, mv.crush, false, mv.dir) == MoveRes::PastDest {
                    self.remove_mover(i);
                }
            }
            MoverKind::Ceiling => match mv.dir {
                0 => {}
                1 => {
                    if self.move_plane(sec, mv.speed, mv.high, false, true, 1) == MoveRes::PastDest {
                        if mv.sub == CEIL_CRUSH {
                            self.movers[i].dir = -1;
                        } else {
                            self.remove_mover(i);
                        }
                    }
                }
                _ => match self.move_plane(sec, mv.speed, mv.low, mv.crush, true, -1) {
                    MoveRes::PastDest => {
                        if mv.sub == CEIL_CRUSH {
                            self.movers[i].dir = 1;
                            self.movers[i].speed = CEILSPEED;
                        } else {
                            self.remove_mover(i);
                        }
                    }
                    MoveRes::Crushed => {
                        if mv.sub == CEIL_CRUSH {
                            self.movers[i].speed = CEILSPEED / 8;
                        }
                    }
                    MoveRes::Ok => {}
                },
            },
        }
    }

    // ================================================================ doors

    fn start_door(&mut self, sec: usize, sub: u8) -> bool {
        if self.lv.sectors[sec].mover != 0 {
            return false;
        }
        let Some(i) = self.new_mover(sec, MoverKind::Door) else { return false };
        let top = self.lv.lowest_ceiling_surrounding(sec).min(fx(4096)) - 4 * FRACUNIT;
        let mv = &mut self.movers[i];
        mv.sub = sub;
        mv.wait = VDOORWAIT;
        mv.speed = VDOORSPEED;
        mv.high = top;
        match sub {
            DOOR_CLOSE => {
                mv.dir = -1;
            }
            DOOR_BLAZE_RAISE | DOOR_BLAZE_OPEN => {
                mv.speed = VDOORSPEED * 4;
                mv.dir = 1;
            }
            _ => mv.dir = 1,
        }
        if self.lv.sectors[sec].ceil == self.movers[i].high && self.movers[i].dir == 1 && sub != DOOR_NORMAL {
            self.remove_mover(i);
            return false;
        }
        true
    }

    fn ev_do_door(&mut self, line: usize, sub: u8) -> bool {
        let tag = self.lv.line(line).tag;
        let mut any = false;
        let mut s = -1isize;
        while let Some(sec) = self.lv.find_sector_from_tag(tag, s) {
            s = sec as isize;
            any |= self.start_door(sec, sub);
        }
        any
    }

    fn ev_vertical_door(&mut self, line: usize, m: MRef, side: usize) {
        let special = self.lv.lines[line].special;
        let is_player = self.mobjs[m as usize].is_player;
        let need = match special {
            LineSpecial::DoorRedRepeat | LineSpecial::DoorRedStay => Some((CARD_RED, "A red key opens this door.")),
            LineSpecial::DoorBlueRepeat | LineSpecial::DoorBlueStay => Some((CARD_BLUE, "A blue key opens this door.")),
            LineSpecial::DoorYellowRepeat | LineSpecial::DoorYellowStay => {
                Some((CARD_YELLOW, "A yellow key opens this door."))
            }
            _ => None,
        };
        if let Some((card, msg)) = need {
            if !is_player {
                return;
            }
            if !self.player.cards[card] {
                self.msg(msg);
                return;
            }
        }
        let Some(sec) = self.lv.side_sector(line, side ^ 1) else { return };
        let active = self.lv.sectors[sec].mover;
        if active != 0 {
            let i = active as usize - 1;
            if self.movers[i].kind == MoverKind::Door
                && matches!(
                    special,
                    LineSpecial::DoorRepeat
                        | LineSpecial::DoorRepeatFast
                        | LineSpecial::DoorRedRepeat
                        | LineSpecial::DoorBlueRepeat
                        | LineSpecial::DoorYellowRepeat
                )
            {
                if self.movers[i].dir == -1 {
                    self.movers[i].dir = 1; // reopen while closing
                } else if is_player {
                    self.movers[i].dir = -1; // close it again
                }
            }
            return;
        }
        let sub = match special {
            LineSpecial::DoorRepeatFast => DOOR_BLAZE_RAISE,
            LineSpecial::DoorStay | LineSpecial::DoorRedStay | LineSpecial::DoorBlueStay | LineSpecial::DoorYellowStay => {
                self.lv.lines[line].special = LineSpecial::None;
                DOOR_OPEN
            }
            _ => DOOR_NORMAL,
        };
        let Some(i) = self.new_mover(sec, MoverKind::Door) else { return };
        let top = self.lv.lowest_ceiling_surrounding(sec).min(fx(4096)) - 4 * FRACUNIT;
        let mv = &mut self.movers[i];
        mv.sub = sub;
        mv.dir = 1;
        mv.speed = if sub == DOOR_BLAZE_RAISE { VDOORSPEED * 4 } else { VDOORSPEED };
        mv.wait = VDOORWAIT;
        mv.high = top;
    }

    // ================================================================ lifts, floors, ceilings, stairs

    fn ev_do_plat(&mut self, line: usize) -> bool {
        let tag = self.lv.line(line).tag;
        let mut any = false;
        let mut s = -1isize;
        while let Some(sec) = self.lv.find_sector_from_tag(tag, s) {
            s = sec as isize;
            if self.lv.sectors[sec].mover != 0 {
                continue;
            }
            let Some(i) = self.new_mover(sec, MoverKind::Plat) else { break };
            let floor = self.lv.sectors[sec].floor;
            let low = self.lv.lowest_floor_surrounding(sec).min(floor);
            let mv = &mut self.movers[i];
            mv.speed = PLATSPEED * 4;
            mv.low = low;
            mv.high = floor;
            mv.wait = PLATWAIT;
            mv.dir = -1;
            any = true;
        }
        any
    }

    fn ev_do_floor(&mut self, line: usize, what: LineSpecial) -> bool {
        let tag = self.lv.line(line).tag;
        let mut any = false;
        let mut s = -1isize;
        while let Some(sec) = self.lv.find_sector_from_tag(tag, s) {
            s = sec as isize;
            if self.lv.sectors[sec].mover != 0 {
                continue;
            }
            let floor = self.lv.sectors[sec].floor;
            let (dir, dest) = match what {
                LineSpecial::W1FloorLowerLowest | LineSpecial::S1FloorLowerLowest => {
                    (-1, self.lv.lowest_floor_surrounding(sec))
                }
                LineSpecial::W1FloorRaiseNearest | LineSpecial::S1FloorRaiseNearest => {
                    (1, self.lv.next_highest_floor(sec, floor))
                }
                LineSpecial::W1FloorRaise24 | LineSpecial::S1FloorRaise24 => (1, floor + fx(24)),
                LineSpecial::W1FloorRaiseHighest => (1, self.lv.highest_floor_surrounding(sec)),
                _ => {
                    let c = self.lv.lowest_ceiling_surrounding(sec).min(self.lv.sectors[sec].ceil);
                    (1, c - fx(8))
                }
            };
            let Some(i) = self.new_mover(sec, MoverKind::Floor) else { break };
            let mv = &mut self.movers[i];
            mv.dir = dir;
            mv.speed = FLOORSPEED;
            if dir > 0 {
                mv.high = dest.max(floor);
            } else {
                mv.low = dest.min(floor);
            }
            any = true;
        }
        any
    }

    fn ev_do_ceiling(&mut self, line: usize, crusher: bool) -> bool {
        let tag = self.lv.line(line).tag;
        // Restart stopped crushers first.
        let mut any = false;
        for mv in self.movers.iter_mut() {
            if mv.kind == MoverKind::Ceiling && mv.tag == tag && mv.dir == 0 {
                mv.dir = mv.olddir;
                any = true;
            }
        }
        let mut s = -1isize;
        while let Some(sec) = self.lv.find_sector_from_tag(tag, s) {
            s = sec as isize;
            if self.lv.sectors[sec].mover != 0 {
                continue;
            }
            let Some(i) = self.new_mover(sec, MoverKind::Ceiling) else { break };
            let (floor, ceil) = (self.lv.sectors[sec].floor, self.lv.sectors[sec].ceil);
            let mv = &mut self.movers[i];
            mv.tag = tag;
            mv.dir = -1;
            mv.speed = CEILSPEED;
            mv.high = ceil;
            if crusher {
                mv.sub = CEIL_CRUSH;
                mv.crush = true;
                mv.low = floor + fx(8);
            } else {
                mv.sub = CEIL_LOWER_FLOOR;
                mv.low = floor;
            }
            any = true;
        }
        any
    }

    fn ev_stop_crusher(&mut self, line: usize) {
        let tag = self.lv.line(line).tag;
        for mv in self.movers.iter_mut() {
            if mv.kind == MoverKind::Ceiling && mv.tag == tag && mv.dir != 0 {
                mv.olddir = mv.dir;
                mv.dir = 0;
            }
        }
    }

    fn ev_build_stairs(&mut self, line: usize) -> bool {
        let tag = self.lv.line(line).tag;
        let step = fx(8);
        let mut any = false;
        let mut s = -1isize;
        while let Some(first) = self.lv.find_sector_from_tag(tag, s) {
            s = first as isize;
            if self.lv.sectors[first].mover != 0 {
                continue;
            }
            any = true;
            let mut sec = first;
            let mut height = self.lv.sectors[sec].floor + step;
            let pic = self.lv.sectors[sec].floorpic;
            for _ in 0..64 {
                let Some(i) = self.new_mover(sec, MoverKind::Floor) else { break };
                self.movers[i].dir = 1;
                self.movers[i].speed = FLOORSPEED / 4;
                self.movers[i].high = height;
                // Next step: a neighbour behind a line we face, with the same floor texture.
                let mut next = None;
                for &l in self.lv.sector_lines(sec) {
                    let l = l as usize;
                    if self.lv.front_sector(l) != sec {
                        continue;
                    }
                    let Some(b) = self.lv.back_sector(l) else { continue };
                    if self.lv.sectors[b].floorpic != pic || self.lv.sectors[b].mover != 0 {
                        continue;
                    }
                    next = Some(b);
                    break;
                }
                match next {
                    Some(n) => {
                        height += step;
                        sec = n;
                    }
                    None => break,
                }
            }
        }
        any
    }

    fn ev_light_turn(&mut self, line: usize, on: bool) {
        let tag = self.lv.line(line).tag;
        let mut s = -1isize;
        while let Some(sec) = self.lv.find_sector_from_tag(tag, s) {
            s = sec as isize;
            let v = if on {
                let mut m = 0u8;
                for &l in self.lv.sector_lines(sec) {
                    if let Some(o) = self.lv.other_sector(l as usize, sec) {
                        m = m.max(self.lv.sectors[o].light);
                    }
                }
                m.max(160)
            } else {
                self.lv.min_light_surrounding(sec, 35)
            };
            self.lv.sectors[sec].light = v;
        }
    }

    // ================================================================ teleport

    fn ev_teleport(&mut self, line: usize, m: MRef) -> bool {
        if self.mobjs[m as usize].flags & MF_MISSILE != 0 {
            return false;
        }
        let tag = self.lv.line(line).tag;
        let mut s = -1isize;
        while let Some(sec) = self.lv.find_sector_from_tag(tag, s) {
            s = sec as isize;
            for d in 0..self.mobjs.len() {
                let dm = self.mobjs[d];
                if !dm.in_use || dm.kind != ThingKind::TeleportDest || dm.sector as usize != sec {
                    continue;
                }
                let o = self.mobjs[m as usize];
                if !self.teleport_move(m, dm.x, dm.y) {
                    return false;
                }
                let fz = self.mobjs[m as usize].floorz;
                self.mobjs[m as usize].z = fz;
                if self.mobjs[m as usize].is_player {
                    self.player.viewz = fz + self.player.viewheight;
                }
                self.spawn_mobj(o.x, o.y, o.z, ThingKind::TeleFog);
                let an = dm.angle;
                self.spawn_mobj(dm.x + 20 * cos_a(an), dm.y + 20 * sin_a(an), fz, ThingKind::TeleFog);
                let mo = &mut self.mobjs[m as usize];
                if mo.is_player {
                    mo.reactiontime = 18;
                }
                mo.angle = an;
                mo.momx = 0;
                mo.momy = 0;
                mo.momz = 0;
                return true;
            }
        }
        false
    }

    // ================================================================ switches

    fn change_switch_texture(&mut self, line: usize, repeat: bool) {
        let si = self.lv.line(line).side[0] as usize;
        let parts = [self.lv.sides[si].top, self.lv.sides[si].mid, self.lv.sides[si].bottom];
        for (part, &tex) in parts.iter().enumerate() {
            for &(off, on) in SWITCH_PAIRS {
                let newtex = if tex == off {
                    on
                } else if tex == on {
                    off
                } else {
                    continue;
                };
                match part {
                    0 => self.lv.sides[si].top = newtex,
                    1 => self.lv.sides[si].mid = newtex,
                    _ => self.lv.sides[si].bottom = newtex,
                }
                if repeat {
                    if let Some(b) = self.buttons.iter_mut().find(|b| b.timer == 0) {
                        *b = Button { line: line as u16, part: part as u8, texture: tex, timer: BUTTONTIME };
                    }
                }
                return;
            }
        }
        // Switch on the back side (lines can face either way in our levels).
        let back = self.lv.line(line).side[1];
        if back != NO_SIDE {
            let bi = back as usize;
            let parts = [self.lv.sides[bi].top, self.lv.sides[bi].mid, self.lv.sides[bi].bottom];
            for (part, &tex) in parts.iter().enumerate() {
                for &(off, on) in SWITCH_PAIRS {
                    let newtex = if tex == off {
                        on
                    } else if tex == on {
                        off
                    } else {
                        continue;
                    };
                    match part {
                        0 => self.lv.sides[bi].top = newtex,
                        1 => self.lv.sides[bi].mid = newtex,
                        _ => self.lv.sides[bi].bottom = newtex,
                    }
                    if repeat {
                        if let Some(b) = self.buttons.iter_mut().find(|b| b.timer == 0) {
                            *b = Button { line: line as u16 | 0x8000, part: part as u8, texture: tex, timer: BUTTONTIME };
                        }
                    }
                    return;
                }
            }
        }
    }

    // ================================================================ line triggers

    pub fn cross_special_line(&mut self, line: usize, _side: usize, m: MRef) {
        let mo = self.mobjs[m as usize];
        let special = self.lv.lines[line].special;
        if !mo.is_player {
            if mo.flags & MF_MISSILE != 0 {
                return;
            }
            if !matches!(
                special,
                LineSpecial::W1Teleport | LineSpecial::WrTeleport | LineSpecial::W1DoorOpen | LineSpecial::WrDoorOpen | LineSpecial::WrLift
            ) {
                return;
            }
        }
        let once = |g: &mut Game| g.lv.lines[line].special = LineSpecial::None;
        match special {
            LineSpecial::W1DoorOpen => {
                self.ev_do_door(line, DOOR_NORMAL);
                once(self);
            }
            LineSpecial::WrDoorOpen => {
                self.ev_do_door(line, DOOR_NORMAL);
            }
            LineSpecial::W1DoorOpenStay => {
                self.ev_do_door(line, DOOR_OPEN);
                once(self);
            }
            LineSpecial::W1DoorClose => {
                self.ev_do_door(line, DOOR_CLOSE);
                once(self);
            }
            LineSpecial::W1Lift => {
                self.ev_do_plat(line);
                once(self);
            }
            LineSpecial::WrLift => {
                self.ev_do_plat(line);
            }
            LineSpecial::W1FloorLowerLowest
            | LineSpecial::W1FloorRaiseNearest
            | LineSpecial::W1FloorRaise24
            | LineSpecial::W1FloorRaiseHighest
            | LineSpecial::W1FloorRaiseCeiling => {
                self.ev_do_floor(line, special);
                once(self);
            }
            LineSpecial::W1CeilingLowerFloor => {
                self.ev_do_ceiling(line, false);
                once(self);
            }
            LineSpecial::W1Crusher => {
                self.ev_do_ceiling(line, true);
                once(self);
            }
            LineSpecial::WrCrusher => {
                self.ev_do_ceiling(line, true);
            }
            LineSpecial::W1CrusherStop => {
                self.ev_stop_crusher(line);
                once(self);
            }
            LineSpecial::W1Stairs8 => {
                self.ev_build_stairs(line);
                once(self);
            }
            LineSpecial::W1LightOff => {
                self.ev_light_turn(line, false);
                once(self);
            }
            LineSpecial::W1LightOn => {
                self.ev_light_turn(line, true);
                once(self);
            }
            LineSpecial::W1Teleport => {
                if self.ev_teleport(line, m) {
                    once(self);
                }
            }
            LineSpecial::WrTeleport => {
                self.ev_teleport(line, m);
            }
            LineSpecial::W1Exit => self.exit = Exit::Normal,
            LineSpecial::W1SecretExit => self.exit = Exit::Secret,
            _ => {}
        }
    }

    pub fn use_special_line(&mut self, m: MRef, line: usize, side: usize) -> bool {
        let mo = self.mobjs[m as usize];
        let special = self.lv.lines[line].special;
        if !mo.is_player {
            if self.lv.lines[line].flags & ML_SECRET != 0 {
                return false;
            }
            if !matches!(special, LineSpecial::DoorRepeat) {
                return false;
            }
        }
        let sw = |g: &mut Game, repeat: bool| {
            g.change_switch_texture(line, repeat);
            if !repeat {
                g.lv.lines[line].special = LineSpecial::None;
            }
        };
        match special {
            LineSpecial::DoorRepeat
            | LineSpecial::DoorRepeatFast
            | LineSpecial::DoorStay
            | LineSpecial::DoorRedRepeat
            | LineSpecial::DoorBlueRepeat
            | LineSpecial::DoorYellowRepeat
            | LineSpecial::DoorRedStay
            | LineSpecial::DoorBlueStay
            | LineSpecial::DoorYellowStay => self.ev_vertical_door(line, m, side),
            LineSpecial::S1DoorOpen => {
                if self.ev_do_door(line, DOOR_NORMAL) {
                    sw(self, false);
                }
            }
            LineSpecial::SrDoorOpen => {
                if self.ev_do_door(line, DOOR_NORMAL) {
                    sw(self, true);
                }
            }
            LineSpecial::S1DoorOpenStay => {
                if self.ev_do_door(line, DOOR_OPEN) {
                    sw(self, false);
                }
            }
            LineSpecial::SrDoorOpenStay => {
                if self.ev_do_door(line, DOOR_OPEN) {
                    sw(self, true);
                }
            }
            LineSpecial::S1DoorClose => {
                if self.ev_do_door(line, DOOR_CLOSE) {
                    sw(self, false);
                }
            }
            LineSpecial::S1Lift => {
                if self.ev_do_plat(line) {
                    sw(self, false);
                }
            }
            LineSpecial::SrLift => {
                if self.ev_do_plat(line) {
                    sw(self, true);
                }
            }
            LineSpecial::S1FloorLowerLowest
            | LineSpecial::S1FloorRaiseNearest
            | LineSpecial::S1FloorRaise24
            | LineSpecial::S1FloorRaiseCeiling => {
                if self.ev_do_floor(line, special) {
                    sw(self, false);
                }
            }
            LineSpecial::S1Stairs8 => {
                if self.ev_build_stairs(line) {
                    sw(self, false);
                }
            }
            LineSpecial::S1Exit => {
                sw(self, false);
                self.exit = Exit::Normal;
            }
            LineSpecial::S1SecretExit => {
                sw(self, false);
                self.exit = Exit::Secret;
            }
            _ => return false,
        }
        true
    }

    pub fn shoot_special_line(&mut self, m: MRef, line: usize) {
        if !self.mobjs[m as usize].is_player {
            return;
        }
        if self.lv.lines[line].special == LineSpecial::G1DoorOpenStay && self.ev_do_door(line, DOOR_OPEN) {
            self.change_switch_texture(line, false);
            self.lv.lines[line].special = LineSpecial::None;
        }
    }

    // ================================================================ sectors

    pub fn player_in_special_sector(&mut self) {
        let m = self.player.mo;
        let mo = self.mobjs[m as usize];
        let sec = mo.sector as usize;
        if mo.z != self.lv.sectors[sec].floor {
            return;
        }
        let hazmat = self.player.powers[PW_IRONFEET] > 0;
        match self.lv.sectors[sec].special {
            SectorSpecial::Damage5 => {
                if !hazmat && self.leveltime & 31 == 0 {
                    self.damage_mobj(m, NONE, NONE, 5);
                }
            }
            SectorSpecial::Damage10 => {
                if !hazmat && self.leveltime & 31 == 0 {
                    self.damage_mobj(m, NONE, NONE, 10);
                }
            }
            SectorSpecial::Damage20 => {
                if (!hazmat || self.rnd() < 5) && self.leveltime & 31 == 0 {
                    self.damage_mobj(m, NONE, NONE, 20);
                }
            }
            SectorSpecial::Secret => {
                self.player.secretcount += 1;
                self.lv.sectors[sec].special = SectorSpecial::None;
                self.msg("You found a secret area!");
            }
            SectorSpecial::ExitDamage => {
                self.player.cheats &= !crate::player::CHEAT_GOD;
                if self.leveltime & 31 == 0 {
                    self.damage_mobj(m, NONE, NONE, 20);
                }
                if self.player.health <= 10 {
                    self.exit = Exit::Normal;
                }
            }
            _ => {}
        }
    }

    pub fn spawn_specials(&mut self) {
        for sec in 0..self.lv.num_sectors() {
            let light = self.lv.sectors[sec].light;
            let kind = match self.lv.sectors[sec].special {
                SectorSpecial::LightFlicker => Some((LightKind::Flicker, 0)),
                SectorSpecial::LightStrobeFast => Some((LightKind::Strobe, 15)),
                SectorSpecial::LightStrobeSlow => Some((LightKind::Strobe, 35)),
                SectorSpecial::LightGlow => Some((LightKind::Glow, 0)),
                SectorSpecial::Secret => {
                    self.totalsecret += 1;
                    None
                }
                SectorSpecial::DoorClose30 => {
                    if let Some(i) = self.new_mover(sec, MoverKind::Door) {
                        self.movers[i].dir = 0;
                        self.movers[i].count = 30 * 35;
                        self.movers[i].speed = VDOORSPEED;
                        self.movers[i].sub = DOOR_NORMAL;
                    }
                    self.lv.sectors[sec].special = SectorSpecial::None;
                    None
                }
                _ => None,
            };
            if let Some((k, dark)) = kind {
                let mut min = self.lv.min_light_surrounding(sec, light);
                if min == light {
                    min = if k == LightKind::Glow { light / 4 } else { 0 };
                }
                let r = self.rnd() as i16;
                if let Some(l) = self.lights.iter_mut().find(|l| l.kind == LightKind::None) {
                    *l = LightFx {
                        kind: k,
                        sector: sec as u16,
                        count: (r & 7) + 1,
                        min,
                        max: light,
                        dark,
                        bright: 5,
                        dir: -1,
                    };
                }
            }
        }
        for li in 0..self.lv.map.lines.len() {
            if self.lv.lines[li].special == LineSpecial::ScrollLeft {
                let side = self.lv.line(li).side[0];
                if let Some(s) = self.scrollers.iter_mut().find(|s| !s.active) {
                    *s = Scroller { side, active: true };
                }
            }
        }
    }

    pub fn update_specials(&mut self) {
        for i in 0..self.movers.len() {
            if self.movers[i].kind != MoverKind::None {
                self.think_mover(i);
            }
        }
        for i in 0..self.lights.len() {
            let l = self.lights[i];
            let sec = l.sector as usize;
            match l.kind {
                LightKind::None => {}
                LightKind::Flicker => {
                    self.lights[i].count -= 1;
                    if self.lights[i].count <= 0 {
                        let r = self.rnd() as i16;
                        if self.lv.sectors[sec].light == l.max {
                            self.lv.sectors[sec].light = l.min;
                            self.lights[i].count = (r & 7) + 1;
                        } else {
                            self.lv.sectors[sec].light = l.max;
                            self.lights[i].count = (r & 63) + 1;
                        }
                    }
                }
                LightKind::Strobe => {
                    self.lights[i].count -= 1;
                    if self.lights[i].count <= 0 {
                        if self.lv.sectors[sec].light == l.min {
                            self.lv.sectors[sec].light = l.max;
                            self.lights[i].count = l.bright;
                        } else {
                            self.lv.sectors[sec].light = l.min;
                            self.lights[i].count = l.dark;
                        }
                    }
                }
                LightKind::Glow => {
                    let cur = self.lv.sectors[sec].light as i32;
                    if l.dir < 0 {
                        let v = cur - 8;
                        if v <= l.min as i32 {
                            self.lv.sectors[sec].light = l.min;
                            self.lights[i].dir = 1;
                        } else {
                            self.lv.sectors[sec].light = v as u8;
                        }
                    } else {
                        let v = cur + 8;
                        if v >= l.max as i32 {
                            self.lv.sectors[sec].light = l.max;
                            self.lights[i].dir = -1;
                        } else {
                            self.lv.sectors[sec].light = v as u8;
                        }
                    }
                }
            }
        }
        for b in 0..self.buttons.len() {
            if self.buttons[b].timer > 0 {
                self.buttons[b].timer -= 1;
                if self.buttons[b].timer == 0 {
                    let bt = self.buttons[b];
                    let line = (bt.line & 0x7fff) as usize;
                    let side = self.lv.line(line).side[if bt.line & 0x8000 != 0 { 1 } else { 0 }] as usize;
                    match bt.part {
                        0 => self.lv.sides[side].top = bt.texture,
                        1 => self.lv.sides[side].mid = bt.texture,
                        _ => self.lv.sides[side].bottom = bt.texture,
                    }
                }
            }
        }
        for s in 0..self.scrollers.len() {
            if self.scrollers[s].active {
                let side = self.scrollers[s].side as usize;
                self.lv.sides[side].xoff += FRACUNIT;
            }
        }
    }
}
