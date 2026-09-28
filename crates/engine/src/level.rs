//! Runtime level state: the mutable parts of a map (sector heights, light,
//! switch textures, line flags) plus geometry helpers used by both the
//! physics and the renderer.

use crate::data::*;
use crate::fixed::*;
use crate::info::{LineSpecial, SectorSpecial};
use crate::limits::*;

pub type MRef = u16;
pub const NONE: MRef = 0xffff;

#[derive(Clone, Copy)]
pub struct Sector {
    pub floor: Fixed,
    pub ceil: Fixed,
    pub floorpic: u8,
    pub ceilpic: u8,
    pub light: u8,
    pub special: SectorSpecial,
    pub tag: u16,
    /// Mobj that last made noise here (monsters wake up on it).
    pub soundtarget: MRef,
    pub soundtraversed: u8,
    pub validcount: u32,
    /// Head of the list of mobjs in this sector.
    pub thinglist: MRef,
    /// Index+1 of the active mover (door/lift/floor) or 0.
    pub mover: u16,
    /// Bounding box in blockmap cells: [top, bottom, left, right].
    pub blockbox: [i16; 4],
}

impl Sector {
    pub const EMPTY: Sector = Sector {
        floor: 0,
        ceil: 0,
        floorpic: 0,
        ceilpic: 0,
        light: 0,
        special: SectorSpecial::None,
        tag: 0,
        soundtarget: NONE,
        soundtraversed: 0,
        validcount: 0,
        thinglist: NONE,
        mover: 0,
        blockbox: [0; 4],
    };
}

#[derive(Clone, Copy)]
pub struct LineRt {
    pub flags: u16,
    pub special: LineSpecial,
    pub validcount: u32,
}

#[derive(Clone, Copy)]
pub struct SideRt {
    pub top: u8,
    pub mid: u8,
    pub bottom: u8,
    pub xoff: Fixed,
    pub yoff: Fixed,
}

pub struct Level {
    pub map: MapRef,
    pub map_index: usize,
    pub sectors: [Sector; MAX_SECTORS],
    pub lines: [LineRt; MAX_LINES],
    pub sides: [SideRt; MAX_SIDES],
    pub blocklinks: [MRef; MAX_BLOCKS],
    pub validcount: u32,
}

/// Slope classification used to speed up box-vs-line tests.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Slope {
    Horizontal,
    Vertical,
    Positive,
    Negative,
}

impl Level {
    pub fn init(&mut self, index: usize) {
        let map = MAPS[index];
        self.map = MapRef(index as u8);
        self.map_index = index;
        for (i, s) in map.sectors.iter().enumerate() {
            self.sectors[i] = Sector {
                floor: fx(s.floor as i32),
                ceil: fx(s.ceil as i32),
                floorpic: s.floorpic,
                ceilpic: s.ceilpic,
                light: s.light,
                special: s.special,
                tag: s.tag,
                ..Sector::EMPTY
            };
        }
        for (i, l) in map.lines.iter().enumerate() {
            self.lines[i] = LineRt { flags: l.flags, special: l.special, validcount: 0 };
        }
        for (i, s) in map.sides.iter().enumerate() {
            self.sides[i] =
                SideRt { top: s.top, mid: s.mid, bottom: s.bottom, xoff: fx(s.xoff as i32), yoff: fx(s.yoff as i32) };
        }
        let nblocks = (map.blockmap.w * map.blockmap.h) as usize;
        for b in self.blocklinks[..nblocks].iter_mut() {
            *b = NONE;
        }
        // Sector bounding boxes in blockmap coordinates (for crushing checks).
        for si in 0..map.sectors.len() {
            let (mut t, mut b, mut l, mut r) = (i32::MIN, i32::MAX, i32::MAX, i32::MIN);
            for &li in self.sector_lines(si) {
                let ld = &map.lines[li as usize];
                for v in [ld.v1, ld.v2] {
                    let p = map.vertices[v as usize];
                    t = t.max(p.y);
                    b = b.min(p.y);
                    l = l.min(p.x);
                    r = r.max(p.x);
                }
            }
            if t < b {
                self.sectors[si].blockbox = [-1, 0, 0, -1]; // no lines: an empty box
                continue;
            }
            let bm = &map.blockmap;
            let cell = |v: Fixed, o: i32| ((v - fx(o)) >> (FRACBITS + 7)) as i16;
            self.sectors[si].blockbox = [cell(t, bm.y), cell(b, bm.y), cell(l, bm.x), cell(r, bm.x)];
        }
        self.validcount = 1;
    }

    #[inline]
    pub fn num_sectors(&self) -> usize {
        self.map.sectors.len()
    }

