//! Command line options shared by every platform.

use super::{parse_size, parse_u32};

/// "x,y,angle" with optional minus signs.
fn parse_triple(s: &[u8]) -> Option<(i32, i32, i32)> {
    let mut v = [0i32; 3];
    let mut n = 0;
    for part in s.split(|&c| c == b',') {
        if n == 3 {
            return None;
        }
        let (neg, digits) = if part.first() == Some(&b'-') { (true, &part[1..]) } else { (false, part) };
        let x = parse_u32(digits)? as i32;
        v[n] = if neg { -x } else { x };
        n += 1;
    }
    if n == 3 { Some((v[0], v[1], v[2])) } else { None }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Backend {
    Auto,
    Term,
    X11,
    Fb,
    Window,
}

pub struct Args {
    pub backend: Backend,
    pub shot: Option<&'static [u8]>,
    pub tics: u32,
    pub map: Option<u32>,
    pub skill: u8,
    pub size: Option<(usize, usize)>,
    pub scale: u32,
    pub script: Option<&'static [u8]>,
    pub bench: bool,
    pub help: bool,
    pub version: bool,
    pub colors: u16,
    pub fps: u32,
    pub nomouse: bool,
    pub fullscreen: bool,
    pub warp: Option<(i32, i32, i32)>,
    pub show: Option<&'static [u8]>,
}

impl Args {
    pub fn parse(get: &dyn Fn(usize) -> Option<&'static [u8]>) -> Args {
        let mut a = Args {
            backend: Backend::Auto,
            shot: None,
            tics: 0,
            map: None,
            skill: 2,
            size: None,
            scale: 0,
            script: None,
            bench: false,
            help: false,
            version: false,
            colors: 0,
            fps: 35,
            nomouse: false,
            fullscreen: false,
            warp: None,
            show: None,
        };
        let mut i = 1;
        while let Some(arg) = get(i) {
            let val = get(i + 1);
            let mut used = 1;
            match arg {
                b"--term" | b"-t" => a.backend = Backend::Term,
                b"--x11" => a.backend = Backend::X11,
                b"--fb" => a.backend = Backend::Fb,
                b"--window" => a.backend = Backend::Window,
                b"--shot" => {
                    a.shot = val;
                    used = 2;
                }
                b"--tics" => {
                    a.tics = val.and_then(parse_u32).unwrap_or(0);
                    used = 2;
                }
                b"--map" | b"-m" => {
                    a.map = val.and_then(parse_u32).map(|m| m.saturating_sub(1));
                    used = 2;
                }
                b"--skill" => {
                    a.skill = val.and_then(parse_u32).unwrap_or(3).clamp(1, 5) as u8 - 1;
                    used = 2;
                }
                b"--size" => {
                    a.size = val.and_then(parse_size);
                    used = 2;
                }
                b"--scale" => {
                    a.scale = val.and_then(parse_u32).unwrap_or(0);
                    used = 2;
                }
                b"--script" => {
                    a.script = val;
                    used = 2;
                }
                b"--colors" => {
                    a.colors = val.and_then(parse_u32).unwrap_or(0).min(999) as u16;
                    used = 2;
                }
                b"--fps" => {
                    a.fps = val.and_then(parse_u32).unwrap_or(35).clamp(1, 1000);
                    used = 2;
                }
                b"--warp" => {
                    a.warp = val.and_then(parse_triple);
                    used = 2;
                }
                b"--show" => {
                    a.show = val;
                    used = 2;
                }
                b"--bench" => a.bench = true,
                b"--nomouse" => a.nomouse = true,
                b"--fullscreen" | b"-f" => a.fullscreen = true,
                b"--help" | b"-h" => a.help = true,
                b"--version" | b"-v" => a.version = true,
                _ => {}
            }
            i += used;
        }
        a
    }
}

pub const HELP: &[u8] = b"hellbyte - a tiny libre retro shooter

usage: hellbyte [options]

  --term, -t        play in the terminal (works over SSH / serial)
  --x11             X11 window (default when $DISPLAY is set)
  --fb              Linux framebuffer + evdev input (consoles, kiosks)
  --map N, -m N     start directly on level N
  --skill 1-5       1 stroll, 2 skirmish, 3 brawl, 4 carnage, 5 inferno
  --size WxH        render resolution (max 320x200)
  --scale N         window scale factor
  --colors N        terminal colours: 256 or 24 (truecolor), 16, 2 (ascii)
  --fps N           frame rate cap (terminal default 35)
  --nomouse         don't grab the mouse
  --shot FILE.png   run headless for --tics N and save a screenshot
  --script KEYS     scripted input for --shot/--bench (see README)
  --bench           run --tics N headless and print timing + memory
  --warp X,Y,ANGLE  (testing) put the player at a map position
  --show WHAT       (testing) open: menu skill options help map title
  --version, --help
";
