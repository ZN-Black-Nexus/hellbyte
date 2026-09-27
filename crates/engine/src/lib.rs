//! Hellbyte engine: a tiny, libre, integer-only retro FPS engine.
//!
//! `#![no_std]`, no heap. Everything lives in one static [`Engine`]; a
//! platform only has to feed it input, call [`Engine::tick`] 35 times a
//! second, call [`Engine::draw`], and put the 8-bit screen plus
//! [`Engine::palette`] on a display.

#![no_std]
#![allow(clippy::too_many_arguments, clippy::needless_range_loop, clippy::new_without_default)]

pub mod automap;
pub mod data;
pub mod draw;
pub mod enemy;
pub mod fixed;
pub mod game;
pub mod hud;
pub mod info;
pub mod level;
pub mod limits;
pub mod map;
pub mod menu;
pub mod mobj;
pub mod models;
pub mod player;
pub mod png;
pub mod render;
pub mod save;
pub mod spec;

use automap::Automap;
use draw::{Line, Surface};
use fixed::*;
use game::{Exit, Game, Skill, TICRATE};
use keys::*;
use level::NONE;
use limits::*;
use menu::{Menu, MenuAction, MenuId, Options, SLOT_NAME};
use player::*;
use render::{Renderer, ViewParams};

/// Platform-independent key codes. Printable keys use their (lowercase) ASCII code.
pub mod keys {
    pub const KEY_BACKSPACE: u16 = 8;
    pub const KEY_TAB: u16 = 9;
    pub const KEY_ENTER: u16 = 13;
    pub const KEY_ESCAPE: u16 = 27;
    pub const KEY_SPACE: u16 = 32;
    pub const KEY_UP: u16 = 0x100;
    pub const KEY_DOWN: u16 = 0x101;
    pub const KEY_LEFT: u16 = 0x102;
    pub const KEY_RIGHT: u16 = 0x103;
    pub const KEY_SHIFT: u16 = 0x104;
    pub const KEY_CTRL: u16 = 0x105;
    pub const KEY_ALT: u16 = 0x106;
    pub const KEY_F1: u16 = 0x110;
    pub const KEY_F2: u16 = 0x111;
    pub const KEY_F3: u16 = 0x112;
    pub const KEY_F4: u16 = 0x113;
    pub const KEY_F5: u16 = 0x114;
    pub const KEY_F6: u16 = 0x115;
    pub const KEY_F7: u16 = 0x116;
    pub const KEY_F8: u16 = 0x117;
    pub const KEY_F9: u16 = 0x118;
    pub const KEY_F10: u16 = 0x119;
    pub const KEY_F11: u16 = 0x11a;
    pub const KEY_F12: u16 = 0x11b;
    pub const KEY_PGUP: u16 = 0x120;
    pub const KEY_PGDN: u16 = 0x121;
    pub const KEY_HOME: u16 = 0x122;
    pub const KEY_END: u16 = 0x123;
    pub const KEY_INSERT: u16 = 0x124;
    pub const KEY_DELETE: u16 = 0x125;
    pub const KEY_PAUSE: u16 = 0x126;
    pub const KEY_MOUSE1: u16 = 0x130;
    pub const KEY_MOUSE2: u16 = 0x131;
    pub const KEY_MOUSE3: u16 = 0x132;
    pub const KEY_WHEELUP: u16 = 0x133;
    pub const KEY_WHEELDOWN: u16 = 0x134;
    pub const KEY_COUNT: usize = 0x140;
}

/// Persistent storage supplied by the platform (save slots 0-5, config 255).
pub trait Host {
    fn save_begin(&mut self, slot: u8) -> bool;
    fn save_write(&mut self, data: &[u8]) -> bool;
    fn save_end(&mut self) -> bool;
    fn load_begin(&mut self, slot: u8) -> bool;
    /// Fill `buf` completely; false at end of data.
    fn load_read(&mut self, buf: &mut [u8]) -> bool;
    fn load_end(&mut self);
}

/// A host with no storage (saves simply fail).
pub struct NullHost;

