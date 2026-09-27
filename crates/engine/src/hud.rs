//! Status bar, compact overlay HUD and message line.

use crate::data::{WALLS, WALL_NAMES};
use crate::draw::{Line, Surface};
use crate::game::Game;
use crate::player::*;

pub const STATUS_H: i32 = 32;

const fn sh(ramp: u8, s: u8) -> u8 {
    ramp * 16 + s
}

fn wall_index(name: &str) -> u8 {
    WALL_NAMES.iter().position(|n| *n == name).unwrap_or(1) as u8
}

/// Heartbeat shape for one beat (y offsets, up is negative).
const BEAT: [i8; 24] = [0, 0, 0, -1, -2, -1, 0, 0, 1, -9, -13, 5, 2, 0, 0, -1, -2, -3, -2, -1, 0, 0, 0, 0];

pub fn draw_status_bar(g: &Game, s: &mut Surface, leveltime: u32) {
    let y0 = s.h - STATUS_H;
    let w = s.w;
    // Steel background built from a wall texture, darkened.
    let tex = wall_index("BASE2");
    let t = &WALLS[tex as usize];
    for x in 0..w {
        let col = crate::data::wall_column(tex, x);
        for y in 0..STATUS_H {
            let c = col[(y as usize + 40) & (t.h as usize - 1)];
            s.put(x, y0 + y, crate::data::COLORMAPS[10 * 256 + c as usize]);
        }
    }
    s.fill(0, y0, w - 1, y0, sh(13, 11));
    let p = &g.player;
    let sections = [(0, 49), (50, 117), (118, 163), (164, 211), (212, 279), (280, 319)];
    for &(a, b) in &sections {
        s.bevel(a + 1, y0 + 2, b - 1, s.h - 2, sh(13, 2), sh(13, 9));
    }
    let lcd_off = |r: u8| sh(r, 1);
    // ammo for the current weapon
    let wi = &WEAPONINFO[p.readyweapon as usize];
    if wi.ammo != AM_NONE {
        s.seg_number(46, y0 + 5, p.ammo[wi.ammo], 3, 12, 18, sh(6, 14), lcd_off(6));
    }
    s.text(5, y0 + 24, b"AMMO", sh(0, 9), None, 1);
    // health
    s.seg_number(98, y0 + 5, p.health, 3, 12, 18, sh(4, 14), lcd_off(4));
    s.text(101, y0 + 10, b"%", sh(4, 13), None, 1);
    s.text(62, y0 + 24, b"HEALTH", sh(0, 9), None, 1);
    // weapon slots
    s.text(128, y0 + 24, b"ARMS", sh(0, 9), None, 1);
    for (i, &(wpn, label)) in [(Weapon::Pistol, b'2'), (Weapon::Shotgun, b'3'), (Weapon::Chaingun, b'4'), (Weapon::Launcher, b'5'), (Weapon::Plasma, b'6'), (Weapon::Arc, b'7')]
        .iter()
        .enumerate()
    {
        let x = 124 + (i as i32 % 3) * 12;
        let y = y0 + 5 + (i as i32 / 3) * 9;
        let owned = p.weaponowned[wpn as usize];
        let c = if p.readyweapon == wpn {
            sh(6, 15)
        } else if owned {
            sh(6, 11)
        } else {
            sh(13, 5)
        };
        s.text(x, y, &[label], c, None, 1);
    }
    // vitals monitor
    let (mx0, my0, mx1, my1) = (168, y0 + 4, 207, s.h - 5);
    let hurt = p.damagecount > 0 && (leveltime / 4) % 2 == 0;
    s.fill(mx0, my0, mx1, my1, if hurt { sh(4, 4) } else { sh(0, 1) });
    for x in (mx0..=mx1).step_by(6) {
        for y in my0..=my1 {
            s.put(x, y, sh(7, 2));
        }
    }
    let health = p.health;
    let col = if health > 60 {
        sh(7, 13)
    } else if health > 25 {
        sh(6, 13)
    } else {
        sh(4, 13)
    };
    let mid = (my0 + my1) / 2 + 3;
    let rate = if health <= 0 { 0 } else { (3 + (100 - health.min(100)) / 12) as u32 };
    let mut prev = mid;
    for x in mx0..=mx1 {
        let yv = if rate == 0 {
            mid
        } else {
            let phase = ((x - mx0) as u32 + leveltime * rate / 2) % 40;
            mid + if (phase as usize) < BEAT.len() { BEAT[phase as usize] as i32 } else { 0 }
        };
        let yv = yv.clamp(my0 + 1, my1 - 1);
        let (a, b) = if yv < prev { (yv, prev) } else { (prev, yv) };
        for y in a..=b {
            s.put(x, y, col);
        }
        prev = yv;
    }
    // armor
    s.seg_number(260, y0 + 5, p.armorpoints, 3, 12, 18, sh(10, 14), lcd_off(10));
    s.text(263, y0 + 10, b"%", sh(10, 13), None, 1);
    s.text(228, y0 + 24, b"ARMOR", sh(0, 9), None, 1);
    // keys
    for (i, &(card, ramp)) in [(CARD_RED, 4u8), (CARD_BLUE, 9), (CARD_YELLOW, 6)].iter().enumerate() {
        let y = y0 + 5 + i as i32 * 8;
        if p.cards[card] {
            s.fill(284, y, 293, y + 5, sh(ramp, 13));
            s.fill(284, y + 1, 293, y + 2, sh(ramp, 8));
        } else {
            s.bevel(284, y, 293, y + 5, sh(13, 4), sh(13, 4));
        }
    }
    // ammo table
    for (i, &(a, label)) in [(AM_BULLETS, b'B'), (AM_SHELLS, b'S'), (AM_ROCKETS, b'R'), (AM_CELLS, b'C')].iter().enumerate() {
        let y = y0 + 5 + i as i32 * 6;
        let mut l = Line::new();
        l.n(p.ammo[a]);
        let c = if wi.ammo == a { sh(6, 15) } else { sh(6, 10) };
        s.small(297, y, &[label], sh(0, 8));
        s.small(303, y, l.as_bytes(), c);
    }
}

