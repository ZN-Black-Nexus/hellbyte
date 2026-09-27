//! Savegames and settings in Hellbyte's own compact binary format.
//!
//! Layout: "HBSV", version, 24-byte slot name, then the level index, skill,
//! timers, player, the dynamic parts of every sector/line/side, all live
//! mobjs, movers, lights, buttons and scrollers, and an "END!" marker.
//! Data is streamed through a small buffer to the platform's `Host`, so a
//! save never needs more than a few hundred bytes of RAM.

use crate::fixed::Fixed;
use crate::game::{Game, Skill};
use crate::info::*;
use crate::level::*;
use crate::limits::*;
use crate::menu::{Menu, Options, SLOTS, SLOT_NAME};
use crate::mobj::{Mobj, SpawnPoint};
use crate::player::*;
use crate::spec::*;
use crate::Host;

const MAGIC: &[u8; 4] = b"HBSV";
const VERSION: u8 = 1;
pub const CONFIG_SLOT: u8 = 0xff;

struct W<'a> {
    host: &'a mut dyn Host,
    buf: [u8; 256],
    len: usize,
    ok: bool,
}

impl W<'_> {
    fn bytes(&mut self, b: &[u8]) {
        for &c in b {
            if self.len == self.buf.len() {
                self.flush();
            }
            self.buf[self.len] = c;
            self.len += 1;
        }
    }
    fn flush(&mut self) {
        if self.len > 0 {
            self.ok &= self.host.save_write(&self.buf[..self.len]);
            self.len = 0;
        }
    }
    fn u8(&mut self, v: u8) {
        self.bytes(&[v]);
    }
    fn u16(&mut self, v: u16) {
        self.bytes(&v.to_le_bytes());
    }
    fn i16(&mut self, v: i16) {
        self.bytes(&v.to_le_bytes());
    }
    fn u32(&mut self, v: u32) {
        self.bytes(&v.to_le_bytes());
    }
    fn i32(&mut self, v: i32) {
        self.bytes(&v.to_le_bytes());
    }
}

struct R<'a> {
    host: &'a mut dyn Host,
    ok: bool,
}

impl R<'_> {
    fn bytes(&mut self, b: &mut [u8]) {
        if self.ok {
            self.ok = self.host.load_read(b);
        }
        if !self.ok {
            b.fill(0);
        }
    }
    fn u8(&mut self) -> u8 {
        let mut b = [0; 1];
        self.bytes(&mut b);
        b[0]
    }
    fn u16(&mut self) -> u16 {
        let mut b = [0; 2];
        self.bytes(&mut b);
        u16::from_le_bytes(b)
    }
    fn i16(&mut self) -> i16 {
        let mut b = [0; 2];
        self.bytes(&mut b);
        i16::from_le_bytes(b)
    }
    fn u32(&mut self) -> u32 {
        let mut b = [0; 4];
        self.bytes(&mut b);
        u32::from_le_bytes(b)
    }
    fn i32(&mut self) -> i32 {
        let mut b = [0; 4];
        self.bytes(&mut b);
        i32::from_le_bytes(b)
    }
}