impl Host for NullHost {
    fn save_begin(&mut self, _: u8) -> bool {
        false
    }
    fn save_write(&mut self, _: &[u8]) -> bool {
        false
    }
    fn save_end(&mut self) -> bool {
        false
    }
    fn load_begin(&mut self, _: u8) -> bool {
        false
    }
    fn load_read(&mut self, _: &mut [u8]) -> bool {
        false
    }
    fn load_end(&mut self) {}
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Mode {
    Title,
    Level,
    Intermission,
    Finale,
}

/// What a text-mode frontend should print (see [`Engine::text_overlay`]).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TextKind {
    Title,
    Item,
    Selected,
    Info,
    Status,
    Message,
}

struct Inter {
    target: [i32; 4],
    cur: [i32; 4],
    stage: u8,
    next: usize,
    finale: bool,
    par: i32,
}

const PENDING_NONE: u8 = 0;
const PENDING_SAVE: u8 = 1;
const PENDING_LOAD: u8 = 2;
const PENDING_CONFIG: u8 = 3;

pub const SEEN_WORDS: usize = MAX_LINES / 32 + 1;

const FINALE: [&[u8]; 9] = [
    b"THE REACTOR CORE GOES DARK.",
    b"",
    b"THE HUSKS STOP TWITCHING, AND THE",
    b"LAST JUGGERNAUT LIES IN PIECES",
    b"ACROSS THE CARGO BAY FLOOR.",
    b"",
    b"BUT THE SIGNAL THAT WOKE THEM",
    b"IS STILL BROADCASTING FROM BELOW...",
    b"TO BE CONTINUED.",
];

pub struct Engine {
    pub g: Game,
    pub r: Renderer,
    pub mode: Mode,
    pub menu: Menu,
    pub am: Automap,
    pub opts: Options,
    /// Frontend renders text itself (terminals); the engine then only draws
    /// the 3D view and map into the framebuffer.
    pub text_ui: bool,
    keydown: [bool; KEY_COUNT],
    mouse_dx: i32,
    turnheld: i32,
    quit: bool,
    pub paused: bool,
    palette: [u8; 768],
    pal_key: u32,
    inter: Inter,
    finale_tics: u32,
    title_tics: u32,
    sw: usize,
    sh: usize,
    square: bool,
    pending: u8,
    pending_slot: u8,
    last_slot: u8,
    cheat: [u8; 8],
}

static mut ENGINE: core::mem::MaybeUninit<Engine> = core::mem::MaybeUninit::zeroed();

/// The one engine instance (about 300 KB, in .bss so it costs nothing on disk).
///
/// # Safety
/// Call once; the returned reference must be the only one in use.
pub unsafe fn instance() -> &'static mut Engine {
    // SAFETY: every field of Engine is valid when all-zero (enums have a zero
    // variant, options are None, no references), and the caller guarantees
    // uniqueness.
    unsafe { (*core::ptr::addr_of_mut!(ENGINE)).assume_init_mut() }
}

impl Engine {
    /// Initialise for a `w` x `h` screen (max 320x200).
    pub fn init(&mut self, w: usize, h: usize, square_pixels: bool, host: &mut dyn Host) {
        self.opts = Options::DEFAULT;
        save::load_config(&mut self.opts, host);
        self.menu = Menu::NEW;
        self.am = Automap::NEW;
        self.g.rng = 0x2545_f491;
        self.g.skill = Skill::Brawl;
        self.g.player.reset_inventory();
        self.sw = w.clamp(16, MAX_W);
        self.sh = h.clamp(16, MAX_H);
        self.square = square_pixels;
        self.r.init(self.sw, self.sh);
        self.pal_key = u32::MAX;
        self.go_title();
    }

    pub fn resize(&mut self, w: usize, h: usize, square_pixels: bool) {
        self.sw = w.clamp(16, MAX_W);
        self.sh = h.clamp(16, MAX_H);
        self.square = square_pixels;
        self.update_view();
    }

    pub fn set_text_ui(&mut self, on: bool) {
        self.text_ui = on;
        self.update_view();
    }

