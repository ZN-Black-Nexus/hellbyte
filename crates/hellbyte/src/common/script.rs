//! Scripted input for headless runs (`--script w:70,right:12,f:6`).

use hellbyte_engine::keys::*;
use hellbyte_engine::{Engine, Host};

/// Scripted input: comma-separated `key:tics` steps, e.g. `w:70,right:12,f:6`.
pub fn run(e: &mut Engine, host: &mut dyn Host, script: &[u8]) -> u32 {
    let mut tics = 0;
    for step in script.split(|&c| c == b',') {
        let (name, dur) = match step.iter().position(|&c| c == b':') {
            Some(p) => (&step[..p], crate::common::parse_u32(&step[p + 1..]).unwrap_or(1)),
            None => (step, 1),
        };
        let key = key_by_name(name);
        if let Some(k) = key {
            e.key(k, true);
        }
        for _ in 0..dur {
            e.tick(host);
            tics += 1;
        }
        if let Some(k) = key {
            e.key(k, false);
        }
    }
    tics
}

pub fn key_by_name(n: &[u8]) -> Option<u16> {
    Some(match n {
        b"up" => KEY_UP,
        b"down" => KEY_DOWN,
        b"left" => KEY_LEFT,
        b"right" => KEY_RIGHT,
        b"ctrl" | b"fire" => KEY_CTRL,
        b"shift" => KEY_SHIFT,
        b"alt" => KEY_ALT,
        b"space" | b"use" => KEY_SPACE,
        b"enter" => KEY_ENTER,
        b"esc" => KEY_ESCAPE,
        b"tab" => KEY_TAB,
        b"wait" | b"" => return None,
        [b'f', rest @ ..] if !rest.is_empty() => {
            let n = crate::common::parse_u32(rest)?;
            if !(1..=12).contains(&n) {
                return None;
            }
            KEY_F1 + (n as u16 - 1)
        }
        [c] => *c as u16,
        _ => return None,
    })
}