pub fn save_game(g: &mut Game, slot: u8, name: &[u8; SLOT_NAME], host: &mut dyn Host) -> bool {
    if !host.save_begin(slot) {
        return false;
    }
    let mut w = W { host, buf: [0; 256], len: 0, ok: true };
    w.bytes(MAGIC);
    w.u8(VERSION);
    w.bytes(name);
    w.u8(g.lv.map_index as u8);
    w.u8(g.skill as u8);
    w.u32(g.leveltime);
    w.u32(g.rng);
    w.i32(g.totalkills);
    w.i32(g.totalitems);
    w.i32(g.totalsecret);
    // player
    let p = &g.player;
    w.u16(p.mo);
    w.u8(p.state as u8);
    w.i32(p.viewheight);
    w.i32(p.deltaviewheight);
    w.i32(p.health);
    w.i32(p.armorpoints);
    w.u8(p.armortype);
    for &v in &p.powers {
        w.i32(v);
    }
    for &c in &p.cards {
        w.u8(c as u8);
    }
    w.u8(p.backpack as u8);
    for &o in &p.weaponowned {
        w.u8(o as u8);
    }
    for i in 0..4 {
        w.i32(p.ammo[i]);
        w.i32(p.maxammo[i]);
    }
    w.u8(p.readyweapon as u8);
    w.u8(p.pendingweapon.map_or(0xff, |x| x as u8));
    w.i32(p.killcount);
    w.i32(p.itemcount);
    w.i32(p.secretcount);
    w.i32(p.damagecount);
    w.i32(p.bonuscount);
    w.u16(p.attacker);
    w.i32(p.extralight);
    w.u32(p.cheats);
    for ps in &p.psprites {
        w.u16(ps.state as u16);
        w.i16(ps.tics);
        w.i32(ps.sx);
        w.i32(ps.sy);
    }
    // level
    for s in &g.lv.sectors[..g.lv.num_sectors()] {
        w.i32(s.floor);
        w.i32(s.ceil);
        w.u8(s.floorpic);
        w.u8(s.ceilpic);
        w.u8(s.light);
        w.u8(s.special as u8);
        w.u16(s.tag);
        w.u16(s.mover);
        w.u16(s.soundtarget);
    }
    for l in &g.lv.lines[..g.lv.map.lines.len()] {
        w.u16(l.flags);
        w.u8(l.special as u8);
    }
    for s in &g.lv.sides[..g.lv.map.sides.len()] {
        w.u8(s.top);
        w.u8(s.mid);
        w.u8(s.bottom);
        w.i32(s.xoff);
        w.i32(s.yoff);
    }
    // mobjs
    for m in g.mobjs.iter() {
        w.u8(m.in_use as u8);
        if !m.in_use {
            continue;
        }
        for v in [m.x, m.y, m.z, m.momx, m.momy, m.momz] {
            w.i32(v);
        }
        w.u32(m.angle);
        for v in [m.floorz, m.ceilingz, m.radius, m.height] {
            w.i32(v);
        }
        w.u32(m.flags);
        w.i32(m.health);
        w.u16(m.state as u16);
        w.i16(m.tics);
        w.u8(m.kind as u8);
        w.u8(m.movedir);
        w.i16(m.movecount);
        w.i16(m.reactiontime);
        w.i16(m.threshold);
        w.u16(m.target);
        w.u16(m.tracer);
        w.i16(m.spawn.x);
        w.i16(m.spawn.y);
        w.u16(m.spawn.angle);
        w.u8(m.spawn.kind);
        w.u8(m.spawn.flags);
        w.u8(m.lastlook);
        w.u8(m.is_player as u8);
    }
    for mv in g.movers.iter() {
        w.u8(mv.kind as u8);
        w.u8(mv.sub);
        w.u16(mv.sector);
        w.u8(mv.dir as u8);
        w.u8(mv.olddir as u8);
        w.i32(mv.speed);
        w.i32(mv.low);
        w.i32(mv.high);
        w.i16(mv.wait);
        w.i16(mv.count);
        w.u8(mv.crush as u8);
        w.u16(mv.tag);
        w.u8(mv.perpetual as u8);
    }
    for l in g.lights.iter() {
        w.u8(l.kind as u8);
        w.u16(l.sector);
        w.i16(l.count);
        w.u8(l.min);
        w.u8(l.max);
        w.i16(l.dark);
        w.i16(l.bright);
        w.u8(l.dir as u8);
    }
    for b in g.buttons.iter() {
        w.u16(b.line);
        w.u8(b.part);
        w.u8(b.texture);
        w.i16(b.timer);
    }
    for s in g.scrollers.iter() {
        w.u16(s.side);
        w.u8(s.active as u8);
    }
    w.bytes(b"END!");
    w.flush();
    let ok = w.ok;
    host.save_end() && ok
}

fn enum_u8<T: Copy>(v: u8, max: u8, fallback: T) -> T {
    if v <= max {
        // SAFETY: only used for fieldless repr(u8) enums whose discriminants are 0..=max.
        unsafe { core::mem::transmute_copy::<u8, T>(&v) }
    } else {
        fallback
    }
}