    fn update_view(&mut self) {
        let bar = !self.text_ui && self.opts.status_bar && self.mode == Mode::Level && self.sw >= 240 && self.sh >= 150;
        let vh = if bar { self.sh - hud::STATUS_H as usize } else { self.sh };
        let detail = self.opts.low_detail as u32;
        self.r.square_pixels = self.square;
        self.r.set_view(self.sw, self.sh, self.sw >> detail, vh, 0, 0);
        self.r.set_detail(detail);
    }

    fn go_title(&mut self) {
        self.mode = Mode::Title;
        self.menu.in_game = false;
        self.am.active = false;
        self.paused = false;
        self.title_tics = 0;
        self.g.setup_level(0, true);
        self.r.clear_seen();
        self.update_view();
    }

    pub fn new_game(&mut self, skill: Skill, map: usize) {
        self.g.skill = skill;
        self.g.player.reset_inventory();
        self.start_level(map, true);
    }

    fn start_level(&mut self, idx: usize, fresh: bool) {
        self.g.setup_level(idx, fresh);
        self.mode = Mode::Level;
        self.menu.in_game = true;
        self.am.active = false;
        self.paused = false;
        self.r.clear_seen();
        self.update_view();
    }

    /// Debug/testing: move the player to (x, y) map units facing `angle` degrees.
    pub fn debug_warp(&mut self, x: i32, y: i32, angle_deg: i32) {
        let m = self.g.player.mo;
        if m == NONE {
            return;
        }
        self.g.teleport_move(m, fx(x), fx(y));
        let fz = self.g.mo(m).floorz;
        let mo = self.g.mo_mut(m);
        mo.z = fz;
        mo.angle = ((angle_deg.rem_euclid(360) as u64 * ANG45 as u64) / 45) as u32;
        mo.momx = 0;
        mo.momy = 0;
        self.g.player.viewz = fz + VIEWHEIGHT;
    }

    /// Debug/testing: open a menu screen or the automap by name.
    pub fn debug_show(&mut self, what: &[u8]) {
        match what {
            b"menu" => self.menu.open(MenuId::Main),
            b"skill" => self.menu.open(MenuId::Skill),
            b"options" => self.menu.open(MenuId::Options),
            b"help" => self.menu.open(MenuId::Help),
            b"map" => self.am.active = true,
            b"title" => self.go_title(),
            _ => {}
        }
    }

    /// FNV-1a hash of the current frame and key game state (determinism tests).
    pub fn state_hash(&self) -> u32 {
        let mut h: u32 = 0x811c_9dc5;
        let mut eat = |b: u8| {
            h ^= b as u32;
            h = h.wrapping_mul(0x0100_0193);
        };
        for &b in &self.r.screen[..self.sw * self.sh] {
            eat(b);
        }
        let pm = self.g.mo(self.g.player.mo);
        for v in [pm.x, pm.y, pm.z, self.g.player.health, self.g.leveltime as i32, self.g.rng as i32] {
            for b in v.to_le_bytes() {
                eat(b);
            }
        }
        h
    }

    pub fn quit_requested(&self) -> bool {
        self.quit
    }

    /// True while mouse look makes sense (hide/grab the pointer).
    pub fn wants_pointer(&self) -> bool {
        self.mode == Mode::Level && !self.menu.active && !self.paused
    }

    pub fn screen(&self) -> (&[u8], usize, usize) {
        (&self.r.screen[..self.sw * self.sh], self.sw, self.sh)
    }

    pub fn palette(&self) -> &[u8; 768] {
        &self.palette
    }

    // ================================================================ input

