//! Everything baked in at build time: trig tables, palette, light maps,
//! textures and the compiled levels (see `build.rs`).

use crate::fixed::Fixed;
use crate::info::{LineSpecial, SectorSpecial, ThingKind};

#[derive(Clone, Copy)]
pub struct Vertex {
    pub x: Fixed,
    pub y: Fixed,
}

pub struct SectorDef {
    pub floor: i16,
    pub ceil: i16,
    pub floorpic: u8,
    pub ceilpic: u8,
    pub light: u8,
    pub special: SectorSpecial,
    pub tag: u16,
}

pub struct SideDef {
    pub xoff: i16,
    pub yoff: i16,
    pub top: u8,
    pub mid: u8,
    pub bottom: u8,
    pub sector: u16,
}

pub struct LineDef {
    pub v1: u16,
    pub v2: u16,
    pub flags: u16,
    pub special: LineSpecial,
    pub tag: u16,
    /// Front and back sidedef; back is `NO_SIDE` for one-sided walls.
    pub side: [u16; 2],
}

pub struct Seg {
    pub v1: u16,
    pub v2: u16,
    /// Direction as a binary angle >> 16.
    pub angle: u16,
    pub line: u16,
    pub side: u8,
    /// Distance from the linedef start to the seg start (texture offset).
    pub offset: Fixed,
}

pub struct SubSector {
    pub first: u16,
    pub count: u16,
    pub sector: u16,
}

pub struct Node {
    pub x: Fixed,
    pub y: Fixed,
    pub dx: Fixed,
    pub dy: Fixed,
    /// Bounding boxes of the front and back child: [top, bottom, left, right].
    pub bbox: [[i16; 4]; 2],
    pub child: [u16; 2],
}

pub struct ThingDef {
    pub x: i16,
    pub y: i16,
    pub angle: u16,
    pub kind: ThingKind,
    pub flags: u8,
}

pub struct BlockmapDef {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub offsets: &'static [u32],
    pub lines: &'static [u16],
}

pub struct MapData {
    pub id: &'static str,
    pub name: &'static str,
    pub sky: i32,
    pub par: i32,
    pub next: &'static str,
    pub secret_next: &'static str,
    pub vertices: &'static [Vertex],
    pub sectors: &'static [SectorDef],
    pub sides: &'static [SideDef],
    pub lines: &'static [LineDef],
    pub segs: &'static [Seg],
    pub subsectors: &'static [SubSector],
    pub nodes: &'static [Node],
    pub root: u16,
    pub things: &'static [ThingDef],
    pub blockmap: BlockmapDef,
    pub sector_line_start: &'static [u16],
    pub sector_lines: &'static [u16],
}

/// Handle to one of the built-in maps. A plain index (not a reference) so
/// that an all-zero engine state is valid; dereferences to the map data.
#[derive(Clone, Copy)]
pub struct MapRef(pub u8);

impl core::ops::Deref for MapRef {
    type Target = MapData;
    fn deref(&self) -> &MapData {
        MAPS[self.0 as usize]
    }
}

impl MapRef {
    pub fn get(self) -> &'static MapData {
        MAPS[self.0 as usize]
    }
}

pub const NO_SIDE: u16 = 0xffff;
pub const NF_SUBSECTOR: u16 = 0x8000;

// Line flags (mirrors mapc).
pub const ML_BLOCKING: u16 = 1;
pub const ML_BLOCKMONSTERS: u16 = 2;
pub const ML_TWOSIDED: u16 = 4;
pub const ML_DONTPEGTOP: u16 = 8;
pub const ML_DONTPEGBOTTOM: u16 = 16;
pub const ML_SECRET: u16 = 32;
pub const ML_SOUNDBLOCK: u16 = 64;
pub const ML_DONTDRAW: u16 = 128;
pub const ML_MAPPED: u16 = 256;

pub struct WallTex {
    pub w: u16,
    pub h: u16,
    pub off: u32,
}

const fn v(x: i64, y: i64) -> Vertex {
    Vertex { x: x as Fixed, y: y as Fixed }
}
const fn sd(floor: i16, ceil: i16, floorpic: u8, ceilpic: u8, light: u8, special: SectorSpecial, tag: u16) -> SectorDef {
    SectorDef { floor, ceil, floorpic, ceilpic, light, special, tag }
}
const fn si(xoff: i16, yoff: i16, top: u8, mid: u8, bottom: u8, sector: u16) -> SideDef {
    SideDef { xoff, yoff, top, mid, bottom, sector }
}
const fn ld(v1: u16, v2: u16, flags: u16, special: LineSpecial, tag: u16, s0: u16, s1: u16) -> LineDef {
    LineDef { v1, v2, flags, special, tag, side: [s0, s1] }
}
const fn sg(v1: u16, v2: u16, angle: u16, line: u16, side: u8, offset: i64) -> Seg {
    Seg { v1, v2, angle, line, side, offset: offset as Fixed }
}
const fn ss(first: u16, count: u16, sector: u16) -> SubSector {
    SubSector { first, count, sector }
}
#[allow(clippy::too_many_arguments)]
const fn nd(x: i64, y: i64, dx: i64, dy: i64, b0: [i16; 4], b1: [i16; 4], c0: u16, c1: u16) -> Node {
    Node { x: x as Fixed, y: y as Fixed, dx: dx as Fixed, dy: dy as Fixed, bbox: [b0, b1], child: [c0, c1] }
}
const fn th(x: i16, y: i16, angle: u16, kind: ThingKind, flags: u8) -> ThingDef {
    ThingDef { x, y, angle, kind, flags }
}

include!(concat!(env!("OUT_DIR"), "/generated.rs"));
include!(concat!(env!("OUT_DIR"), "/maps.rs"));

pub fn wall_column(tex: u8, col: i32) -> &'static [u8] {
    let t = &WALLS[tex as usize];
    let c = (col & (t.w as i32 - 1)) as usize;
    let start = t.off as usize + c * t.h as usize;
    &WALL_PIXELS[start..start + t.h as usize]
}

pub fn flat(pic: u8) -> &'static [u8] {
    let s = pic as usize * 4096;
    &FLAT_PIXELS[s..s + 4096]
}

pub fn colormap(n: usize) -> &'static [u8] {
    &COLORMAPS[n * 256..n * 256 + 256]
}
