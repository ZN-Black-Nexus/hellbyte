//! mapc — the Hellbyte level compiler.
//!
//! Levels are written in a small text language made of *layered polygons*:
//! every `rect`/`poly`/`ngon` paints a region of the map with a sector style,
//! later shapes are painted on top of earlier ones, and `void` shapes cut
//! solid holes. mapc resolves the layers into a planar map (vertices,
//! linedefs, sidedefs, sectors), picks wall textures from the styles on each
//! side, builds a BSP tree and a blockmap, and emits the whole level as Rust
//! `static` data that the engine links in directly (no loading, no heap).
//!
//! See `levels/README.md` for the language reference.

use std::collections::HashMap;
use std::fmt::Write as _;

/// Resolves a texture/flat name to its index (0 is reserved for "none").
pub type Lookup<'a> = &'a dyn Fn(&str) -> Option<u8>;

pub struct Textures<'a> {
    pub wall: Lookup<'a>,
    pub flat: Lookup<'a>,
    /// Flat index that marks a sky ceiling.
    pub sky_flat: u8,
}

// Line flags (kept in sync with the engine's `mapdata` module).
pub const ML_BLOCKING: u16 = 1;
pub const ML_BLOCKMONSTERS: u16 = 2;
pub const ML_TWOSIDED: u16 = 4;
pub const ML_DONTPEGTOP: u16 = 8;
pub const ML_DONTPEGBOTTOM: u16 = 16;
pub const ML_SECRET: u16 = 32;
pub const ML_SOUNDBLOCK: u16 = 64;
pub const ML_DONTDRAW: u16 = 128;
pub const ML_MAPPED: u16 = 256;

pub const NO_SIDE: u16 = 0xffff;
pub const NF_SUBSECTOR: u16 = 0x8000;

type P = (f64, f64);

#[derive(Clone, Debug)]
struct Style {
    floor: i32,
    ceil: i32,
    light: i32,
    ftex: String,
    ctex: String,
    wall: String,
    upper: Option<String>,
    lower: Option<String>,
    special: String,
    tag: i32,
    /// Line special given to lines bordering this sector (doors).
    door: Option<String>,
    /// Source line of the shape that created this sector (for errors).
    line: usize,
}

impl Default for Style {
    fn default() -> Self {
        Style {
            floor: 0,
            ceil: 128,
            light: 160,
            ftex: "FLOOR1".into(),
            ctex: "CEIL1".into(),
            wall: "BASE1".into(),
            upper: None,
            lower: None,
            special: "none".into(),
            tag: 0,
            door: None,
            line: 0,
        }
    }
}

struct Region {
    poly: Vec<P>,
    sector: Option<usize>,
}

#[derive(Clone, Default, Debug)]
struct LineProps {
    special: Option<String>,
    tag: Option<i32>,
    flags_set: u16,
    front: SideTex,
    back: SideTex,
    xoff: Option<i32>,
    yoff: Option<i32>,
    /// Keep this edge even if both sides are the same sector (walk-over triggers).
    forced: bool,
    /// Switch: put `tex` on whichever side faces into a sector.
    switch_tex: Option<String>,
}

#[derive(Clone, Default, Debug)]
struct SideTex {
    upper: Option<String>,
    mid: Option<String>,
    lower: Option<String>,
}

struct Extra {
    a: P,
    b: P,
    props: LineProps,
}

#[derive(Clone, Debug)]
struct Thing {
    kind: String,
    x: i32,
    y: i32,
    angle: i32,
    flags: u8,
}

pub struct CompiledLevel {
    pub ident: String,
    pub name: String,
    pub code: String,
    pub stats: String,
    pub sectors: usize,
    pub lines: usize,
    pub sides: usize,
    pub blocks: usize,
}

// ------------------------------------------------------------------ parsing

struct Parser {
    styles: HashMap<String, Style>,
    sectors: Vec<Style>,
    sector_names: HashMap<String, usize>,
    regions: Vec<Region>,
    extras: Vec<Extra>,
    things: Vec<Thing>,
    ident: String,
    name: String,
    sky: i32,
    par: i32,
    next_map: String,
    secret_map: String,
}

fn err<T>(line: usize, msg: impl Into<String>) -> Result<T, String> {
    Err(format!("line {}: {}", line, msg.into()))
}

fn num(line: usize, s: &str) -> Result<i32, String> {
    s.parse::<i32>().or_else(|_| err(line, format!("expected a number, got `{s}`")))
}

fn camel(s: &str) -> String {
    let mut out = String::new();
    for part in s.split('_') {
        let mut c = part.chars();
        if let Some(f) = c.next() {
            out.extend(f.to_uppercase());
            out.push_str(&c.as_str().to_lowercase());
        }
    }
    out
}

fn apply_kv(style: &mut Style, key: &str, val: &str, line: usize) -> Result<bool, String> {
    match key {
        "floor" => style.floor = num(line, val)?,
        "ceil" => style.ceil = num(line, val)?,
        "light" => style.light = num(line, val)?,
        "ftex" => style.ftex = val.to_string(),
        "ctex" => style.ctex = val.to_string(),
        "wall" => style.wall = val.to_string(),
        "upper" => style.upper = Some(val.to_string()),
        "lower" => style.lower = Some(val.to_string()),
        "special" => style.special = val.to_string(),
        "tag" => style.tag = num(line, val)?,
        "door" => style.door = Some(val.to_string()),
        _ => return Ok(false),
    }
    Ok(true)
}