    pub fn key(&mut self, k: u16, down: bool) {
        if (k as usize) < KEY_COUNT {
            self.keydown[k as usize] = down;
        }
        if !down {
            return;
        }
        if self.menu.active {
            let name = self.g.lv.map.name.as_bytes();
            let act = self.menu.key(k, &mut self.opts, name);
            self.menu_action(act);
            return;
        }
        match self.mode {
            Mode::Title => {
                if k != KEY_SHIFT && k != KEY_CTRL && k != KEY_ALT {
                    self.menu.open(MenuId::Main);
                }
            }
            Mode::Intermission => {
                if matches!(k, KEY_ENTER | KEY_SPACE | KEY_ESCAPE | KEY_CTRL | KEY_MOUSE1)
                    || k == b'e' as u16
                    || k == b'f' as u16
                {
                    self.inter_advance();
                }
            }
            Mode::Finale => {
                if self.finale_tics > TICRATE {
                    self.go_title();
                }
            }
            Mode::Level => match k {
                KEY_ESCAPE => self.menu.open(MenuId::Main),
                KEY_TAB => self.am.active = !self.am.active,
                KEY_PAUSE => self.paused = !self.paused,
                KEY_F2 => self.menu.open(MenuId::Save),
                KEY_F3 => self.menu.open(MenuId::Load),
                KEY_F4 => self.menu.open(MenuId::Options),
                KEY_F1 => self.menu.open(MenuId::Help),
                KEY_F6 => {
                    self.pending = PENDING_SAVE;
                    self.pending_slot = self.last_slot;
                }
                KEY_F9 => {
                    self.pending = PENDING_LOAD;
                    self.pending_slot = self.last_slot;
                }
                KEY_F10 => self.menu.open(MenuId::Quit),
                KEY_F11 => {
                    self.opts.brightness = (self.opts.brightness + 1) % 5;
                    self.pending = PENDING_CONFIG;
                }
                _ => {
                    if k == b'p' as u16 {
                        self.paused = !self.paused;
                    } else if self.am.active && (k == b'=' as u16 || k == b'+' as u16) {
                        self.am.zoom(true);
                    } else if self.am.active && k == b'-' as u16 {
                        self.am.zoom(false);
                    } else if self.am.active && k == b'0' as u16 {
                        self.am.follow = true;
                    }
                    if (32..127).contains(&k) {
                        self.cheat_key(k as u8);
                    }
                }
            },
        }
    }

    pub fn mouse_motion(&mut self, dx: i32, _dy: i32) {
        self.mouse_dx += dx;
    }

    pub fn mouse_button(&mut self, button: u8, down: bool) {
        self.key(KEY_MOUSE1 + button.min(4) as u16, down);
    }

    /// Release every key (e.g. when the window loses focus).
    pub fn release_all(&mut self) {
        self.keydown = [false; KEY_COUNT];
    }

    fn held(&self, k: u16) -> bool {
        self.keydown[k as usize]
    }

    fn cheat_key(&mut self, c: u8) {
        self.cheat.copy_within(1.., 0);
        self.cheat[7] = c.to_ascii_lowercase();
        let ends = |code: &[u8]| self.cheat.ends_with(code);
        let p = &mut self.g.player;
        if ends(b"hbgod") {
            p.cheats ^= CHEAT_GOD;
            self.g.msg(if self.g.player.cheats & CHEAT_GOD != 0 { "Cheat: god mode ON" } else { "Cheat: god mode OFF" });
        } else if ends(b"hbghost") {
            p.cheats ^= CHEAT_NOCLIP;
            self.g.msg(if self.g.player.cheats & CHEAT_NOCLIP != 0 { "Cheat: noclip ON" } else { "Cheat: noclip OFF" });
        } else if ends(b"hbammo") {
            p.weaponowned = [true; NUMWEAPONS];
            if !p.backpack {
                p.backpack = true;
                for m in p.maxammo.iter_mut() {
                    *m *= 2;
                }
            }
            p.ammo = p.maxammo;
            p.cards = [true; 3];
            p.armorpoints = 200;
            p.armortype = 2;
            self.g.msg("Cheat: fully loaded");
        } else if ends(b"hbreveal") {
            p.powers[PW_ALLMAP] = 1;
            self.g.msg("Cheat: map revealed");
        } else if ends(b"hbnext") {
            self.g.exit = Exit::Normal;
        } else if ends(b"hbsmite") {
            // Kill every monster on the level (testing / impatience).
            for i in 0..limits::MAX_MOBJS {
                let m = &self.g.mobjs[i];
                if m.in_use && m.flags & info::MF_COUNTKILL != 0 && m.health > 0 {
                    self.g.damage_mobj(i as level::MRef, NONE, NONE, 10000);
                }
            }
            self.g.msg("Cheat: smite!");
        }
    }