/// Minimal HUD drawn over a fullscreen view (and on tiny screens).
pub fn draw_overlay(g: &Game, s: &mut Surface) {
    let p = &g.player;
    let wi = &WEAPONINFO[p.readyweapon as usize];
    if s.w >= 200 && s.h >= 120 {
        let y = s.h - 22;
        s.seg_number(40, y, p.health, 3, 9, 16, sh(4, 14), 0);
        s.text(42, y + 9, b"%", sh(4, 13), Some(1), 1);
        if p.armorpoints > 0 {
            s.seg_number(90, y, p.armorpoints, 3, 9, 16, sh(10, 14), 0);
            s.text(92, y + 9, b"%", sh(10, 13), Some(1), 1);
        }
        if wi.ammo != AM_NONE {
            s.seg_number(s.w - 6, y, p.ammo[wi.ammo], 3, 9, 16, sh(6, 14), 0);
        }
        for (i, &(card, ramp)) in [(CARD_RED, 4u8), (CARD_BLUE, 9), (CARD_YELLOW, 6)].iter().enumerate() {
            if p.cards[card] {
                let x = s.w - 60 + i as i32 * 8;
                s.fill(x, y + 8, x + 5, y + 15, sh(ramp, 13));
            }
        }
    } else {
        let mut l = Line::new();
        l.s(b"H").n(p.health);
        if p.armorpoints > 0 {
            l.s(b" A").n(p.armorpoints);
        }
        if wi.ammo != AM_NONE {
            l.s(b" M").n(p.ammo[wi.ammo]);
        }
        let y = s.h - 6;
        s.fill(0, y - 1, l.len as i32 * 4 + 1, s.h - 1, 1);
        s.small(1, y, l.as_bytes(), sh(6, 14));
    }
}

pub fn draw_message(g: &Game, s: &mut Surface) {
    if let Some(m) = g.message {
        let bytes = m.as_bytes();
        if s.w >= 160 {
            let per_line = ((s.w - 4) / 6) as usize;
            for (i, chunk) in bytes.chunks(per_line.max(1)).enumerate().take(3) {
                s.text(2, 2 + i as i32 * 9, chunk, sh(6, 14), Some(1), 1);
            }
        } else {
            s.fill(0, 0, s.w - 1, 6, 1);
            s.small(1, 1, bytes, sh(6, 14));
        }
    }
}

/// One-line textual status for text-mode frontends (terminals).
pub fn status_line(g: &Game) -> Line {
    let p = &g.player;
    let wi = &WEAPONINFO[p.readyweapon as usize];
    let mut l = Line::new();
    l.s(b"HP ").n(p.health).s(b"%  AR ").n(p.armorpoints).s(b"%");
    if wi.ammo != AM_NONE {
        l.s(b"  AMMO ").n(p.ammo[wi.ammo]);
    }
    l.s(b"  ").s(weapon_name(p.readyweapon));
    if p.cards.iter().any(|&c| c) {
        l.s(b"  KEYS ");
        if p.cards[CARD_RED] {
            l.s(b"R");
        }
        if p.cards[CARD_BLUE] {
            l.s(b"B");
        }
        if p.cards[CARD_YELLOW] {
            l.s(b"Y");
        }
    }
    l
}

pub fn weapon_name(w: Weapon) -> &'static [u8] {
    match w {
        Weapon::Fist => b"FISTS",
        Weapon::Pistol => b"SIDEARM",
        Weapon::Shotgun => b"SCATTERGUN",
        Weapon::Chaingun => b"ROTARY",
        Weapon::Launcher => b"LAUNCHER",
        Weapon::Plasma => b"PULSE RIFLE",
        Weapon::Arc => b"ARC CANNON",
        Weapon::Drill => b"DRILL",
    }
}