impl Parser {
    fn parse(&mut self, src: &str) -> Result<(), String> {
        for (i, raw) in src.lines().enumerate() {
            let ln = i + 1;
            let text = match raw.find('#') {
                Some(p) => &raw[..p],
                None => raw,
            };
            let toks = tokenize(text);
            if toks.is_empty() {
                continue;
            }
            let (pos, kv): (Vec<&str>, Vec<(&str, &str)>) = {
                let mut pos = vec![];
                let mut kv = vec![];
                for t in &toks {
                    if let Some(eq) = t.find('=') {
                        kv.push((&t[..eq], &t[eq + 1..]));
                    } else {
                        pos.push(t.as_str());
                    }
                }
                (pos, kv)
            };
            match pos[0] {
                "map" => {
                    if pos.len() < 2 {
                        return err(ln, "usage: map IDENT \"Name\"");
                    }
                    self.ident = pos[1].to_string();
                    self.name = toks.get(2).map(|s| s.trim_matches('"').to_string()).unwrap_or_default();
                }
                "sky" => self.sky = num(ln, pos.get(1).copied().unwrap_or("0"))?,
                "par" => self.par = num(ln, pos.get(1).copied().unwrap_or("0"))?,
                "next" => self.next_map = pos.get(1).copied().unwrap_or("").to_string(),
                "secretnext" => self.secret_map = pos.get(1).copied().unwrap_or("").to_string(),
                "style" => {
                    if pos.len() < 2 {
                        return err(ln, "usage: style NAME key=value...");
                    }
                    let mut st = match kv.iter().find(|(k, _)| *k == "from") {
                        Some((_, base)) => self
                            .styles
                            .get(*base)
                            .cloned()
                            .ok_or_else(|| format!("line {ln}: unknown base style `{base}`"))?,
                        None => Style::default(),
                    };
                    for (k, v) in &kv {
                        if *k == "from" {
                            continue;
                        }
                        if !apply_kv(&mut st, k, v, ln)? {
                            return err(ln, format!("unknown style key `{k}`"));
                        }
                    }
                    self.styles.insert(pos[1].to_string(), st);
                }
                "rect" | "poly" | "ngon" => {
                    if pos.len() < 2 {
                        return err(ln, "missing style");
                    }
                    let nums: Vec<i32> = pos[2..].iter().map(|s| num(ln, s)).collect::<Result<_, _>>()?;
                    let poly: Vec<P> = match pos[0] {
                        "rect" => {
                            if nums.len() != 4 {
                                return err(ln, "rect needs x1 y1 x2 y2");
                            }
                            let (x1, y1, x2, y2) = (
                                nums[0].min(nums[2]) as f64,
                                nums[1].min(nums[3]) as f64,
                                nums[0].max(nums[2]) as f64,
                                nums[1].max(nums[3]) as f64,
                            );
                            vec![(x1, y1), (x2, y1), (x2, y2), (x1, y2)]
                        }
                        "poly" => {
                            if nums.len() < 6 || nums.len() % 2 != 0 {
                                return err(ln, "poly needs at least 3 x y pairs");
                            }
                            nums.chunks(2).map(|c| (c[0] as f64, c[1] as f64)).collect()
                        }
                        _ => {
                            if nums.len() != 4 {
                                return err(ln, "ngon needs cx cy radius sides");
                            }
                            let rot = kv
                                .iter()
                                .find(|(k, _)| *k == "rot")
                                .map(|(_, v)| num(ln, v))
                                .transpose()?
                                .unwrap_or(0) as f64;
                            let n = nums[3].max(3);
                            (0..n)
                                .map(|i| {
                                    let a = (rot + 360.0 * i as f64 / n as f64).to_radians();
                                    (
                                        (nums[0] as f64 + nums[2] as f64 * a.cos()).round(),
                                        (nums[1] as f64 + nums[2] as f64 * a.sin()).round(),
                                    )
                                })
                                .collect()
                        }
                    };
                    let sector = if pos[1] == "void" {
                        None
                    } else {
                        let mut st = self
                            .styles
                            .get(pos[1])
                            .cloned()
                            .ok_or_else(|| format!("line {ln}: unknown style `{}`", pos[1]))?;
                        let mut name = None;
                        for (k, v) in &kv {
                            match *k {
                                "as" => name = Some(v.to_string()),
                                "rot" => {}
                                _ => {
                                    if !apply_kv(&mut st, k, v, ln)? {
                                        return err(ln, format!("unknown key `{k}`"));
                                    }
                                }
                            }
                        }
                        st.line = ln;
                        Some(match name {
                            Some(n) => match self.sector_names.get(&n) {
                                Some(&idx) => idx,
                                None => {
                                    self.sectors.push(st);
                                    self.sector_names.insert(n, self.sectors.len() - 1);
                                    self.sectors.len() - 1
                                }
                            },
                            None => {
                                self.sectors.push(st);
                                self.sectors.len() - 1
                            }
                        })
                    };
                    self.regions.push(Region { poly: make_ccw(poly), sector });
                }
                "stairs" => {
                    // stairs STYLE x1 y1 x2 y2 COUNT n|s|e|w [step=8]: COUNT strips inside the
                    // rectangle, each `step` units higher than the last, climbing towards DIR.
                    if pos.len() != 8 {
                        return err(ln, "usage: stairs STYLE x1 y1 x2 y2 COUNT n|s|e|w [step=N]");
                    }
                    let base = self
                        .styles
                        .get(pos[1])
                        .cloned()
                        .ok_or_else(|| format!("line {ln}: unknown style `{}`", pos[1]))?;
                    let n: Vec<i32> = pos[2..6].iter().map(|v| num(ln, v)).collect::<Result<_, _>>()?;
                    let count = num(ln, pos[6])?.max(1);
                    let step = kv.iter().find(|(k, _)| *k == "step").map(|(_, v)| num(ln, v)).transpose()?.unwrap_or(8);
                    let (x1, y1, x2, y2) = (n[0].min(n[2]), n[1].min(n[3]), n[0].max(n[2]), n[1].max(n[3]));
                    for i in 0..count {
                        let (a, b) = |len: i32| -> (i32, i32) { (len * i / count, len * (i + 1) / count) }(match pos[7] {
                            "n" | "s" => y2 - y1,
                            _ => x2 - x1,
                        });
                        let poly = match pos[7] {
                            "n" => vec![(x1, y1 + a), (x2, y1 + a), (x2, y1 + b), (x1, y1 + b)],
                            "s" => vec![(x1, y2 - b), (x2, y2 - b), (x2, y2 - a), (x1, y2 - a)],
                            "e" => vec![(x1 + a, y1), (x1 + b, y1), (x1 + b, y2), (x1 + a, y2)],
                            "w" => vec![(x2 - b, y1), (x2 - a, y1), (x2 - a, y2), (x2 - b, y2)],
                            d => return err(ln, format!("stairs direction must be n, s, e or w (got `{d}`)")),
                        };
                        let mut st = base.clone();
                        st.floor = base.floor + step * (i + 1);
                        for (k, v) in &kv {
                            if *k != "step" && !apply_kv(&mut st, k, v, ln)? {
                                return err(ln, format!("unknown key `{k}`"));
                            }
                        }
                        st.floor = base.floor + step * (i + 1);
                        st.line = ln;
                        self.sectors.push(st);
                        let poly: Vec<P> = poly.into_iter().map(|(x, y)| (x as f64, y as f64)).collect();
                        self.regions.push(Region { poly: make_ccw(poly), sector: Some(self.sectors.len() - 1) });
                    }
                }
                "line" | "trigger" | "switch" => {
                    let nums: Vec<i32> = pos[1..].iter().map(|s| num(ln, s)).collect::<Result<_, _>>()?;
                    if nums.len() != 4 {
                        return err(ln, format!("{} needs x1 y1 x2 y2", pos[0]));
                    }
                    let mut props = LineProps { forced: pos[0] == "trigger", ..Default::default() };
                    if pos[0] == "switch" {
                        props.switch_tex = Some("SWITCH0".into());
                    }
                    for (k, v) in &kv {
                        match *k {
                            "special" => props.special = Some(v.to_string()),
                            "tag" => props.tag = Some(num(ln, v)?),
                            "tex" => props.switch_tex = Some(v.to_string()),
                            "xoff" => props.xoff = Some(num(ln, v)?),
                            "yoff" => props.yoff = Some(num(ln, v)?),
                            "flags" => {
                                for f in v.split(',') {
                                    props.flags_set |= match f {
                                        "impassable" => ML_BLOCKING,
                                        "blockmonsters" => ML_BLOCKMONSTERS,
                                        "upperunpegged" => ML_DONTPEGTOP,
                                        "lowerunpegged" => ML_DONTPEGBOTTOM,
                                        "secret" => ML_SECRET,
                                        "blocksound" => ML_SOUNDBLOCK,
                                        "hidden" => ML_DONTDRAW,
                                        "mapped" => ML_MAPPED,
                                        _ => return err(ln, format!("unknown line flag `{f}`")),
                                    };
                                }
                            }
                            "upper" => {
                                props.front.upper = Some(v.to_string());
                                props.back.upper = Some(v.to_string())
                            }
                            "mid" => {
                                props.front.mid = Some(v.to_string());
                                props.back.mid = Some(v.to_string())
                            }
                            "lower" => {
                                props.front.lower = Some(v.to_string());
                                props.back.lower = Some(v.to_string())
                            }
                            "front.upper" => props.front.upper = Some(v.to_string()),
                            "front.mid" => props.front.mid = Some(v.to_string()),
                            "front.lower" => props.front.lower = Some(v.to_string()),
                            "back.upper" => props.back.upper = Some(v.to_string()),
                            "back.mid" => props.back.mid = Some(v.to_string()),
                            "back.lower" => props.back.lower = Some(v.to_string()),
                            _ => return err(ln, format!("unknown line key `{k}`")),
                        }
                    }
                    self.extras.push(Extra {
                        a: (nums[0] as f64, nums[1] as f64),
                        b: (nums[2] as f64, nums[3] as f64),
                        props,
                    });
                }
                "thing" => {
                    if pos.len() < 4 {
                        return err(ln, "usage: thing KIND x y [angle] [easy normal hard ambush]");
                    }
                    let x = num(ln, pos[2])?;
                    let y = num(ln, pos[3])?;
                    let mut angle = 0;
                    let mut skill = 0u8;
                    let mut other = 0u8;
                    for t in &pos[4..] {
                        match *t {
                            "easy" => skill |= 1,
                            "normal" => skill |= 2,
                            "hard" => skill |= 4,
                            "ambush" => other |= 8,
                            "multi" => other |= 16,
                            n => angle = num(ln, n)?,
                        }
                    }
                    if skill == 0 {
                        skill = 7;
                    }
                    self.things.push(Thing { kind: pos[1].to_string(), x, y, angle, flags: skill | other });
                }
                other => return err(ln, format!("unknown command `{other}`")),
            }
        }
        if self.ident.is_empty() {
            return Err("missing `map` line".into());
        }
        Ok(())
    }
}