    fn menu_action(&mut self, act: MenuAction) {
        match act {
            MenuAction::None | MenuAction::Close => {}
            MenuAction::NewGame(skill) => self.new_game(skill, 0),
            MenuAction::Load(slot) => {
                self.pending = PENDING_LOAD;
                self.pending_slot = slot;
            }
            MenuAction::Save(slot) => {
                self.pending = PENDING_SAVE;
                self.pending_slot = slot;
            }
            MenuAction::Quit => self.quit = true,
            MenuAction::OptionsChanged => {
                self.update_view();
                self.pending = PENDING_CONFIG;
            }
        }
    }

    // ================================================================ simulation

    fn build_ticcmd(&mut self) -> TicCmd {
        const FORWARD: [i32; 2] = [25, 50];
        const SIDE: [i32; 2] = [24, 40];
        const TURN: [i32; 3] = [640, 1280, 320];
        let mut cmd = TicCmd::default();
        let run = (self.held(KEY_SHIFT) as usize) ^ (self.opts.always_run as usize);
        let strafe = self.held(KEY_ALT);
        let left = self.held(KEY_LEFT);
        let right = self.held(KEY_RIGHT);
        if left || right {
            self.turnheld += 1;
        } else {
            self.turnheld = 0;
        }
        // Tapping turns slowly for fine aim; holding accelerates.
        let tspeed = if self.turnheld < 6 { 2 } else { run };
        let (mut forward, mut side, mut turn) = (0i32, 0i32, 0i32);
        if strafe {
            if right {
                side += SIDE[run];
            }
            if left {
                side -= SIDE[run];
            }
        } else {
            if right {
                turn -= TURN[tspeed];
            }
            if left {
                turn += TURN[tspeed];
            }
        }
        if self.held(KEY_UP) || self.held(b'w' as u16) {
            forward += FORWARD[run];
        }
        if self.held(KEY_DOWN) || self.held(b's' as u16) {
            forward -= FORWARD[run];
        }
        if self.held(b'a' as u16) || self.held(b',' as u16) {
            side -= SIDE[run];
        }
        if self.held(b'd' as u16) || self.held(b'.' as u16) {
            side += SIDE[run];
        }
        if self.held(KEY_MOUSE3) {
            forward += FORWARD[run];
        }
        if self.held(KEY_CTRL) || self.held(b'f' as u16) || self.held(b'k' as u16) || self.held(KEY_MOUSE1) {
            cmd.buttons |= BT_ATTACK;
        }
        if self.held(KEY_SPACE) || self.held(b'e' as u16) || self.held(KEY_MOUSE2) {
            cmd.buttons |= BT_USE;
        }
        for (i, key) in (b'1'..=b'7').enumerate() {
            if self.held(key as u16) {
                cmd.buttons |= BT_CHANGE | ((i as u8) << BT_WEAPONSHIFT);
                break;
            }
        }
        turn -= self.mouse_dx * 8 * (self.opts.mouse_sens as i32 + 5) / 10;
        self.mouse_dx = 0;
        cmd.forwardmove = forward.clamp(-50, 50) as i8;
        cmd.sidemove = side.clamp(-50, 50) as i8;
        cmd.angleturn = turn.clamp(-32000, 32000) as i16;
        cmd
    }

    /// Advance the game by one tic (1/35 s).
    pub fn tick(&mut self, host: &mut dyn Host) {
        self.menu.tics = self.menu.tics.wrapping_add(1);
        self.do_pending(host);
        if self.menu.active && self.menu.slots_dirty {
            save::read_slot_names(&mut self.menu, host);
        }
        match self.mode {
            Mode::Title => self.title_tics += 1,
            Mode::Level => {
                if !self.menu.active && !self.paused {
                    let cmd = self.build_ticcmd();
                    self.g.player.cmd = cmd;
                    self.g.tick_world();
                    if self.g.player.state == PState::Reborn {
                        let idx = self.g.map_index;
                        self.g.player.reset_inventory();
                        self.start_level(idx, true);
                    } else if self.g.exit != Exit::None {
                        self.start_intermission();
                    }
                } else {
                    self.mouse_dx = 0;
                }
            }
            Mode::Intermission => self.tick_intermission(),
            Mode::Finale => self.finale_tics += 1,
        }
    }

