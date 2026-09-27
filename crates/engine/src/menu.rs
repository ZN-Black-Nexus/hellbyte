//! Menus: main, skill, options, load/save, help and quit.

use crate::draw::{Line, Surface, TEXT_W};
use crate::game::Skill;
use crate::keys::*;

const K_W: u16 = b'w' as u16;
const K_S: u16 = b's' as u16;
const K_Y: u16 = b'y' as u16;
const K_YU: u16 = b'Y' as u16;
const K_N: u16 = b'n' as u16;
const K_NU: u16 = b'N' as u16;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuId {
    Main,
    Skill,
    Options,
    Load,
    Save,
    Help,
    Quit,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuAction {
    None,
    NewGame(Skill),
    Load(u8),
    Save(u8),
    Quit,
    Close,
    OptionsChanged,
}

pub const SLOTS: usize = 6;
pub const SLOT_NAME: usize = 24;

pub struct Options {
    pub mouse_sens: u8,
    pub always_run: bool,
    pub status_bar: bool,
    pub messages: bool,
    pub low_detail: bool,
    pub brightness: u8,
}

impl Options {
    pub const DEFAULT: Options =
        Options { mouse_sens: 5, always_run: false, status_bar: true, messages: true, low_detail: false, brightness: 1 };
}

pub struct Menu {
    pub active: bool,
    pub id: MenuId,
    pub sel: usize,
    pub slots: [[u8; SLOT_NAME]; SLOTS],
    pub slot_used: [bool; SLOTS],
    pub slots_dirty: bool,
    pub editing: bool,
    pub edit: [u8; SLOT_NAME],
    pub edit_len: usize,
    pub quip: usize,
    pub in_game: bool,
    pub tics: u32,
}

const MAIN: [&[u8]; 6] = [b"NEW GAME", b"OPTIONS", b"LOAD GAME", b"SAVE GAME", b"CONTROLS", b"QUIT"];
const SKILLS: [&[u8]; 5] = [b"STROLL", b"SKIRMISH", b"BRAWL", b"CARNAGE", b"INFERNO"];
const SKILL_HELP: [&[u8]; 5] = [
    b"HALF DAMAGE, DOUBLE AMMO",
    b"A GENTLE WARM-UP",
    b"THE WAY IT'S MEANT TO BE",
    b"MORE OF EVERYTHING",
    b"FAST, MEAN AND UNFAIR",
];
const QUIPS: [&[u8]; 6] = [
    b"THE FIENDS WILL THROW A PARTY.",
    b"THE HUSKS WILL MISS YOU. NOT.",
    b"YOUR DRILL WILL GET LONELY.",
    b"GO ON THEN. SEE IF WE CARE.",
    b"THE JUGGERNAUT CALLS YOU CHICKEN.",
    b"LEAVING ALREADY? IT'S ONLY BYTES.",
];
pub const HELP: [&[u8]; 12] = [
    b"MOVE      ARROWS / W A S D",
    b"TURN      ARROWS / MOUSE",
    b"STRAFE    A D  , .  ALT+TURN",
    b"FIRE      CTRL / F / K / MOUSE1",
    b"USE/OPEN  SPACE / E / MOUSE2",
    b"RUN       SHIFT (OR ALWAYS RUN)",
    b"WEAPONS   1 - 7",
    b"MAP       TAB  (+ - ZOOM)",
    b"MENU      ESC",
    b"SAVE/LOAD F2 / F3",
    b"QUICK S/L F6 / F9",
    b"PAUSE     P",
];

impl Menu {
    pub const NEW: Menu = Menu {
        active: false,
        id: MenuId::Main,
        sel: 0,
        slots: [[0; SLOT_NAME]; SLOTS],
        slot_used: [false; SLOTS],
        slots_dirty: true,
        editing: false,
        edit: [0; SLOT_NAME],
        edit_len: 0,
        quip: 0,
        in_game: false,
        tics: 0,
    };

    pub fn open(&mut self, id: MenuId) {
        self.active = true;
        self.id = id;
        self.sel = 0;
        self.editing = false;
        if matches!(id, MenuId::Load | MenuId::Save) {
            self.slots_dirty = true;
        }
        if id == MenuId::Quit {
            self.quip = (self.tics as usize) % QUIPS.len();
        }
    }

    fn count(&self) -> usize {
        match self.id {
            MenuId::Main => MAIN.len(),
            MenuId::Skill => SKILLS.len(),
            MenuId::Options => 7,
            MenuId::Load | MenuId::Save => SLOTS,
            MenuId::Help | MenuId::Quit => 1,
        }
    }

    /// Handle a key press while the menu is open.
    pub fn key(&mut self, key: u16, opts: &mut Options, level_name: &[u8]) -> MenuAction {
        if self.editing {
            match key {
                KEY_ESCAPE => self.editing = false,
                KEY_ENTER => {
                    self.editing = false;
                    let slot = self.sel;
                    self.slots[slot] = [0; SLOT_NAME];
                    self.slots[slot][..self.edit_len].copy_from_slice(&self.edit[..self.edit_len]);
                    self.slot_used[slot] = true;
                    self.active = false;
                    return MenuAction::Save(slot as u8);
                }
                KEY_BACKSPACE => self.edit_len = self.edit_len.saturating_sub(1),
                k if (32..127).contains(&k) && self.edit_len < SLOT_NAME - 1 => {
                    self.edit[self.edit_len] = (k as u8).to_ascii_uppercase();
                    self.edit_len += 1;
                }
                _ => {}
            }
            return MenuAction::None;
        }
        let n = self.count();
        match key {
            KEY_UP | K_W => self.sel = (self.sel + n - 1) % n,
            KEY_DOWN | K_S => self.sel = (self.sel + 1) % n,
            KEY_ESCAPE | KEY_BACKSPACE => {
                match self.id {
                    MenuId::Main => {
                        self.active = false;
                        return MenuAction::Close;
                    }
                    MenuId::Skill | MenuId::Options | MenuId::Load | MenuId::Save | MenuId::Help => {
                        let back = match self.id {
                            MenuId::Skill => 0,
                            MenuId::Options => 1,
                            MenuId::Load => 2,
                            MenuId::Save => 3,
                            _ => 4,
                        };
                        self.open(MenuId::Main);
                        self.sel = back;
                    }
                    MenuId::Quit => {
                        self.open(MenuId::Main);
                        self.sel = 5;
                    }
                }
                return MenuAction::None;
            }
            KEY_LEFT | KEY_RIGHT if self.id == MenuId::Options => {
                let d: i32 = if key == KEY_RIGHT { 1 } else { -1 };
                self.adjust(opts, d);
                return MenuAction::OptionsChanged;
            }
            K_Y | K_YU if self.id == MenuId::Quit => return MenuAction::Quit,
            K_N | K_NU if self.id == MenuId::Quit => {
                self.active = false;
                return MenuAction::Close;
            }
            KEY_ENTER | 32 | KEY_MOUSE1 => return self.select(opts, level_name),
            _ => {}
        }
        MenuAction::None
    }

    fn adjust(&mut self, opts: &mut Options, d: i32) {
        match self.sel {
            0 => opts.mouse_sens = (opts.mouse_sens as i32 + d).clamp(0, 9) as u8,
            1 => opts.always_run = !opts.always_run,
            2 => opts.status_bar = !opts.status_bar,
            3 => opts.messages = !opts.messages,
            4 => opts.low_detail = !opts.low_detail,
            5 => opts.brightness = (opts.brightness as i32 + d).clamp(0, 4) as u8,
            _ => {}
        }
    }

    fn select(&mut self, opts: &mut Options, level_name: &[u8]) -> MenuAction {
        match self.id {
            MenuId::Main => match self.sel {
                0 => self.open(MenuId::Skill),
                1 => self.open(MenuId::Options),
                2 => self.open(MenuId::Load),
                3 => {
                    if self.in_game {
                        self.open(MenuId::Save);
                    }
                }
                4 => self.open(MenuId::Help),
                _ => self.open(MenuId::Quit),
            },
            MenuId::Skill => {
                self.active = false;
                return MenuAction::NewGame(Skill::from_u8(self.sel as u8));
            }
            MenuId::Options => {
                if self.sel == 6 {
                    self.open(MenuId::Main);
                    self.sel = 1;
                } else {
                    self.adjust(opts, 1);
                    return MenuAction::OptionsChanged;
                }
            }
            MenuId::Load => {
                if self.slot_used[self.sel] {
                    self.active = false;
                    return MenuAction::Load(self.sel as u8);
                }
            }
            MenuId::Save => {
                self.editing = true;
                let src = if self.slot_used[self.sel] { &self.slots[self.sel][..] } else { level_name };
                let len = src.iter().position(|&c| c == 0).unwrap_or(src.len()).min(SLOT_NAME - 1);
                self.edit[..len].copy_from_slice(&src[..len]);
                self.edit_len = len;
            }
            MenuId::Help => {
                self.open(MenuId::Main);
                self.sel = 4;
            }
            MenuId::Quit => return MenuAction::Quit,
        }
        MenuAction::None
    }

    // ---------------------------------------------------------------- drawing

    /// Calls `f(text, selected)` for each line of the current menu (for text UIs).
    pub fn lines(&self, opts: &Options, f: &mut dyn FnMut(&[u8], bool)) {
        f(self.title(), false);
        match self.id {
            MenuId::Main => {
                for (i, t) in MAIN.iter().enumerate() {
                    f(t, i == self.sel);
                }
            }
            MenuId::Skill => {
                for (i, t) in SKILLS.iter().enumerate() {
                    f(t, i == self.sel);
                }
                f(SKILL_HELP[self.sel], false);
            }
            MenuId::Options => {
                for i in 0..7 {
                    let l = self.option_line(opts, i);
                    f(l.as_bytes(), i == self.sel);
                }
            }
            MenuId::Load | MenuId::Save => {
                for i in 0..SLOTS {
                    let mut l = Line::new();
                    l.n(i as i32 + 1).s(b". ");
                    if self.editing && i == self.sel {
                        l.s(&self.edit[..self.edit_len]);
                        if self.tics & 8 != 0 {
                            l.s(b"_");
                        }
                    } else if self.slot_used[i] {
                        let name = &self.slots[i];
                        let n = name.iter().position(|&c| c == 0).unwrap_or(SLOT_NAME);
                        l.s(&name[..n]);
                    } else {
                        l.s(b"- EMPTY -");
                    }
                    f(l.as_bytes(), i == self.sel);
                }
            }
            MenuId::Help => {
                for h in HELP.iter() {
                    f(h, false);
                }
            }
            MenuId::Quit => {
                f(QUIPS[self.quip], false);
                f(b"QUIT HELLBYTE? (Y/N)", true);
            }
        }
    }

    fn title(&self) -> &'static [u8] {
        match self.id {
            MenuId::Main => b"HELLBYTE",
            MenuId::Skill => b"CHOOSE YOUR POISON",
            MenuId::Options => b"OPTIONS",
            MenuId::Load => b"LOAD GAME",
            MenuId::Save => b"SAVE GAME",
            MenuId::Help => b"CONTROLS",
            MenuId::Quit => b"QUIT",
        }
    }

    fn option_line(&self, o: &Options, i: usize) -> Line {
        let mut l = Line::new();
        let onoff = |b: bool| -> &'static [u8] { if b { b"ON" } else { b"OFF" } };
        match i {
            0 => {
                l.s(b"MOUSE SPEED  ");
                for k in 0..10 {
                    l.s(if k == o.mouse_sens { b"#" } else { b"-" });
                }
            }
            1 => {
                l.s(b"ALWAYS RUN   ").s(onoff(o.always_run));
            }
            2 => {
                l.s(b"STATUS BAR   ").s(onoff(o.status_bar));
            }
            3 => {
                l.s(b"MESSAGES     ").s(onoff(o.messages));
            }
            4 => {
                l.s(b"LOW DETAIL   ").s(onoff(o.low_detail));
            }
            5 => {
                l.s(b"BRIGHTNESS   ");
                for k in 0..5 {
                    l.s(if k == o.brightness { b"#" } else { b"-" });
                }
            }
            _ => {
                l.s(b"BACK");
            }
        }
        l
    }

    pub fn draw(&self, s: &mut Surface, opts: &Options) {
        // Darken whatever is behind the menu.
        s.remap(0, 0, s.w - 1, s.h - 1, 20);
        let big = if s.w >= 300 { 2 } else { 1 };
        let mut y = if s.h >= 180 { 18 } else { 4 };
        let title = self.title();
        let tw = title.len() as i32 * TEXT_W * big * if self.id == MenuId::Main { 2 } else { 1 };
        if self.id == MenuId::Main && big == 2 {
            s.text_grad((s.w - tw) / 2, y, title, 5, 15, 7, 4);
            y += 7 * 4 + 14;
        } else {
            s.text_grad((s.w - tw) / 2, y, title, 5, 15, 8, big);
            y += 7 * big + 12;
        }
        let mut idx = 0usize;
        let step = if big == 2 { 18 } else { 9 };
        let flash = self.tics & 8 != 0;
        self.lines(opts, &mut |t, selected| {
            idx += 1;
            if idx == 1 {
                return; // title already drawn
            }
            let scale = if matches!(self.id, MenuId::Main | MenuId::Skill | MenuId::Quit) { big } else { 1 };
            let w = t.len() as i32 * TEXT_W * scale;
            let x = (s.w - w) / 2;
            let color = if selected { 6 * 16 + 15 } else { 13 * 16 + 11 };
            s.text(x, y, t, color, Some(1), scale);
            if selected && flash {
                s.text(x - 10 * scale, y, b">", 4 * 16 + 14, Some(1), scale);
            }
            y += if scale == 2 { step } else { 10 };
        });
    }
}