fn tokenize(s: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut quoted = false;
    for c in s.chars() {
        if c == '"' {
            quoted = !quoted;
            cur.push(c);
        } else if (c.is_whitespace() || c == ',') && !quoted && !cur.contains('=') || (c.is_whitespace() && !quoted) {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else {
            cur.push(c);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn area2(poly: &[P]) -> f64 {
    let mut a = 0.0;
    for i in 0..poly.len() {
        let (x1, y1) = poly[i];
        let (x2, y2) = poly[(i + 1) % poly.len()];
        a += x1 * y2 - x2 * y1;
    }
    a
}

fn make_ccw(mut poly: Vec<P>) -> Vec<P> {
    if area2(&poly) < 0.0 {
        poly.reverse();
    }
    poly
}

fn point_in_poly(p: P, poly: &[P]) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if (yi > p.1) != (yj > p.1) && p.0 < (xj - xi) * (p.1 - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

// ------------------------------------------------------------------ planar arrangement

struct VertPool {
    pts: Vec<P>,
    map: HashMap<(i64, i64), usize>,
}

impl VertPool {
    fn get(&mut self, p: P) -> usize {
        // Snap to 1/64 map unit so shared corners of different shapes merge.
        let key = ((p.0 * 64.0).round() as i64, (p.1 * 64.0).round() as i64);
        if let Some(&i) = self.map.get(&key) {
            return i;
        }
        self.pts.push((key.0 as f64 / 64.0, key.1 as f64 / 64.0));
        self.map.insert(key, self.pts.len() - 1);
        self.pts.len() - 1
    }
}

fn cross(a: P, b: P) -> f64 {
    a.0 * b.1 - a.1 * b.0
}
fn sub(a: P, b: P) -> P {
    (a.0 - b.0, a.1 - b.1)
}

#[derive(Clone)]
struct Line {
    v1: usize,
    v2: usize,
    front: usize,
    back: Option<usize>,
    flags: u16,
    special: String,
    tag: i32,
    props: Vec<usize>, // indices into extras that cover this line
}

struct Side {
    xoff: i32,
    yoff: i32,
    top: u8,
    mid: u8,
    bottom: u8,
    sector: usize,
}

// ------------------------------------------------------------------ BSP

#[derive(Clone)]
struct BSeg {
    a: usize,
    b: usize,
    line: usize,
    side: u8,
    offset: f64,
}

struct Bsp<'v> {
    verts: &'v mut Vec<P>,
    segs_out: Vec<BSeg>,
    subsectors: Vec<(usize, usize, usize)>,
    nodes: Vec<(P, P, [[f64; 4]; 2], [u16; 2])>,
    line_sectors: Vec<(usize, Option<usize>)>,
}

const EPS: f64 = 1e-4;

fn side_of(p: P, o: P, d: P) -> f64 {
    let len = (d.0 * d.0 + d.1 * d.1).sqrt();
    cross(d, sub(p, o)) / len
}

impl Bsp<'_> {
    fn classify(&self, s: &BSeg, o: P, d: P) -> (i32, f64, f64) {
        // -1 front (right), 1 back (left), 0 split, 2 collinear
        let ca = side_of(self.verts[s.a], o, d);
        let cb = side_of(self.verts[s.b], o, d);
        let fa = ca < -EPS;
        let ba = ca > EPS;
        let fb = cb < -EPS;
        let bb = cb > EPS;
        if !fa && !ba && !fb && !bb {
            return (2, ca, cb);
        }
        if (fa || !ba) && (fb || !bb) && (fa || fb) {
            return (-1, ca, cb);
        }
        if (ba || !fa) && (bb || !fb) && (ba || bb) {
            return (1, ca, cb);
        }
        (0, ca, cb)
    }

    fn seg_dir(&self, s: &BSeg) -> P {
        sub(self.verts[s.b], self.verts[s.a])
    }

    fn is_convex(&self, segs: &[BSeg]) -> bool {
        for s in segs {
            let o = self.verts[s.a];
            let d = self.seg_dir(s);
            for t in segs {
                let (c, _, _) = self.classify(t, o, d);
                if c == 1 || c == 0 {
                    return false;
                }
            }
        }
        true
    }

    fn bbox(&self, segs: &[BSeg]) -> [f64; 4] {
        let (mut t, mut b, mut l, mut r) = (f64::MIN, f64::MAX, f64::MAX, f64::MIN);
        for s in segs {
            for &v in &[s.a, s.b] {
                let (x, y) = self.verts[v];
                t = t.max(y);
                b = b.min(y);
                l = l.min(x);
                r = r.max(x);
            }
        }
        [t, b, l, r]
    }

    fn build(&mut self, segs: Vec<BSeg>) -> u16 {
        if self.is_convex(&segs) {
            let first = self.segs_out.len();
            let sector = {
                let s = &segs[0];
                let (f, b) = self.line_sectors[s.line];
                if s.side == 0 { f } else { b.unwrap() }
            };
            let n = segs.len();
            self.segs_out.extend(segs);
            self.subsectors.push((first, n, sector));
            return (self.subsectors.len() - 1) as u16 | NF_SUBSECTOR;
        }
        // Choose the partition that splits least and balances best.
        let mut best: Option<(i64, usize)> = None;
        let step = if segs.len() > 400 { segs.len() / 200 } else { 1 };
        for step in [step, 1] {
        if best.is_some() {
            break;
        }
        for (i, p) in segs.iter().enumerate().step_by(step.max(1)) {
            let o = self.verts[p.a];
            let d = self.seg_dir(p);
            let (mut front, mut back, mut splits) = (0i64, 0i64, 0i64);
            for t in &segs {
                match self.classify(t, o, d).0 {
                    -1 => front += 1,
                    1 => back += 1,
                    0 => splits += 1,
                    _ => {
                        let td = self.seg_dir(t);
                        if td.0 * d.0 + td.1 * d.1 > 0.0 {
                            front += 1
                        } else {
                            back += 1
                        }
                    }
                }
            }
            if back == 0 && splits == 0 {
                continue;
            }
            let score = splits * 8 + (front - back).abs();
            if best.map_or(true, |(s, _)| score < s) {
                best = Some((score, i));
            }
        }
        }
        let pi = best.expect("non-convex seg set without a usable partition").1;
        let o = self.verts[segs[pi].a];
        let d = self.seg_dir(&segs[pi]);
        let mut fr = vec![];
        let mut bk = vec![];
        for s in segs {
            let (c, ca, cb) = self.classify(&s, o, d);
            match c {
                -1 => fr.push(s),
                1 => bk.push(s),
                2 => {
                    let sd = self.seg_dir(&s);
                    if sd.0 * d.0 + sd.1 * d.1 > 0.0 {
                        fr.push(s)
                    } else {
                        bk.push(s)
                    }
                }
                _ => {
                    let t = ca / (ca - cb);
                    let (ax, ay) = self.verts[s.a];
                    let (bx, by) = self.verts[s.b];
                    let np = (ax + t * (bx - ax), ay + t * (by - ay));
                    self.verts.push(np);
                    let nv = self.verts.len() - 1;
                    let first_len = ((np.0 - ax).powi(2) + (np.1 - ay).powi(2)).sqrt();
                    let s1 = BSeg { a: s.a, b: nv, line: s.line, side: s.side, offset: s.offset };
                    let s2 = BSeg { a: nv, b: s.b, line: s.line, side: s.side, offset: s.offset + first_len };
                    if ca < 0.0 {
                        fr.push(s1);
                        bk.push(s2);
                    } else {
                        bk.push(s1);
                        fr.push(s2);
                    }
                }
            }
        }
        let bb = [self.bbox(&fr), self.bbox(&bk)];
        let idx = self.nodes.len();
        self.nodes.push((o, d, bb, [0, 0]));
        let c0 = self.build(fr);
        let c1 = self.build(bk);
        self.nodes[idx].3 = [c0, c1];
        idx as u16
    }
}

// ------------------------------------------------------------------ compile

pub fn compile(src: &str, tex: &Textures) -> Result<CompiledLevel, String> {
    let mut p = Parser {
        styles: HashMap::new(),
        sectors: vec![],
        sector_names: HashMap::new(),
        regions: vec![],
        extras: vec![],
        things: vec![],
        ident: String::new(),
        name: String::new(),
        sky: 0,
        par: 0,
        next_map: String::new(),
        secret_map: String::new(),
    };
    p.parse(src)?;

    // 1. collect every edge (region outlines + extra lines)
    let mut pool = VertPool { pts: vec![], map: HashMap::new() };
    struct RawSeg {
        a: P,
        b: P,
        extra: Option<usize>,
    }
    let mut raw = vec![];
    for r in &p.regions {
        for i in 0..r.poly.len() {
            raw.push(RawSeg { a: r.poly[i], b: r.poly[(i + 1) % r.poly.len()], extra: None });
        }
    }
    for (i, e) in p.extras.iter().enumerate() {
        raw.push(RawSeg { a: e.a, b: e.b, extra: Some(i) });
    }

    // 2. split all edges at mutual intersections / touching vertices
    let mut cuts: Vec<Vec<f64>> = raw.iter().map(|_| vec![0.0, 1.0]).collect();
    for i in 0..raw.len() {
        let (a, b) = (raw[i].a, raw[i].b);
        let (minx, maxx) = (a.0.min(b.0) - 0.01, a.0.max(b.0) + 0.01);
        let (miny, maxy) = (a.1.min(b.1) - 0.01, a.1.max(b.1) + 0.01);
        for j in i + 1..raw.len() {
            let (c, d) = (raw[j].a, raw[j].b);
            if c.0.max(d.0) < minx || c.0.min(d.0) > maxx || c.1.max(d.1) < miny || c.1.min(d.1) > maxy {
                continue;
            }
            let r = sub(b, a);
            let s = sub(d, c);
            let den = cross(r, s);
            let rl = (r.0 * r.0 + r.1 * r.1).sqrt();
            let sl = (s.0 * s.0 + s.1 * s.1).sqrt();
            if den.abs() < 1e-9 * rl * sl {
                // parallel: if collinear, each segment is cut by the other's endpoints
                if (cross(r, sub(c, a)) / rl).abs() > 1e-3 {
                    continue;
                }
                for (q, k, oa, orr) in [(c, i, a, r), (d, i, a, r), (a, j, c, s), (b, j, c, s)] {
                    let t = ((q.0 - oa.0) * orr.0 + (q.1 - oa.1) * orr.1) / (orr.0 * orr.0 + orr.1 * orr.1);
                    if t > 1e-9 && t < 1.0 - 1e-9 {
                        cuts[k].push(t);
                    }
                }
                continue;
            }
            let t = cross(sub(c, a), s) / den;
            let u = cross(sub(c, a), r) / den;
            let tol_t = 1e-3 / rl;
            let tol_u = 1e-3 / sl;
            if t < -tol_t || t > 1.0 + tol_t || u < -tol_u || u > 1.0 + tol_u {
                continue;
            }
            cuts[i].push(t.clamp(0.0, 1.0));
            cuts[j].push(u.clamp(0.0, 1.0));
        }
    }

    // 3. unique edges
    let mut edges: HashMap<(usize, usize), (usize, usize, Vec<usize>)> = HashMap::new();
    let mut edge_order = vec![];
    for (i, rs) in raw.iter().enumerate() {
        let mut ts = cuts[i].clone();
        ts.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let mut vs: Vec<usize> = vec![];
        for t in ts {
            let v = pool.get((rs.a.0 + t * (rs.b.0 - rs.a.0), rs.a.1 + t * (rs.b.1 - rs.a.1)));
            if vs.last() != Some(&v) {
                vs.push(v);
            }
        }
        for w in vs.windows(2) {
            let key = (w[0].min(w[1]), w[0].max(w[1]));
            let e = edges.entry(key).or_insert_with(|| {
                edge_order.push(key);
                (w[0], w[1], vec![])
            });
            if let Some(x) = rs.extra {
                e.2.push(x);
            }
        }
    }

    // 4. classify both sides of every edge by the top-most covering region
    let lookup = |pt: P| -> Option<usize> {
        for r in p.regions.iter().rev() {
            if point_in_poly(pt, &r.poly) {
                return r.sector;
            }
        }
        None
    };
    let mut lines: Vec<Line> = vec![];
    for key in &edge_order {
        let (va, vb, extras) = edges[key].clone();
        let a = pool.pts[va];
        let b = pool.pts[vb];
        let d = sub(b, a);
        let len = (d.0 * d.0 + d.1 * d.1).sqrt();
        if len < 1e-6 {
            continue;
        }
        let mid = ((a.0 + b.0) * 0.5, (a.1 + b.1) * 0.5);
        let n = (d.1 / len * 0.05, -d.0 / len * 0.05); // right-hand normal
        let right = lookup((mid.0 + n.0, mid.1 + n.1));
        let left = lookup((mid.0 - n.0, mid.1 - n.1));
        let forced = extras.iter().any(|&x| p.extras[x].props.forced);
        let (v1, v2, front, back) = match (right, left) {
            (None, None) => continue,
            (Some(r), None) => (va, vb, r, None),
            (None, Some(l)) => (vb, va, l, None),
            (Some(r), Some(l)) => {
                if r == l && !forced {
                    continue;
                }
                (va, vb, r, Some(l))
            }
        };
        lines.push(Line {
            v1,
            v2,
            front,
            back,
            flags: 0,
            special: "none".into(),
            tag: 0,
            props: extras,
        });
    }

    // 5. line specials & flags from styles and extra commands
    for l in lines.iter_mut() {
        if l.back.is_none() {
            l.flags |= ML_BLOCKING;
            if p.sectors[l.front].door.is_some() {
                l.flags |= ML_DONTPEGBOTTOM; // door tracks stay put
            }
        } else {
            l.flags |= ML_TWOSIDED;
            let f = &p.sectors[l.front];
            let b = &p.sectors[l.back.unwrap()];
            if l.front != l.back.unwrap() {
                // The special (and tag) of a "door"-style sector goes on every
                // line around it: doors open from either side, lifts lower when used.
                let owner = if b.door.is_some() { Some(b) } else if f.door.is_some() { Some(f) } else { None };
                if let Some(o) = owner {
                    l.special = o.door.clone().unwrap();
                    l.tag = o.tag;
                }
                // As in the classic format, the door is the line's back side and
                // is used from the front, so turn lines whose door is in front.
                if b.door.is_none() && f.door.is_some() {
                    let front = l.front;
                    l.front = l.back.unwrap();
                    l.back = Some(front);
                    core::mem::swap(&mut l.v1, &mut l.v2);
                }
            }
        }
        for &x in &l.props {
            let pr = &p.extras[x].props;
            if let Some(s) = &pr.special {
                l.special = s.clone();
            }
            if let Some(t) = pr.tag {
                l.tag = t;
            }
            l.flags |= pr.flags_set;
        }
    }

    // 6. merge collinear chains that are otherwise identical
    loop {
        let mut uses: HashMap<usize, Vec<usize>> = HashMap::new();
        for (i, l) in lines.iter().enumerate() {
            uses.entry(l.v1).or_default().push(i);
            uses.entry(l.v2).or_default().push(i);
        }
        let mut merged = false;
        let mut dead = vec![false; lines.len()];
        for (&v, ls) in &uses {
            if ls.len() != 2 {
                continue;
            }
            let (i, j) = (ls[0], ls[1]);
            if dead[i] || dead[j] {
                continue;
            }
            let (x, y) = if lines[i].v2 == v && lines[j].v1 == v {
                (i, j)
            } else if lines[j].v2 == v && lines[i].v1 == v {
                (j, i)
            } else {
                continue;
            };
            let (lx, ly) = (&lines[x], &lines[y]);
            if lx.front != ly.front
                || lx.back != ly.back
                || lx.flags != ly.flags
                || lx.special != ly.special
                || lx.tag != ly.tag
                || lx.props != ly.props
            {
                continue;
            }
            let d1 = sub(pool.pts[lx.v2], pool.pts[lx.v1]);
            let d2 = sub(pool.pts[ly.v2], pool.pts[ly.v1]);
            let l1 = (d1.0 * d1.0 + d1.1 * d1.1).sqrt();
            let l2 = (d2.0 * d2.0 + d2.1 * d2.1).sqrt();
            if (cross(d1, d2) / (l1 * l2)).abs() > 1e-6 || d1.0 * d2.0 + d1.1 * d2.1 <= 0.0 {
                continue;
            }
            lines[x].v2 = lines[y].v2;
            dead[y] = true;
            merged = true;
        }
        if !merged {
            break;
        }
        let mut k = 0;
        lines.retain(|_| {
            k += 1;
            !dead[k - 1]
        });
    }

    // 7. sidedefs: pick textures from the styles on both sides
    let wall = |name: &str, what: &str| -> Result<u8, String> {
        if name == "-" {
            return Ok(0);
        }
        (tex.wall)(name).ok_or_else(|| format!("{}: unknown wall texture `{name}` ({what})", p.ident))
    };
    let mut sides: Vec<Side> = vec![];
    let mut line_sides: Vec<[u16; 2]> = vec![];
    for l in &lines {
        let mut ids = [NO_SIDE, NO_SIDE];
        let pairs = [(l.front, l.back), (l.back.unwrap_or(usize::MAX), Some(l.front))];
        for (k, &(me, other)) in pairs.iter().enumerate() {
            if me == usize::MAX {
                continue;
            }
            let ms = &p.sectors[me];
            let (mut top, mut mid, mut bottom) = (0u8, 0u8, 0u8);
            match other {
                None => mid = wall(&ms.wall, "wall")?,
                Some(o) => {
                    let os = &p.sectors[o];
                    // Always fill uppers/lowers: doors and lifts expose them when they move.
                    top = wall(os.upper.as_deref().unwrap_or(&ms.wall), "upper")?;
                    bottom = wall(os.lower.as_deref().unwrap_or(&ms.wall), "lower")?;
                }
            }
            let (mut xoff, mut yoff) = (0, 0);
            for &x in &l.props {
                let pr = &p.extras[x].props;
                let st = if k == 0 { &pr.front } else { &pr.back };
                if let Some(t) = &st.upper {
                    top = wall(t, "line upper")?;
                }
                if let Some(t) = &st.mid {
                    mid = wall(t, "line mid")?;
                }
                if let Some(t) = &st.lower {
                    bottom = wall(t, "line lower")?;
                }
                if let Some(t) = &pr.switch_tex {
                    // The switch face goes on the side the player can reach.
                    if other.is_none() {
                        mid = wall(t, "switch")?;
                    } else {
                        let os = &p.sectors[other.unwrap()];
                        if os.floor > ms.floor {
                            bottom = wall(t, "switch")?;
                        } else {
                            top = wall(t, "switch")?;
                        }
                    }
                }
                xoff = pr.xoff.unwrap_or(xoff);
                yoff = pr.yoff.unwrap_or(yoff);
            }
            sides.push(Side { xoff, yoff, top, mid, bottom, sector: me });
            ids[k] = (sides.len() - 1) as u16;
        }
        line_sides.push(ids);
    }

    // Align textures along connected runs of wall so seams line up.
    {
        let mut by_start: HashMap<usize, Vec<usize>> = HashMap::new();
        for (i, l) in lines.iter().enumerate() {
            by_start.entry(l.v1).or_default().push(i);
        }
        let mut has_prev = vec![false; lines.len()];
        for l in &lines {
            if let Some(n) = by_start.get(&l.v2) {
                for &j in n {
                    has_prev[j] = true;
                }
            }
        }
        let mut done = vec![false; lines.len()];
        let order: Vec<usize> = (0..lines.len()).filter(|&i| !has_prev[i]).chain(0..lines.len()).collect();
        for start in order {
            if done[start] {
                continue;
            }
            let mut acc = 0.0f64;
            let mut cur = start;
            loop {
                done[cur] = true;
                let fs = line_sides[cur][0] as usize;
                if sides[fs].xoff == 0 {
                    sides[fs].xoff = (acc.round() as i32).rem_euclid(256);
                }
                let d = sub(pool.pts[lines[cur].v2], pool.pts[lines[cur].v1]);
                acc += (d.0 * d.0 + d.1 * d.1).sqrt();
                let next = by_start
                    .get(&lines[cur].v2)
                    .and_then(|n| n.iter().copied().find(|&j| !done[j] && lines[j].front == lines[cur].front));
                match next {
                    Some(n) => cur = n,
                    None => break,
                }
            }
        }
    }

    // 8. BSP
    let mut verts = pool.pts.clone();
    let line_sectors: Vec<(usize, Option<usize>)> = lines.iter().map(|l| (l.front, l.back)).collect();
    let mut segs = vec![];
    for (i, l) in lines.iter().enumerate() {
        segs.push(BSeg { a: l.v1, b: l.v2, line: i, side: 0, offset: 0.0 });
        if l.back.is_some() {
            segs.push(BSeg { a: l.v2, b: l.v1, line: i, side: 1, offset: 0.0 });
        }
    }
    if segs.is_empty() {
        return Err(format!("{}: level has no walls", p.ident));
    }
    let mut bsp = Bsp { verts: &mut verts, segs_out: vec![], subsectors: vec![], nodes: vec![], line_sectors };
    let root = bsp.build(segs);
    let Bsp { segs_out, subsectors, nodes, .. } = bsp;

    // 9. blockmap (128-unit cells)
    let (mut minx, mut miny, mut maxx, mut maxy) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for l in &lines {
        for v in [l.v1, l.v2] {
            let (x, y) = verts[v];
            minx = minx.min(x);
            miny = miny.min(y);
            maxx = maxx.max(x);
            maxy = maxy.max(y);
        }
    }
    let bx = minx.floor() as i32 - 8;
    let by = miny.floor() as i32 - 8;
    let bw = ((maxx - bx as f64) / 128.0).floor() as i32 + 1;
    let bh = ((maxy - by as f64) / 128.0).floor() as i32 + 1;
    let mut cells: Vec<Vec<u16>> = vec![vec![]; (bw * bh) as usize];
    for (li, l) in lines.iter().enumerate() {
        let (x1, y1) = verts[l.v1];
        let (x2, y2) = verts[l.v2];
        let cx1 = ((x1.min(x2) - bx as f64) / 128.0).floor() as i32;
        let cx2 = ((x1.max(x2) - bx as f64) / 128.0).floor() as i32;
        let cy1 = ((y1.min(y2) - by as f64) / 128.0).floor() as i32;
        let cy2 = ((y1.max(y2) - by as f64) / 128.0).floor() as i32;
        for cy in cy1..=cy2 {
            for cx in cx1..=cx2 {
                let (l0, b0) = (bx as f64 + cx as f64 * 128.0, by as f64 + cy as f64 * 128.0);
                let corners = [(l0, b0), (l0 + 128.0, b0), (l0, b0 + 128.0), (l0 + 128.0, b0 + 128.0)];
                let d = (x2 - x1, y2 - y1);
                let s: Vec<f64> = corners.iter().map(|c| cross(d, sub(*c, (x1, y1)))).collect();
                let all_pos = s.iter().all(|&v| v > 0.0);
                let all_neg = s.iter().all(|&v| v < 0.0);
                if !(all_pos || all_neg) {
                    cells[(cy * bw + cx) as usize].push(li as u16);
                }
            }
        }
    }

    // sector -> lines
    let mut sector_lines: Vec<Vec<u16>> = vec![vec![]; p.sectors.len()];
    for (i, l) in lines.iter().enumerate() {
        sector_lines[l.front].push(i as u16);
        if let Some(b) = l.back {
            if b != l.front {
                sector_lines[b].push(i as u16);
            }
        }
    }

    // A shape hidden under later shapes leaves a sector without lines.
    for (si, ls) in sector_lines.iter().enumerate() {
        if ls.is_empty() {
            return Err(format!(
                "{}: line {}: this shape is completely covered by shapes drawn after it (move it further down)",
                p.ident, p.sectors[si].line
            ));
        }
    }

    // player start sanity
    if !p.things.iter().any(|t| t.kind == "player1") {
        return Err(format!("{}: no player1 start", p.ident));
    }
    for t in &p.things {
        if lookup((t.x as f64 + 0.25, t.y as f64 + 0.25)).is_none() {
            return Err(format!("{}: thing {} at ({}, {}) is outside the map", p.ident, t.kind, t.x, t.y));
        }
    }

    // 10. emit Rust
    let fx = |v: f64| (v * 65536.0).round() as i64;
    let flat = |name: &str| -> Result<u8, String> {
        if name == "SKY" {
            return Ok(tex.sky_flat);
        }
        (tex.flat)(name).ok_or_else(|| format!("{}: unknown flat `{name}`", p.ident))
    };
    let id = p.ident.to_uppercase();
    let mut c = String::new();
    writeln!(c, "pub static MAP_{id}: MapData = MapData {{").unwrap();
    writeln!(c, "    id: {:?}, name: {:?}, sky: {}, par: {}, next: {:?}, secret_next: {:?},", p.ident, p.name, p.sky, p.par, p.next_map, p.secret_map).unwrap();
    c.push_str("    vertices: &[");
    for (x, y) in &verts {
        write!(c, "v({},{}),", fx(*x), fx(*y)).unwrap();
    }
    c.push_str("],\n    sectors: &[");
    for s in &p.sectors {
        write!(
            c,
            "sd({},{},{},{},{},SectorSpecial::{},{}),",
            s.floor,
            s.ceil,
            flat(&s.ftex)?,
            flat(&s.ctex)?,
            s.light.clamp(0, 255),
            camel(&s.special),
            s.tag
        )
        .unwrap();
    }
    c.push_str("],\n    sides: &[");
    for s in &sides {
        write!(c, "si({},{},{},{},{},{}),", s.xoff, s.yoff, s.top, s.mid, s.bottom, s.sector).unwrap();
    }
    c.push_str("],\n    lines: &[");
    for (i, l) in lines.iter().enumerate() {
        write!(
            c,
            "ld({},{},{},LineSpecial::{},{},{},{}),",
            l.v1,
            l.v2,
            l.flags,
            camel(&l.special),
            l.tag,
            line_sides[i][0],
            line_sides[i][1]
        )
        .unwrap();
    }
    c.push_str("],\n    segs: &[");
    for s in &segs_out {
        let (ax, ay) = verts[s.a];
        let (bx2, by2) = verts[s.b];
        let ang = (by2 - ay).atan2(bx2 - ax);
        let bam = ((ang / (2.0 * std::f64::consts::PI)).rem_euclid(1.0) * 65536.0).round() as u32 & 0xffff;
        write!(c, "sg({},{},{},{},{},{}),", s.a, s.b, bam, s.line, s.side, fx(s.offset)).unwrap();
    }
    c.push_str("],\n    subsectors: &[");
    for (f, n, s) in &subsectors {
        write!(c, "ss({f},{n},{s}),").unwrap();
    }
    c.push_str("],\n    nodes: &[");
    for (o, d, bb, ch) in &nodes {
        let b = |v: [f64; 4]| {
            format!("[{},{},{},{}]", v[0].ceil() as i32, v[1].floor() as i32, v[2].floor() as i32, v[3].ceil() as i32)
        };
        write!(c, "nd({},{},{},{},{},{},{},{}),", fx(o.0), fx(o.1), fx(d.0), fx(d.1), b(bb[0]), b(bb[1]), ch[0], ch[1]).unwrap();
    }
    writeln!(c, "],\n    root: {root},").unwrap();
    c.push_str("    things: &[");
    for t in &p.things {
        write!(c, "th({},{},{},ThingKind::{},{}),", t.x, t.y, t.angle.rem_euclid(360), camel(&t.kind), t.flags).unwrap();
    }
    c.push_str("],\n");
    let mut offs = vec![];
    let mut list = vec![];
    for cell in &cells {
        offs.push(list.len());
        list.extend_from_slice(cell);
        list.push(0xffff);
    }
    write!(c, "    blockmap: BlockmapDef {{ x: {bx}, y: {by}, w: {bw}, h: {bh}, offsets: &{:?}, lines: &{:?} }},\n", offs, list).unwrap();
    let mut soffs = vec![];
    let mut slist = vec![];
    for sl in &sector_lines {
        soffs.push(slist.len() as u16);
        slist.extend_from_slice(sl);
    }
    soffs.push(slist.len() as u16);
    writeln!(c, "    sector_line_start: &{soffs:?},\n    sector_lines: &{slist:?},\n}};").unwrap();

    let stats = format!(
        "{}: {} vertices, {} lines, {} sides, {} sectors, {} segs, {} subsectors, {} nodes, {} things, blockmap {}x{}",
        p.ident,
        verts.len(),
        lines.len(),
        sides.len(),
        p.sectors.len(),
        segs_out.len(),
        subsectors.len(),
        nodes.len(),
        p.things.len(),
        bw,
        bh
    );
    Ok(CompiledLevel {
        ident: id,
        name: p.name,
        code: c,
        stats,
        sectors: p.sectors.len(),
        lines: lines.len(),
        sides: sides.len(),
        blocks: (bw * bh) as usize,
    })
}