    fn do_pending(&mut self, host: &mut dyn Host) {
        let what = self.pending;
        self.pending = PENDING_NONE;
        match what {
            PENDING_SAVE => {
                if self.mode != Mode::Level {
                    return;
                }
                // Record what the player has seen on the automap.
                for i in 0..self.g.lv.map.lines.len() {
                    if self.r.seen[i / 32] & (1 << (i % 32)) != 0 {
                        self.g.lv.lines[i].flags |= data::ML_MAPPED;
                    }
                }
                let slot = self.pending_slot;
                let mut name = self.menu.slots[slot as usize % menu::SLOTS];
                if name[0] == 0 {
                    let n = self.g.lv.map.name.as_bytes();
                    let l = n.len().min(SLOT_NAME - 1);
                    name = [0; SLOT_NAME];
                    name[..l].copy_from_slice(&n[..l]);
                }
                let ok = save::save_game(&mut self.g, slot, &name, host);
                self.g.msg(if ok { "Game saved." } else { "Could not save the game!" });
                if ok {
                    self.last_slot = slot;
                    self.menu.slots_dirty = true;
                }
            }
            PENDING_LOAD => {
                if save::load_game(&mut self.g, self.pending_slot, host) {
                    self.last_slot = self.pending_slot;
                    self.mode = Mode::Level;
                    self.menu.in_game = true;
                    self.menu.active = false;
                    self.am.active = false;
                    self.paused = false;
                    self.r.clear_seen();
                    self.update_view();
                    self.g.msg("Game loaded.");
                } else if self.mode == Mode::Level {
                    self.g.msg("No saved game there.");
                }
            }
            PENDING_CONFIG => {
                save::save_config(&self.opts, host);
            }
            _ => {}
        }
    }

    fn start_intermission(&mut self) {
        let g = &self.g;
        let pct = |a: i32, b: i32| if b > 0 { a * 100 / b } else { 100 };
        let map = g.lv.map;
        let secret = g.exit == Exit::Secret;
        let want = if secret && !map.secret_next.is_empty() { map.secret_next } else { map.next };
        let next = if want.is_empty() {
            g.map_index + 1
        } else {
            data::MAPS.iter().position(|m| m.id == want).unwrap_or(g.map_index + 1)
        };
        self.inter = Inter {
            target: [
                pct(g.player.killcount, g.totalkills),
                pct(g.player.itemcount, g.totalitems),
                pct(g.player.secretcount, g.totalsecret),
                (g.leveltime / TICRATE) as i32,
            ],
            cur: [0; 4],
            stage: 0,
            next,
            finale: next >= data::MAPS.len(),
            par: map.par,
        };
        self.mode = Mode::Intermission;
        self.am.active = false;
        self.update_view();
    }

    fn tick_intermission(&mut self) {
        let i = &mut self.inter;
        if i.stage >= 4 {
            return;
        }
        let s = i.stage as usize;
        let step = if s == 3 { 3 } else { 2 };
        i.cur[s] = (i.cur[s] + step).min(i.target[s]);
        if i.cur[s] >= i.target[s] {
            i.stage += 1;
        }
    }

    fn inter_advance(&mut self) {
        if self.inter.stage < 4 {
            self.inter.cur = self.inter.target;
            self.inter.stage = 4;
            return;
        }
        if self.inter.finale {
            self.mode = Mode::Finale;
            self.finale_tics = 0;
            self.menu.in_game = false;
        } else {
            let n = self.inter.next;
            self.start_level(n, false);
        }
    }

    // ================================================================ drawing

    fn view_params(&self) -> ViewParams {
        let g = &self.g;
        let pm = g.mo(g.player.mo);
        let (angle, z) = if self.mode == Mode::Title {
            (pm.angle.wrapping_add(self.title_tics.wrapping_mul(ANG1 / 3)), pm.z + VIEWHEIGHT)
        } else {
            (pm.angle, g.player.viewz)
        };
        ViewParams {
            x: pm.x,
            y: pm.y,
            z,
            angle,
            extralight: g.player.extralight + self.opts.brightness as i32 / 2,
            fixedcolormap: g.player.fixedcolormap,
            player: g.player.mo,
            psprites: if self.mode == Mode::Level { g.player.psprite_views(g.leveltime) } else { [None, None] },
            shadow_weapon: false,
            leveltime: if self.mode == Mode::Title { self.title_tics } else { g.leveltime },
            sky: g.lv.map.sky,
        }
    }