pub fn load_game(g: &mut Game, slot: u8, host: &mut dyn Host) -> bool {
    if !host.load_begin(slot) {
        return false;
    }
    let mut r = R { host, ok: true };
    let mut magic = [0u8; 4];
    r.bytes(&mut magic);
    let version = r.u8();
    let mut name = [0u8; SLOT_NAME];
    r.bytes(&mut name);
    let map = r.u8() as usize;
    if !r.ok || &magic != MAGIC || version != VERSION || map >= crate::data::MAPS.len() {
        r.host.load_end();
        return false;
    }
    g.skill = Skill::from_u8(r.u8());
    // Rebuild the static parts of the level, then overwrite the dynamic state.
    g.lv.init(map);
    g.map_index = map;
    g.leveltime = r.u32();
    g.rng = r.u32();
    g.totalkills = r.i32();
    g.totalitems = r.i32();
    g.totalsecret = r.i32();
    g.exit = crate::game::Exit::None;
    g.message = None;
    let p = &mut g.player;
    p.mo = r.u16();
    p.state = enum_u8(r.u8(), PState::Reborn as u8, PState::Live);
    p.viewheight = r.i32();
    p.deltaviewheight = r.i32();
    p.health = r.i32();
    p.armorpoints = r.i32();
    p.armortype = r.u8();
    for v in p.powers.iter_mut() {
        *v = r.i32();
    }
    for c in p.cards.iter_mut() {
        *c = r.u8() != 0;
    }
    p.backpack = r.u8() != 0;
    for o in p.weaponowned.iter_mut() {
        *o = r.u8() != 0;
    }
    for i in 0..4 {
        p.ammo[i] = r.i32();
        p.maxammo[i] = r.i32();
    }
    p.readyweapon = Weapon::from_u8(r.u8()).unwrap_or(Weapon::Pistol);
    p.pendingweapon = Weapon::from_u8(r.u8());
    p.killcount = r.i32();
    p.itemcount = r.i32();
    p.secretcount = r.i32();
    p.damagecount = r.i32();
    p.bonuscount = r.i32();
    p.attacker = r.u16();
    p.extralight = r.i32();
    p.cheats = r.u32();
    for ps in p.psprites.iter_mut() {
        ps.state = S::from_u16(r.u16());
        ps.tics = r.i16();
        ps.sx = r.i32();
        ps.sy = r.i32();
    }
    p.fixedcolormap = -1;
    p.cmd = TicCmd::default();
    p.attackdown = true;
    p.usedown = true;
    let ns = g.lv.num_sectors();
    for s in g.lv.sectors[..ns].iter_mut() {
        s.floor = r.i32();
        s.ceil = r.i32();
        s.floorpic = r.u8();
        s.ceilpic = r.u8();
        s.light = r.u8();
        s.special = enum_u8(r.u8(), SectorSpecial::ExitDamage as u8, SectorSpecial::None);
        s.tag = r.u16();
        s.mover = r.u16();
        s.soundtarget = r.u16();
        s.thinglist = NONE;
    }
    let nl = g.lv.map.lines.len();
    for l in g.lv.lines[..nl].iter_mut() {
        l.flags = r.u16();
        l.special = enum_u8(r.u8(), LineSpecial::ScrollLeft as u8, LineSpecial::None);
    }
    let nsd = g.lv.map.sides.len();
    for s in g.lv.sides[..nsd].iter_mut() {
        s.top = r.u8();
        s.mid = r.u8();
        s.bottom = r.u8();
        s.xoff = r.i32();
        s.yoff = r.i32();
    }
    for m in g.mobjs.iter_mut() {
        *m = Mobj::EMPTY;
        if r.u8() == 0 {
            continue;
        }
        let mut v = [0 as Fixed; 6];
        for x in v.iter_mut() {
            *x = r.i32();
        }
        m.x = v[0];
        m.y = v[1];
        m.z = v[2];
        m.momx = v[3];
        m.momy = v[4];
        m.momz = v[5];
        m.angle = r.u32();
        m.floorz = r.i32();
        m.ceilingz = r.i32();
        m.radius = r.i32();
        m.height = r.i32();
        m.flags = r.u32();
        m.health = r.i32();
        m.state = S::from_u16(r.u16());
        m.tics = r.i16();
        m.kind = ThingKind::from_u8(r.u8());
        m.movedir = r.u8();
        m.movecount = r.i16();
        m.reactiontime = r.i16();
        m.threshold = r.i16();
        m.target = r.u16();
        m.tracer = r.u16();
        m.spawn = SpawnPoint { x: r.i16(), y: r.i16(), angle: r.u16(), kind: r.u8(), flags: r.u8() };
        m.lastlook = r.u8();
        m.is_player = r.u8() != 0;
        let st = state(m.state);
        m.sprite = st.sprite;
        m.frame = st.frame;
        m.in_use = true;
    }
    for mv in g.movers.iter_mut() {
        mv.kind = enum_u8(r.u8(), MoverKind::Ceiling as u8, MoverKind::None);
        mv.sub = r.u8();
        mv.sector = r.u16();
        mv.dir = r.u8() as i8;
        mv.olddir = r.u8() as i8;
        mv.speed = r.i32();
        mv.low = r.i32();
        mv.high = r.i32();
        mv.wait = r.i16();
        mv.count = r.i16();
        mv.crush = r.u8() != 0;
        mv.tag = r.u16();
        mv.perpetual = r.u8() != 0;
    }
    for l in g.lights.iter_mut() {
        l.kind = enum_u8(r.u8(), LightKind::Glow as u8, LightKind::None);
        l.sector = r.u16();
        l.count = r.i16();
        l.min = r.u8();
        l.max = r.u8();
        l.dark = r.i16();
        l.bright = r.i16();
        l.dir = r.u8() as i8;
    }
    for b in g.buttons.iter_mut() {
        *b = Button { line: r.u16(), part: r.u8(), texture: r.u8(), timer: r.i16() };
    }
    for s in g.scrollers.iter_mut() {
        *s = Scroller { side: r.u16(), active: r.u8() != 0 };
    }
    let mut end = [0u8; 4];
    r.bytes(&mut end);
    let ok = r.ok && &end == b"END!";
    r.host.load_end();
    if !ok {
        return false;
    }
    // Relink every mobj into the sector lists and blockmap.
    for i in 0..MAX_MOBJS {
        if g.mobjs[i].in_use {
            g.set_thing_position(i as MRef);
        }
    }
    true
}