    #[inline]
    pub fn sector_lines(&self, s: usize) -> &'static [u16] {
        let m = self.map.get();
        &m.sector_lines[m.sector_line_start[s] as usize..m.sector_line_start[s + 1] as usize]
    }

    #[inline]
    pub fn vert(&self, i: u16) -> Vertex {
        self.map.vertices[i as usize]
    }

    #[inline]
    pub fn line(&self, i: usize) -> &'static LineDef {
        &self.map.get().lines[i]
    }

    pub fn line_dxdy(&self, i: usize) -> (Fixed, Fixed) {
        let l = self.line(i);
        let a = self.vert(l.v1);
        let b = self.vert(l.v2);
        (b.x - a.x, b.y - a.y)
    }

    pub fn line_slope(&self, i: usize) -> Slope {
        let (dx, dy) = self.line_dxdy(i);
        if dx == 0 {
            Slope::Vertical
        } else if dy == 0 {
            Slope::Horizontal
        } else if (dy > 0) == (dx > 0) {
            Slope::Positive
        } else {
            Slope::Negative
        }
    }

    /// [top, bottom, left, right] of a linedef.
    pub fn line_bbox(&self, i: usize) -> [Fixed; 4] {
        let l = self.line(i);
        let a = self.vert(l.v1);
        let b = self.vert(l.v2);
        [a.y.max(b.y), a.y.min(b.y), a.x.min(b.x), a.x.max(b.x)]
    }

    pub fn front_sector(&self, line: usize) -> usize {
        self.map.sides[self.line(line).side[0] as usize].sector as usize
    }

    pub fn back_sector(&self, line: usize) -> Option<usize> {
        let s = self.line(line).side[1];
        if s == NO_SIDE { None } else { Some(self.map.sides[s as usize].sector as usize) }
    }

    /// Sector on the given side (0 front, 1 back) of a line.
    pub fn side_sector(&self, line: usize, side: usize) -> Option<usize> {
        if side == 0 { Some(self.front_sector(line)) } else { self.back_sector(line) }
    }

    pub fn subsector_at(&self, x: Fixed, y: Fixed) -> usize {
        let m = self.map.get();
        let mut n = m.root;
        while n & NF_SUBSECTOR == 0 {
            let node = &m.nodes[n as usize];
            n = node.child[point_on_node_side(x, y, node)];
        }
        (n & !NF_SUBSECTOR) as usize
    }

    pub fn sector_at(&self, x: Fixed, y: Fixed) -> usize {
        self.map.subsectors[self.subsector_at(x, y)].sector as usize
    }

    /// Which side of a linedef a point is on: 0 front (right), 1 back.
    pub fn point_on_line_side(&self, x: Fixed, y: Fixed, line: usize) -> usize {
        let l = self.line(line);
        let a = self.vert(l.v1);
        let (dx, dy) = self.line_dxdy(line);
        point_on_side(x, y, a.x, a.y, dx, dy)
    }

    /// Returns 0/1 if the box is entirely on one side of the line, -1 if it straddles.
    pub fn box_on_line_side(&self, bbox: &[Fixed; 4], line: usize) -> i32 {
        let l = self.line(line);
        let a = self.vert(l.v1);
        let (dx, dy) = self.line_dxdy(line);
        let (p1, p2) = match self.line_slope(line) {
            Slope::Horizontal => {
                let mut p1 = (bbox[BOXTOP] > a.y) as i32;
                let mut p2 = (bbox[BOXBOTTOM] > a.y) as i32;
                if dx < 0 {
                    p1 ^= 1;
                    p2 ^= 1;
                }
                (p1, p2)
            }
            Slope::Vertical => {
                let mut p1 = (bbox[BOXRIGHT] < a.x) as i32;
                let mut p2 = (bbox[BOXLEFT] < a.x) as i32;
                if dy < 0 {
                    p1 ^= 1;
                    p2 ^= 1;
                }
                (p1, p2)
            }
            Slope::Positive => (
                point_on_side(bbox[BOXLEFT], bbox[BOXTOP], a.x, a.y, dx, dy) as i32,
                point_on_side(bbox[BOXRIGHT], bbox[BOXBOTTOM], a.x, a.y, dx, dy) as i32,
            ),
            Slope::Negative => (
                point_on_side(bbox[BOXRIGHT], bbox[BOXTOP], a.x, a.y, dx, dy) as i32,
                point_on_side(bbox[BOXLEFT], bbox[BOXBOTTOM], a.x, a.y, dx, dy) as i32,
            ),
        };
        if p1 == p2 { p1 } else { -1 }
    }

    /// Vertical opening of a two-sided line: (top, bottom, range, lowfloor).
    pub fn line_opening(&self, line: usize) -> Option<(Fixed, Fixed, Fixed, Fixed)> {
        let back = self.back_sector(line)?;
        let f = &self.sectors[self.front_sector(line)];
        let b = &self.sectors[back];
        let top = f.ceil.min(b.ceil);
        let (bottom, low) = if f.floor > b.floor { (f.floor, b.floor) } else { (b.floor, f.floor) };
        Some((top, bottom, top - bottom, low))
    }

    // ---------------------------------------------------------------- blockmap

    pub fn block_coords(&self, x: Fixed, y: Fixed) -> (i32, i32) {
        let bm = &self.map.blockmap;
        ((x - fx(bm.x)) >> (FRACBITS + 7), (y - fx(bm.y)) >> (FRACBITS + 7))
    }

    pub fn block_index(&self, bx: i32, by: i32) -> Option<usize> {
        let bm = &self.map.blockmap;
        if bx < 0 || by < 0 || bx >= bm.w || by >= bm.h { None } else { Some((by * bm.w + bx) as usize) }
    }

    /// Calls `f` for every line in a blockmap cell that hasn't been visited
    /// this `validcount`. Stops early when `f` returns false.
    pub fn block_lines(&mut self, bx: i32, by: i32, f: &mut dyn FnMut(&mut Level, usize) -> bool) -> bool {
        let Some(cell) = self.block_index(bx, by) else { return true };
        let bm = &self.map.get().blockmap;
        let mut i = bm.offsets[cell] as usize;
        loop {
            let li = bm.lines[i];
            if li == 0xffff {
                return true;
            }
            i += 1;
            let li = li as usize;
            if self.lines[li].validcount == self.validcount {
                continue;
            }
            self.lines[li].validcount = self.validcount;
            if !f(self, li) {
                return false;
            }
        }
    }

    // ---------------------------------------------------------------- sector searches (for movers)

    pub fn other_sector(&self, line: usize, sec: usize) -> Option<usize> {
        let f = self.front_sector(line);
        let b = self.back_sector(line)?;
        if f == sec { Some(b) } else { Some(f) }
    }

    pub fn lowest_floor_surrounding(&self, sec: usize) -> Fixed {
        let mut h = self.sectors[sec].floor;
        for &l in self.sector_lines(sec) {
            if let Some(o) = self.other_sector(l as usize, sec) {
                h = h.min(self.sectors[o].floor);
            }
        }
        h
    }

    pub fn highest_floor_surrounding(&self, sec: usize) -> Fixed {
        let mut h = fx(-500);
        for &l in self.sector_lines(sec) {
            if let Some(o) = self.other_sector(l as usize, sec) {
                h = h.max(self.sectors[o].floor);
            }
        }
        h
    }

    pub fn next_highest_floor(&self, sec: usize, current: Fixed) -> Fixed {
        let mut best: Option<Fixed> = None;
        for &l in self.sector_lines(sec) {
            if let Some(o) = self.other_sector(l as usize, sec) {
                let f = self.sectors[o].floor;
                if f > current && best.is_none_or(|b| f < b) {
                    best = Some(f);
                }
            }
        }
        best.unwrap_or(current)
    }

    pub fn lowest_ceiling_surrounding(&self, sec: usize) -> Fixed {
        let mut h = i32::MAX;
        for &l in self.sector_lines(sec) {
            if let Some(o) = self.other_sector(l as usize, sec) {
                h = h.min(self.sectors[o].ceil);
            }
        }
        h
    }

    pub fn highest_ceiling_surrounding(&self, sec: usize) -> Fixed {
        let mut h = 0;
        for &l in self.sector_lines(sec) {
            if let Some(o) = self.other_sector(l as usize, sec) {
                h = h.max(self.sectors[o].ceil);
            }
        }
        h
    }

    pub fn min_light_surrounding(&self, sec: usize, max: u8) -> u8 {
        let mut m = max;
        for &l in self.sector_lines(sec) {
            if let Some(o) = self.other_sector(l as usize, sec) {
                m = m.min(self.sectors[o].light);
            }
        }
        m
    }

    /// Iterate sectors with the given tag, starting after `start` (-1 to begin).
    pub fn find_sector_from_tag(&self, tag: u16, start: isize) -> Option<usize> {
        ((start + 1) as usize..self.num_sectors()).find(|&i| self.sectors[i].tag == tag)
    }
}

pub const BOXTOP: usize = 0;
pub const BOXBOTTOM: usize = 1;
pub const BOXLEFT: usize = 2;
pub const BOXRIGHT: usize = 3;

/// 0 = front (right of the direction), 1 = back.
#[inline]
pub fn point_on_side(x: Fixed, y: Fixed, lx: Fixed, ly: Fixed, ldx: Fixed, ldy: Fixed) -> usize {
    if ldx == 0 {
        return if x <= lx { (ldy > 0) as usize } else { (ldy < 0) as usize };
    }
    if ldy == 0 {
        return if y <= ly { (ldx < 0) as usize } else { (ldx > 0) as usize };
    }
    let dx = (x - lx) as i64;
    let dy = (y - ly) as i64;
    let left = (ldy as i64 >> 8) * dx;
    let right = dy * (ldx as i64 >> 8);
    if right < left { 0 } else { 1 }
}

#[inline]
pub fn point_on_node_side(x: Fixed, y: Fixed, n: &Node) -> usize {
    point_on_side(x, y, n.x, n.y, n.dx, n.dy)
}