    /// Render the current frame into the screen buffer.
    pub fn draw(&mut self) {
        let (sw, sh) = (self.sw as i32, self.sh as i32);
        match self.mode {
            Mode::Title | Mode::Level => {
                if self.mode == Mode::Level && self.am.active {
                    let vh = self.r.vh as i32;
                    let mut s = Surface { px: &mut self.r.screen[..(sw * sh) as usize], w: sw, h: vh };
                    self.am.draw(&self.g, &self.r.seen, &mut s);
                } else if self.g.player.mo != NONE {
                    let v = self.view_params();
                    self.r.render(&self.g.lv, &self.g.mobjs, &v);
                }
                let mut s = Surface { px: &mut self.r.screen[..(sw * sh) as usize], w: sw, h: sh };
                if self.mode == Mode::Title && !self.text_ui {
                    s.remap(0, 0, sw - 1, sh - 1, 8);
                    let scale = if sw >= 300 { 4 } else if sw >= 150 { 2 } else { 1 };
                    let w = 8 * draw::TEXT_W * scale;
                    s.text_grad((sw - w) / 2, sh / 4, b"HELLBYTE", 5, 15, 6, scale);
                    if !self.menu.active && (self.title_tics / 16) % 2 == 0 {
                        s.text_centered(sh * 3 / 4, b"PRESS ANY KEY", 6 * 16 + 14, 1);
                    }
                } else if self.mode == Mode::Level && !self.text_ui {
                    if self.r.vh < self.sh {
                        hud::draw_status_bar(&self.g, &mut s, self.g.leveltime);
                    } else {
                        hud::draw_overlay(&self.g, &mut s);
                    }
                    if self.opts.messages {
                        hud::draw_message(&self.g, &mut s);
                    }
                    if self.paused {
                        s.text_centered(sh / 2 - 8, b"PAUSED", 6 * 16 + 15, if sw >= 300 { 2 } else { 1 });
                    }
                }
            }
            Mode::Intermission | Mode::Finale => {
                self.backdrop();
                if !self.text_ui {
                    let mut lines: [Line; 12] = Default::default();
                    let n = self.info_lines(&mut lines);
                    let mut s = Surface { px: &mut self.r.screen[..(sw * sh) as usize], w: sw, h: sh };
                    let mut y = if sh >= 150 { 20 } else { 2 };
                    let big = if sw >= 300 { 2 } else { 1 };
                    for (i, l) in lines[..n].iter().enumerate() {
                        let scale = if i == 0 { big } else { 1 };
                        let c = if i == 0 { 5 * 16 + 15 } else { 13 * 16 + 14 };
                        s.text_centered(y, l.as_bytes(), c, scale);
                        y += 8 * scale + if i == 0 { 10 } else { 3 };
                    }
                }
            }
        }
        if self.menu.active && !self.text_ui {
            let mut s = Surface { px: &mut self.r.screen[..(sw * sh) as usize], w: sw, h: sh };
            self.menu.draw(&mut s, &self.opts);
        }
        self.update_palette();
    }

    fn backdrop(&mut self) {
        let (sw, sh) = (self.sw, self.sh);
        let tex = data::WALL_NAMES.iter().position(|n| *n == "HELLROCK").unwrap_or(1) as u8;
        for x in 0..sw {
            let col = data::wall_column(tex, x as i32);
            for y in 0..sh {
                let c = col[y & (col.len() - 1)];
                self.r.screen[y * sw + x] = data::COLORMAPS[14 * 256 + c as usize];
            }
        }
    }