/// Fill the menu's slot names by peeking at each save's header.
pub fn read_slot_names(menu: &mut Menu, host: &mut dyn Host) {
    for slot in 0..SLOTS {
        menu.slot_used[slot] = false;
        menu.slots[slot] = [0; SLOT_NAME];
        if !host.load_begin(slot as u8) {
            continue;
        }
        let mut head = [0u8; 5];
        let mut name = [0u8; SLOT_NAME];
        if host.load_read(&mut head) && &head[..4] == MAGIC && head[4] == VERSION && host.load_read(&mut name) {
            menu.slots[slot] = name;
            menu.slot_used[slot] = true;
        }
        host.load_end();
    }
    menu.slots_dirty = false;
}

pub fn save_config(o: &Options, host: &mut dyn Host) -> bool {
    if !host.save_begin(CONFIG_SLOT) {
        return false;
    }
    let data = [
        b'H',
        b'B',
        b'C',
        b'F',
        1,
        o.mouse_sens,
        o.always_run as u8,
        o.status_bar as u8,
        o.messages as u8,
        o.low_detail as u8,
        o.brightness,
    ];
    let ok = host.save_write(&data);
    host.save_end() && ok
}

pub fn load_config(o: &mut Options, host: &mut dyn Host) {
    if !host.load_begin(CONFIG_SLOT) {
        return;
    }
    let mut d = [0u8; 11];
    if host.load_read(&mut d) && &d[..4] == b"HBCF" && d[4] == 1 {
        o.mouse_sens = d[5].min(9);
        o.always_run = d[6] != 0;
        o.status_bar = d[7] != 0;
        o.messages = d[8] != 0;
        o.low_detail = d[9] != 0;
        o.brightness = d[10].min(4);
    }
    host.load_end();
}