    fn info_lines(&self, out: &mut [Line; 12]) -> usize {
        let mut n = 0;
        let mut push = |l: Line| {
            if n < out.len() {
                out[n] = l;
                n += 1;
            }
        };
        match self.mode {
            Mode::Intermission => {
                let i = &self.inter;
                let mut l = Line::new();
                l.s(self.g.lv.map.name.as_bytes());
                push(l);
                let mut l = Line::new();
                l.s(b"COMPLETED");
                push(l);
                let mut l = Line::new();
                push(l);
                for (k, label) in [b"KILLS   " as &[u8], b"ITEMS   ", b"SECRETS "].iter().enumerate() {
                    l = Line::new();
                    l.s(label).n(i.cur[k]).s(b"%");
                    push(l);
                }
                l = Line::new();
                l.s(b"TIME    ").n(i.cur[3] / 60).s(b":");
                if i.cur[3] % 60 < 10 {
                    l.s(b"0");
                }
                l.n(i.cur[3] % 60);
                push(l);
                if i.par > 0 {
                    l = Line::new();
                    l.s(b"PAR     ").n(i.par / 60).s(b":");
                    if i.par % 60 < 10 {
                        l.s(b"0");
                    }
                    l.n(i.par % 60);
                    push(l);
                }
                if i.stage >= 4 {
                    l = Line::new();
                    push(l);
                    l = Line::new();
                    l.s(b"PRESS FIRE TO CONTINUE");
                    push(l);
                }
            }
            Mode::Finale => {
                for t in FINALE.iter() {
                    let mut l = Line::new();
                    l.s(t);
                    push(l);
                }
            }
            _ => {}
        }
        n
    }

    /// Text for frontends that draw their own UI (`set_text_ui(true)`).
    pub fn text_overlay(&self, f: &mut dyn FnMut(TextKind, &[u8])) {
        if self.menu.active {
            let mut first = true;
            self.menu.lines(&self.opts, &mut |t, sel| {
                let k = if first {
                    TextKind::Title
                } else if sel {
                    TextKind::Selected
                } else {
                    TextKind::Item
                };
                first = false;
                f(k, t);
            });
            return;
        }
        match self.mode {
            Mode::Title => {
                f(TextKind::Title, b"HELLBYTE");
                f(TextKind::Info, b"PRESS ANY KEY");
            }
            Mode::Level => {
                let l = hud::status_line(&self.g);
                f(TextKind::Status, l.as_bytes());
                if self.opts.messages {
                    if let Some(m) = self.g.message {
                        f(TextKind::Message, m.as_bytes());
                    }
                }
                if self.paused {
                    f(TextKind::Info, b"PAUSED");
                }
            }
            Mode::Intermission | Mode::Finale => {
                let mut lines: [Line; 12] = Default::default();
                let n = self.info_lines(&mut lines);
                for (i, l) in lines[..n].iter().enumerate() {
                    f(if i == 0 { TextKind::Title } else { TextKind::Item }, l.as_bytes());
                }
            }
        }
    }

    fn update_palette(&mut self) {
        let p = &self.g.player;
        let mut red = p.damagecount;
        if p.powers[PW_STRENGTH] > 0 {
            let fade = 12 - (p.powers[PW_STRENGTH] >> 6);
            red = red.max(fade * 3);
        }
        let (tint, amount): ([i32; 3], i32) = if self.mode != Mode::Level {
            ([0, 0, 0], 0)
        } else if red > 0 {
            ([255, 0, 0], (red * 5).min(170))
        } else if p.bonuscount > 0 {
            ([215, 186, 69], ((p.bonuscount + 7) >> 3).min(4) * 22)
        } else if p.powers[PW_IRONFEET] > 4 * 32 || p.powers[PW_IRONFEET] & 8 != 0 {
            ([0, 255, 0], 36)
        } else {
            ([0, 0, 0], 0)
        };
        let key = (amount as u32) << 8 | (tint[1] as u32 & 0xf0) | self.opts.brightness as u32;
        if key == self.pal_key {
            return;
        }
        self.pal_key = key;
        let b = self.opts.brightness as i32;
        for i in 0..768 {
            let v = data::PALETTE[i] as i32;
            // Brightness lifts mid tones, leaving black and white alone.
            let v = v + (255 - v) * v * b / (255 * 6);
            let t = tint[i % 3];
            self.palette[i] = (v + (t - v) * amount / 256).clamp(0, 255) as u8;
        }
    }
}
