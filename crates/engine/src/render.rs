//! The 3D view: a front-to-back BSP renderer with column-drawn walls,
//! column-drawn floors/ceilings (no visplanes: saves ~80 KB), distance
//! lighting, and clipped billboard sprites. All integer maths.

use crate::data::*;
use crate::fixed::*;
use crate::info::*;
use crate::level::*;
use crate::limits::*;
use crate::mobj::Mobj;
use crate::models::{self, Canvas};

const HEIGHTBITS: u32 = 12;
const HEIGHTUNIT: i32 = 1 << HEIGHTBITS;
const MINZ: Fixed = FRACUNIT * 4;

const LIGHTLEVELS: usize = 16;
const LIGHTSEGSHIFT: u32 = 4;
const MAXLIGHTSCALE: usize = 48;
const LIGHTSCALESHIFT: u32 = 12;
const MAXLIGHTZ: usize = 128;
const LIGHTZSHIFT: u32 = 20;

const MAXSEGS: usize = 64;
const MAX_DRAWSEGS: usize = 192;
const MAX_OPENINGS: usize = MAX_W * 24;
const MAX_VISSPRITES: usize = 96;

const SIL_BOTTOM: u8 = 1;
const SIL_TOP: u8 = 2;

#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
enum Clip {
    None,
    /// An array full of the view height (used as a top clip: hides everything).
    Screen,
    /// An array full of -1 (used as a bottom clip: hides everything).
    NegOne,
    /// `openings[base + x]`.
    At(i32),
}

#[derive(Clone, Copy)]
struct DrawSeg {
    seg: u16,
    x1: i32,
    x2: i32,
    scale1: Fixed,
    scale2: Fixed,
    scalestep: Fixed,
    silhouette: u8,
    bsilheight: Fixed,
    tsilheight: Fixed,
    sprtopclip: Clip,
    sprbottomclip: Clip,
    masked: Clip,
}

const EMPTY_DS: DrawSeg = DrawSeg {
    seg: 0,
    x1: 0,
    x2: 0,
    scale1: 0,
    scale2: 0,
    scalestep: 0,
    silhouette: 0,
    bsilheight: 0,
    tsilheight: 0,
    sprtopclip: Clip::None,
    sprbottomclip: Clip::None,
    masked: Clip::None,
};

#[derive(Clone, Copy)]
struct VisSprite {
    x1: i32,
    x2: i32,
    gx: Fixed,
    gy: Fixed,
    gz: Fixed,
    gzt: Fixed,
    startfrac: Fixed,
    xiscale: Fixed,
    scale: Fixed,
    yscale: Fixed,
    texturemid: Fixed,
    colormap: i16,
    shadow: bool,
    sprite: Spr,
    frame: u8,
    rot: u8,
    anim: u16,
}

const EMPTY_VS: VisSprite = VisSprite {
    x1: 0,
    x2: 0,
    gx: 0,
    gy: 0,
    gz: 0,
    gzt: 0,
    startfrac: 0,
    xiscale: 0,
    scale: 0,
    yscale: 0,
    texturemid: 0,
    colormap: 0,
    shadow: false,
    sprite: Spr::None,
    frame: 0,
    rot: 0,
    anim: 0,
};

/// Per-wall working state (what the classic renderer kept in globals).
#[derive(Default)]
struct Wall {
    normalangle: Angle,
    angle1: Angle,
    distance: Fixed,
    offset: Fixed,
    centerangle: Angle,
    scale: Fixed,
    scalestep: Fixed,
    topfrac: Fixed,
    topstep: Fixed,
    bottomfrac: Fixed,
    bottomstep: Fixed,
    pixhigh: Fixed,
    pixhighstep: Fixed,
    pixlow: Fixed,
    pixlowstep: Fixed,
    midtex: u8,
    toptex: u8,
    bottomtex: u8,
    masked: bool,
    midtexmid: Fixed,
    toptexmid: Fixed,
    bottomtexmid: Fixed,
    markfloor: bool,
    markceiling: bool,
    textured: bool,
    lightrow: usize,
    front: usize,
    maskbase: i32,
}

/// A weapon sprite overlay ("psprite") to draw over the view.
#[derive(Clone, Copy)]
pub struct PSpriteView {
    pub sprite: Spr,
    pub frame: u8,
    pub sx: Fixed,
    pub sy: Fixed,
    pub anim: u16,
}

/// Everything the renderer needs to know about the camera.
pub struct ViewParams {
    pub x: Fixed,
    pub y: Fixed,
    pub z: Fixed,
    pub angle: Angle,
    pub extralight: i32,
    /// Forced colormap (e.g. the invulnerability look), or -1.
    pub fixedcolormap: i32,
    pub player: MRef,
    pub psprites: [Option<PSpriteView>; 2],
    pub shadow_weapon: bool,
    pub leveltime: u32,
    pub sky: i32,
}

pub struct Renderer {
    pub screen: [u8; MAX_W * MAX_H],
    pub sw: usize,
    pub sh: usize,
    pub vx: usize,
    pub vy: usize,
    pub vw: usize,
    pub vh: usize,
    /// Treat output pixels as square (terminal / modern displays) instead of
    /// the classic 320x200 non-square pixels.
    pub square_pixels: bool,

    centerx: i32,
    centery: i32,
    centerxfrac: Fixed,
    centeryfrac: Fixed,
    projection: Fixed,
    yprojection: Fixed,
    viewangletox: [i16; FINEANGLES / 2],
    xtoviewangle: [Angle; MAX_W + 1],
    clipangle: Angle,
    yslope: [Fixed; MAX_H],
    distscale: [Fixed; MAX_W],
    pdirx: [Fixed; MAX_W],
    pdiry: [Fixed; MAX_W],
    scalelight: [[u8; MAXLIGHTSCALE]; LIGHTLEVELS],
    zlight: [[u8; MAXLIGHTZ]; LIGHTLEVELS],
    skyrow: [u8; 128],
    skyrow_red: [u8; 128],
    skyprofile: [u8; 256],

    viewx: Fixed,
    viewy: Fixed,
    viewz: Fixed,
    viewangle: Angle,
    viewsin: Fixed,
    viewcos: Fixed,
    extralight: i32,
    fixedcolormap: i32,
    leveltime: u32,
    sky: i32,

    solidsegs: [(i32, i32); MAXSEGS],
    nsolid: usize,
    ceilingclip: [i16; MAX_W],
    floorclip: [i16; MAX_W],
    drawsegs: [DrawSeg; MAX_DRAWSEGS],
    ndrawsegs: usize,
    openings: [i16; MAX_OPENINGS],
    lastopening: usize,
    vissprites: [VisSprite; MAX_VISSPRITES],
    nvis: usize,
    order: [u8; MAX_VISSPRITES],
    sector_frame: [u32; MAX_SECTORS],
    framecount: u32,
    flat_xlat: [u8; 64],
    player: MRef,

    /// Lines drawn since the level started (for the automap).
    pub seen: [u32; crate::SEEN_WORDS],
    /// 0 = full detail, 1 = render half width and double pixels.
    detail: u32,

    pub canvas: Canvas,
}

impl Renderer {
    /// Initialise in place (the struct is large; never build it on the stack).
    pub fn init(&mut self, sw: usize, sh: usize) {
        self.framecount = 0;
        self.square_pixels = false;
        init_sky(self);
        self.set_view(sw, sh, sw, sh, 0, 0);
    }

    /// Set the screen size and the 3D view window inside it.
    pub fn set_view(&mut self, sw: usize, sh: usize, vw: usize, vh: usize, vx: usize, vy: usize) {
        self.sw = sw.clamp(16, MAX_W);
        self.sh = sh.clamp(16, MAX_H);
        self.vw = vw.clamp(16, self.sw);
        self.vh = vh.clamp(8, self.sh);
        self.vx = vx.min(self.sw - self.vw);
        self.vy = vy.min(self.sh - self.vh);
        let vw = self.vw as i32;
        let vh = self.vh as i32;
        self.centerx = vw / 2;
        self.centery = vh / 2;
        self.centerxfrac = fx(self.centerx);
        self.centeryfrac = fx(self.centery);
        self.projection = self.centerxfrac;
        // Classic pixels are 1.2x taller than wide; on square pixels scale up to match.
        let yp = self.projection << self.detail;
        self.yprojection = if self.square_pixels { yp * 6 / 5 } else { yp };

        // Angle -> column mapping over the 90 degree field of view.
        let focal = fdiv(self.centerxfrac, finetangent(FINEANGLES / 4 + FINEANGLES / 8));
        for i in 0..FINEANGLES / 2 {
            let t = finetangent(i);
            let x = if t > FRACUNIT * 2 {
                -1
            } else if t < -FRACUNIT * 2 {
                vw + 1
            } else {
                let tx = fmul(t, focal);
                ((self.centerxfrac - tx + FRACUNIT - 1) >> FRACBITS).clamp(-1, vw + 1)
            };
            self.viewangletox[i] = x as i16;
        }
        for x in 0..=vw {
            let mut i = 0;
            while (self.viewangletox[i] as i32) > x {
                i += 1;
            }
            self.xtoviewangle[x as usize] = ((i as u32) << ANGLETOFINESHIFT).wrapping_sub(ANG90);
        }
        for v in self.viewangletox.iter_mut() {
            if *v == -1 {
                *v = 0;
            } else if *v as i32 == vw + 1 {
                *v = vw as i16;
            }
        }
        self.clipangle = self.xtoviewangle[0];
        for x in 0..vw as usize {
            let c = cos_a(self.xtoviewangle[x]).abs();
            self.distscale[x] = fdiv(FRACUNIT, c);
        }
        for y in 0..vh {
            let dy = ((y - self.centery) << FRACBITS) + FRACUNIT / 2;
            self.yslope[y as usize] = fdiv(self.yprojection, dy.abs());
        }
        // Light tables: brighter when near, normalised to a 320-wide view.
        for i in 0..LIGHTLEVELS {
            let start = ((LIGHTLEVELS - 1 - i) * 2 * NUM_COLORMAPS / LIGHTLEVELS) as i32;
            for j in 0..MAXLIGHTSCALE {
                let level = start - (j as i32 * 320 / (vw << self.detail)) / 2;
                self.scalelight[i][j] = level.clamp(0, NUM_COLORMAPS as i32 - 1) as u8;
            }
            for j in 0..MAXLIGHTZ {
                let scale = fdiv(fx(160), ((j as i32) + 1) << LIGHTZSHIFT) >> LIGHTSCALESHIFT;
                let level = start - scale / 2;
                self.zlight[i][j] = level.clamp(0, NUM_COLORMAPS as i32 - 1) as u8;
            }
        }
    }

    pub fn clear_seen(&mut self) {
        self.seen = [0; crate::SEEN_WORDS];
    }

    /// Low detail renders a half-width view and doubles every pixel.
    pub fn set_detail(&mut self, detail: u32) {
        self.detail = detail.min(1);
        self.set_view(self.sw, self.sh, self.vw, self.vh, self.vx, self.vy);
    }

    pub fn set_square_pixels(&mut self, sq: bool) {
        self.square_pixels = sq;
        self.set_view(self.sw, self.sh, self.vw, self.vh, self.vx, self.vy);
    }

    #[inline]
    fn put(&mut self, x: i32, y: i32, c: u8) {
        let i = (self.vy + y as usize) * self.sw + self.vx + x as usize;
        self.screen[i] = c;
    }

    fn light_row(&self, light: u8, horizontal: bool, vertical: bool) -> usize {
        let mut l = (light >> LIGHTSEGSHIFT) as i32 + self.extralight;
        if horizontal {
            l -= 1;
        } else if vertical {
            l += 1;
        }
        l.clamp(0, LIGHTLEVELS as i32 - 1) as usize
    }

    // ================================================================ frame

    pub fn render(&mut self, lv: &Level, mobjs: &[Mobj], view: &ViewParams) {
        self.framecount = self.framecount.wrapping_add(1);
        self.viewx = view.x;
        self.viewy = view.y;
        self.viewz = view.z;
        self.viewangle = view.angle;
        self.viewsin = sin_a(view.angle);
        self.viewcos = cos_a(view.angle);
        self.extralight = view.extralight;
        self.fixedcolormap = view.fixedcolormap;
        self.leveltime = view.leveltime;
        self.sky = view.sky;
        self.player = view.player;
        for (i, t) in self.flat_xlat.iter_mut().enumerate() {
            *t = i as u8;
        }
        for &(first, count) in FLAT_ANIMS {
            for k in 0..count {
                let frame = (k as u32 + view.leveltime / 8) % count as u32;
                self.flat_xlat[(first + k) as usize] = first + frame as u8;
            }
        }
        for x in 0..self.vw {
            let a = self.viewangle.wrapping_add(self.xtoviewangle[x]);
            self.pdirx[x] = fmul(cos_a(a), self.distscale[x]);
            self.pdiry[x] = fmul(sin_a(a), self.distscale[x]);
        }
        self.solidsegs[0] = (-0x7fff, -1);
        self.solidsegs[1] = (self.vw as i32, 0x7fff);
        self.nsolid = 2;
        for x in 0..self.vw {
            self.ceilingclip[x] = -1;
            self.floorclip[x] = self.vh as i16;
        }
        self.ndrawsegs = 0;
        self.lastopening = 0;
        self.nvis = 0;
        self.render_bsp(lv, mobjs, lv.map.root, 0);
        self.draw_masked(lv, mobjs);
        self.draw_psprites(lv, mobjs, view);
        if self.detail == 1 {
            // Stretch the half-width view back to full width, right to left.
            for y in 0..self.vh {
                let row = (self.vy + y) * self.sw + self.vx;
                for x in (0..self.vw).rev() {
                    let c = self.screen[row + x];
                    if row + 2 * x + 1 < self.screen.len() {
                        self.screen[row + 2 * x] = c;
                        self.screen[row + 2 * x + 1] = c;
                    }
                }
            }
        }
    }

    fn render_bsp(&mut self, lv: &Level, mobjs: &[Mobj], node: u16, depth: u32) {
        if node & NF_SUBSECTOR != 0 {
            self.subsector(lv, mobjs, (node & !NF_SUBSECTOR) as usize);
            return;
        }
        if depth > 256 {
            return;
        }
        let n = &lv.map.nodes[node as usize];
        let side = point_on_node_side(self.viewx, self.viewy, n);
        self.render_bsp(lv, mobjs, n.child[side], depth + 1);
        if self.check_bbox(&n.bbox[side ^ 1]) {
            self.render_bsp(lv, mobjs, n.child[side ^ 1], depth + 1);
        }
    }

    fn check_bbox(&self, bb: &[i16; 4]) -> bool {
        const CHECK: [[usize; 4]; 11] = [
            [3, 0, 2, 1],
            [3, 0, 2, 0],
            [3, 1, 2, 0],
            [0, 0, 0, 0],
            [2, 0, 2, 1],
            [0, 0, 0, 0],
            [3, 1, 3, 0],
            [0, 0, 0, 0],
            [2, 0, 3, 1],
            [2, 1, 3, 1],
            [2, 1, 3, 0],
        ];
        let b = [fx(bb[0] as i32), fx(bb[1] as i32), fx(bb[2] as i32), fx(bb[3] as i32)];
        let bx = if self.viewx <= b[2] { 0 } else if self.viewx < b[3] { 1 } else { 2 };
        let by = if self.viewy >= b[0] { 0 } else if self.viewy > b[1] { 1 } else { 2 };
        let pos = by * 4 + bx;
        if pos == 5 {
            return true;
        }
        let c = CHECK[pos];
        let (x1, y1, x2, y2) = (b[c[0]], b[c[1]], b[c[2]], b[c[3]]);
        let mut angle1 = point_to_angle(x1 - self.viewx, y1 - self.viewy).wrapping_sub(self.viewangle);
        let mut angle2 = point_to_angle(x2 - self.viewx, y2 - self.viewy).wrapping_sub(self.viewangle);
        let span = angle1.wrapping_sub(angle2);
        if span >= ANG180 {
            return true;
        }
        let clip2 = self.clipangle.wrapping_mul(2);
        let mut tspan = angle1.wrapping_add(self.clipangle);
        if tspan > clip2 {
            tspan = tspan.wrapping_sub(clip2);
            if tspan >= span {
                return false;
            }
            angle1 = self.clipangle;
        }
        tspan = self.clipangle.wrapping_sub(angle2);
        if tspan > clip2 {
            tspan = tspan.wrapping_sub(clip2);
            if tspan >= span {
                return false;
            }
            angle2 = 0u32.wrapping_sub(self.clipangle);
        }
        let sx1 = self.viewangletox[(angle1.wrapping_add(ANG90) >> ANGLETOFINESHIFT) as usize] as i32;
        let mut sx2 = self.viewangletox[(angle2.wrapping_add(ANG90) >> ANGLETOFINESHIFT) as usize] as i32;
        if sx1 == sx2 {
            return false;
        }
        sx2 -= 1;
        let mut i = 0;
        while self.solidsegs[i].1 < sx2 {
            i += 1;
        }
        !(sx1 >= self.solidsegs[i].0 && sx2 <= self.solidsegs[i].1)
    }

    fn subsector(&mut self, lv: &Level, mobjs: &[Mobj], ss: usize) {
        let sub = &lv.map.subsectors[ss];
        let sec = sub.sector as usize;
        if self.sector_frame[sec] != self.framecount {
            self.sector_frame[sec] = self.framecount;
            self.add_sprites(lv, mobjs, sec);
        }
        for i in sub.first as usize..(sub.first + sub.count) as usize {
            self.add_line(lv, i);
        }
    }

    fn seg_sectors(&self, lv: &Level, seg: &Seg) -> (usize, Option<usize>) {
        let li = seg.line as usize;
        if seg.side == 0 {
            (lv.front_sector(li), lv.back_sector(li))
        } else {
            (lv.back_sector(li).unwrap(), Some(lv.front_sector(li)))
        }
    }

    fn add_line(&mut self, lv: &Level, segi: usize) {
        let seg = &lv.map.segs[segi];
        let v1 = lv.vert(seg.v1);
        let v2 = lv.vert(seg.v2);
        let mut angle1 = point_to_angle(v1.x - self.viewx, v1.y - self.viewy);
        let mut angle2 = point_to_angle(v2.x - self.viewx, v2.y - self.viewy);
        let span = angle1.wrapping_sub(angle2);
        if span >= ANG180 {
            return;
        }
        let rw_angle1 = angle1;
        angle1 = angle1.wrapping_sub(self.viewangle);
        angle2 = angle2.wrapping_sub(self.viewangle);
        let clip2 = self.clipangle.wrapping_mul(2);
        let mut tspan = angle1.wrapping_add(self.clipangle);
        if tspan > clip2 {
            tspan = tspan.wrapping_sub(clip2);
            if tspan >= span {
                return;
            }
            angle1 = self.clipangle;
        }
        tspan = self.clipangle.wrapping_sub(angle2);
        if tspan > clip2 {
            tspan = tspan.wrapping_sub(clip2);
            if tspan >= span {
                return;
            }
            angle2 = 0u32.wrapping_sub(self.clipangle);
        }
        let x1 = self.viewangletox[(angle1.wrapping_add(ANG90) >> ANGLETOFINESHIFT) as usize] as i32;
        let x2 = self.viewangletox[(angle2.wrapping_add(ANG90) >> ANGLETOFINESHIFT) as usize] as i32;
        if x1 >= x2 {
            return;
        }
        let (front, back) = self.seg_sectors(lv, seg);
        let solid = match back {
            None => true,
            Some(b) => {
                let (f, b) = (&lv.sectors[front], &lv.sectors[b]);
                if b.ceil <= f.floor || b.floor >= f.ceil {
                    true
                } else if b.ceil == f.ceil
                    && b.floor == f.floor
                    && b.ceilpic == f.ceilpic
                    && b.floorpic == f.floorpic
                    && b.light == f.light
                    && lv.sides[lv.line(seg.line as usize).side[seg.side as usize] as usize].mid == 0
                {
                    return; // invisible line between identical sectors
                } else {
                    false
                }
            }
        };
        if solid {
            self.clip_solid(lv, segi, rw_angle1, x1, x2 - 1);
        } else {
            self.clip_pass(lv, segi, rw_angle1, x1, x2 - 1);
        }
    }

    fn clip_solid(&mut self, lv: &Level, seg: usize, a1: Angle, first: i32, last: i32) {
        let mut start = 0;
        while self.solidsegs[start].1 < first - 1 {
            start += 1;
        }
        if first < self.solidsegs[start].0 {
            if last < self.solidsegs[start].0 - 1 {
                self.store_wall_range(lv, seg, a1, first, last);
                if self.nsolid < MAXSEGS {
                    for i in (start..self.nsolid).rev() {
                        self.solidsegs[i + 1] = self.solidsegs[i];
                    }
                    self.solidsegs[start] = (first, last);
                    self.nsolid += 1;
                }
                return;
            }
            let s0 = self.solidsegs[start].0;
            self.store_wall_range(lv, seg, a1, first, s0 - 1);
            self.solidsegs[start].0 = first;
        }
        if last <= self.solidsegs[start].1 {
            return;
        }
        let mut next = start;
        loop {
            if last < self.solidsegs[next + 1].0 - 1 {
                break;
            }
            let (a, b) = (self.solidsegs[next].1 + 1, self.solidsegs[next + 1].0 - 1);
            self.store_wall_range(lv, seg, a1, a, b);
            next += 1;
            if last <= self.solidsegs[next].1 {
                self.solidsegs[start].1 = self.solidsegs[next].1;
                self.crunch(start, next);
                return;
            }
        }
        let a = self.solidsegs[next].1 + 1;
        self.store_wall_range(lv, seg, a1, a, last);
        self.solidsegs[start].1 = last;
        self.crunch(start, next);
    }

    fn crunch(&mut self, start: usize, next: usize) {
        if next == start {
            return;
        }
        let removed = next - start;
        for i in next + 1..self.nsolid {
            self.solidsegs[i - removed] = self.solidsegs[i];
        }
        self.nsolid -= removed;
    }

    fn clip_pass(&mut self, lv: &Level, seg: usize, a1: Angle, first: i32, last: i32) {
        let mut start = 0;
        while self.solidsegs[start].1 < first - 1 {
            start += 1;
        }
        if first < self.solidsegs[start].0 {
            if last < self.solidsegs[start].0 - 1 {
                self.store_wall_range(lv, seg, a1, first, last);
                return;
            }
            let s0 = self.solidsegs[start].0;
            self.store_wall_range(lv, seg, a1, first, s0 - 1);
        }
        if last <= self.solidsegs[start].1 {
            return;
        }
        while last >= self.solidsegs[start + 1].0 - 1 {
            let (a, b) = (self.solidsegs[start].1 + 1, self.solidsegs[start + 1].0 - 1);
            self.store_wall_range(lv, seg, a1, a, b);
            start += 1;
            if last <= self.solidsegs[start].1 {
                return;
            }
        }
        let a = self.solidsegs[start].1 + 1;
        self.store_wall_range(lv, seg, a1, a, last);
    }

    fn scale_from_angle(&self, visangle: Angle, w: &Wall) -> Fixed {
        let anglea = ANG90.wrapping_add(visangle.wrapping_sub(self.viewangle));
        let angleb = ANG90.wrapping_add(visangle.wrapping_sub(w.normalangle));
        let sinea = sin_a(anglea);
        let sineb = sin_a(angleb);
        let num = fmul(self.yprojection, sineb);
        let den = fmul(w.distance, sinea);
        if den > num >> FRACBITS {
            fdiv(num, den).clamp(256, 64 * FRACUNIT)
        } else {
            64 * FRACUNIT
        }
    }

    fn alloc_openings(&mut self, n: usize) -> Option<usize> {
        if self.lastopening + n > MAX_OPENINGS {
            return None;
        }
        let at = self.lastopening;
        self.lastopening += n;
        Some(at)
    }

    fn store_wall_range(&mut self, lv: &Level, segi: usize, rw_angle1: Angle, start: i32, stop: i32) {
        if self.ndrawsegs >= MAX_DRAWSEGS || start > stop {
            return;
        }
        let seg = &lv.map.segs[segi];
        let line = lv.line(seg.line as usize);
        let side = &lv.sides[line.side[seg.side as usize] as usize];
        let lflags = lv.lines[seg.line as usize].flags;
        let (fi, bi) = self.seg_sectors(lv, seg);
        let front = lv.sectors[fi];
        self.seen[seg.line as usize / 32] |= 1 << (seg.line % 32);
        let v1 = lv.vert(seg.v1);
        let v2 = lv.vert(seg.v2);

        let mut w = Wall {
            normalangle: ((seg.angle as u32) << 16).wrapping_add(ANG90),
            angle1: rw_angle1,
            front: fi,
            ..Default::default()
        };
        let mut offsetangle = w.normalangle.wrapping_sub(rw_angle1);
        if offsetangle > ANG180 {
            offsetangle = 0u32.wrapping_sub(offsetangle);
        }
        if offsetangle > ANG90 {
            offsetangle = ANG90;
        }
        let distangle = ANG90 - offsetangle;
        let hyp = point_dist(v1.x - self.viewx, v1.y - self.viewy);
        w.distance = fmul(hyp, sin_a(distangle)).max(1);

        let mut ds = EMPTY_DS;
        ds.seg = segi as u16;
        ds.x1 = start;
        ds.x2 = stop;
        w.scale = self.scale_from_angle(self.viewangle.wrapping_add(self.xtoviewangle[start as usize]), &w);
        ds.scale1 = w.scale;
        if stop > start {
            ds.scale2 = self.scale_from_angle(self.viewangle.wrapping_add(self.xtoviewangle[stop as usize]), &w);
            w.scalestep = (ds.scale2 - w.scale) / (stop - start);
            ds.scalestep = w.scalestep;
        } else {
            ds.scale2 = ds.scale1;
        }

        let mut worldtop = front.ceil - self.viewz;
        let worldbottom = front.floor - self.viewz;
        let mut worldhigh = 0;
        let mut worldlow = 0;
        match bi {
            None => {
                w.midtex = side.mid;
                w.markfloor = true;
                w.markceiling = true;
                w.midtexmid = if lflags & ML_DONTPEGBOTTOM != 0 {
                    front.floor + fx(WALLS[w.midtex as usize].h as i32) - self.viewz
                } else {
                    worldtop
                } + side.yoff;
                ds.silhouette = SIL_BOTH;
                ds.sprtopclip = Clip::Screen;
                ds.sprbottomclip = Clip::NegOne;
                ds.bsilheight = i32::MAX;
                ds.tsilheight = i32::MIN;
            }
            Some(bi) => {
                let back = lv.sectors[bi];
                if front.floor > back.floor {
                    ds.silhouette = SIL_BOTTOM;
                    ds.bsilheight = front.floor;
                } else if back.floor > self.viewz {
                    ds.silhouette = SIL_BOTTOM;
                    ds.bsilheight = i32::MAX;
                }
                if front.ceil < back.ceil {
                    ds.silhouette |= SIL_TOP;
                    ds.tsilheight = front.ceil;
                } else if back.ceil < self.viewz {
                    ds.silhouette |= SIL_TOP;
                    ds.tsilheight = i32::MIN;
                }
                if back.ceil <= front.floor {
                    ds.sprbottomclip = Clip::NegOne;
                    ds.bsilheight = i32::MAX;
                    ds.silhouette |= SIL_BOTTOM;
                }
                if back.floor >= front.ceil {
                    ds.sprtopclip = Clip::Screen;
                    ds.tsilheight = i32::MIN;
                    ds.silhouette |= SIL_TOP;
                }
                worldhigh = back.ceil - self.viewz;
                worldlow = back.floor - self.viewz;
                if front.ceilpic == SKY_FLAT && back.ceilpic == SKY_FLAT {
                    worldtop = worldhigh; // sky hack: no upper wall between two skies
                }
                w.markfloor = worldlow != worldbottom || back.floorpic != front.floorpic || back.light != front.light;
                w.markceiling = worldhigh != worldtop || back.ceilpic != front.ceilpic || back.light != front.light;
                if back.ceil <= front.floor || back.floor >= front.ceil {
                    w.markceiling = true;
                    w.markfloor = true;
                }
                if worldhigh < worldtop {
                    w.toptex = side.top;
                    w.toptexmid = if lflags & ML_DONTPEGTOP != 0 {
                        worldtop
                    } else {
                        back.ceil + fx(WALLS[w.toptex as usize].h as i32) - self.viewz
                    } + side.yoff;
                }
                if worldlow > worldbottom {
                    w.bottomtex = side.bottom;
                    w.bottomtexmid = if lflags & ML_DONTPEGBOTTOM != 0 { worldtop } else { worldlow } + side.yoff;
                }
                if side.mid != 0 {
                    w.masked = true;
                    w.midtexmid = if lflags & ML_DONTPEGBOTTOM != 0 {
                        front.floor.max(back.floor) + fx(WALLS[side.mid as usize].h as i32) - self.viewz
                    } else {
                        front.ceil.min(back.ceil) - self.viewz
                    } + side.yoff;
                    match self.alloc_openings((stop - start + 1) as usize) {
                        Some(at) => {
                            w.maskbase = at as i32 - start;
                            ds.masked = Clip::At(w.maskbase);
                        }
                        None => w.masked = false,
                    }
                }
            }
        }

        w.textured = w.midtex != 0 || w.toptex != 0 || w.bottomtex != 0 || w.masked;
        if w.textured {
            let mut off = w.normalangle.wrapping_sub(rw_angle1);
            if off > ANG180 {
                off = 0u32.wrapping_sub(off);
            }
            if off > ANG90 {
                off = ANG90;
            }
            w.offset = fmul(hyp, sin_a(off));
            if w.normalangle.wrapping_sub(rw_angle1) < ANG180 {
                w.offset = -w.offset;
            }
            w.offset += side.xoff + seg.offset;
            w.centerangle = ANG90.wrapping_add(self.viewangle).wrapping_sub(w.normalangle);
            w.lightrow = self.light_row(front.light, v1.y == v2.y, v1.x == v2.x);
        }
        if front.floor >= self.viewz {
            w.markfloor = false;
        }
        if front.ceil <= self.viewz && front.ceilpic != SKY_FLAT {
            w.markceiling = false;
        }

        let wt = worldtop >> 4;
        let wb = worldbottom >> 4;
        w.topstep = -fmul(w.scalestep, wt);
        w.topfrac = (self.centeryfrac >> 4) - fmul(wt, w.scale);
        w.bottomstep = -fmul(w.scalestep, wb);
        w.bottomfrac = (self.centeryfrac >> 4) - fmul(wb, w.scale);
        if bi.is_some() {
            let wh = worldhigh >> 4;
            let wl = worldlow >> 4;
            if worldhigh < worldtop {
                w.pixhigh = (self.centeryfrac >> 4) - fmul(wh, w.scale);
                w.pixhighstep = -fmul(w.scalestep, wh);
            }
            if worldlow > worldbottom {
                w.pixlow = (self.centeryfrac >> 4) - fmul(wl, w.scale);
                w.pixlowstep = -fmul(w.scalestep, wl);
            }
        }

        self.seg_loop(lv, &mut w, start, stop + 1);

        if (ds.silhouette & SIL_TOP != 0 || w.masked) && ds.sprtopclip == Clip::None {
            if let Some(at) = self.alloc_openings((stop - start + 1) as usize) {
                for x in start..=stop {
                    self.openings[at + (x - start) as usize] = self.ceilingclip[x as usize];
                }
                ds.sprtopclip = Clip::At(at as i32 - start);
            }
        }
        if (ds.silhouette & SIL_BOTTOM != 0 || w.masked) && ds.sprbottomclip == Clip::None {
            if let Some(at) = self.alloc_openings((stop - start + 1) as usize) {
                for x in start..=stop {
                    self.openings[at + (x - start) as usize] = self.floorclip[x as usize];
                }
                ds.sprbottomclip = Clip::At(at as i32 - start);
            }
        }
        if w.masked && ds.silhouette & SIL_TOP == 0 {
            ds.silhouette |= SIL_TOP;
            ds.tsilheight = i32::MIN;
        }
        if w.masked && ds.silhouette & SIL_BOTTOM == 0 {
            ds.silhouette |= SIL_BOTTOM;
            ds.bsilheight = i32::MAX;
        }
        self.drawsegs[self.ndrawsegs] = ds;
        self.ndrawsegs += 1;
    }

    fn seg_loop(&mut self, lv: &Level, w: &mut Wall, x0: i32, x1: i32) {
        let front = lv.sectors[w.front];
        for x in x0..x1 {
            let xi = x as usize;
            let cc = self.ceilingclip[xi] as i32;
            let fc = self.floorclip[xi] as i32;
            let mut yl = (w.topfrac + HEIGHTUNIT - 1) >> HEIGHTBITS;
            if yl < cc + 1 {
                yl = cc + 1;
            }
            if w.markceiling {
                let top = cc + 1;
                let bottom = (yl - 1).min(fc - 1);
                if top <= bottom {
                    self.plane_column(x, top, bottom, front.ceil, front.ceilpic, front.light);
                }
            }
            let mut yh = w.bottomfrac >> HEIGHTBITS;
            if yh >= fc {
                yh = fc - 1;
            }
            if w.markfloor {
                let top = (yh + 1).max(cc + 1);
                let bottom = fc - 1;
                if top <= bottom {
                    self.plane_column(x, top, bottom, front.floor, front.floorpic, front.light);
                }
            }
            let mut texcol = 0;
            let mut cmap = 0usize;
            let mut iscale = 0;
            if w.textured {
                let angle = (w.centerangle.wrapping_add(self.xtoviewangle[xi]) >> ANGLETOFINESHIFT) as usize;
                texcol = (w.offset - fmul(finetangent(angle), w.distance)) >> FRACBITS;
                let idx = ((w.scale >> LIGHTSCALESHIFT) as usize).min(MAXLIGHTSCALE - 1);
                cmap = if self.fixedcolormap >= 0 {
                    self.fixedcolormap as usize
                } else {
                    self.scalelight[w.lightrow][idx] as usize
                };
                iscale = (0xffff_ffffu32 / (w.scale as u32).max(1)) as i32;
            }
            if w.midtex != 0 {
                self.wall_column(x, yl, yh, w.midtex, texcol, w.midtexmid, iscale, cmap);
                self.ceilingclip[xi] = self.vh as i16;
                self.floorclip[xi] = -1;
            } else {
                if w.toptex != 0 {
                    let mut mid = w.pixhigh >> HEIGHTBITS;
                    w.pixhigh += w.pixhighstep;
                    if mid >= fc {
                        mid = fc - 1;
                    }
                    if mid >= yl {
                        self.wall_column(x, yl, mid, w.toptex, texcol, w.toptexmid, iscale, cmap);
                        self.ceilingclip[xi] = mid as i16;
                    } else {
                        self.ceilingclip[xi] = (yl - 1) as i16;
                    }
                } else if w.markceiling {
                    self.ceilingclip[xi] = (yl - 1) as i16;
                }
                if w.bottomtex != 0 {
                    let mut mid = (w.pixlow + HEIGHTUNIT - 1) >> HEIGHTBITS;
                    w.pixlow += w.pixlowstep;
                    if mid <= self.ceilingclip[xi] as i32 {
                        mid = self.ceilingclip[xi] as i32 + 1;
                    }
                    if mid <= yh {
                        self.wall_column(x, mid, yh, w.bottomtex, texcol, w.bottomtexmid, iscale, cmap);
                        self.floorclip[xi] = mid as i16;
                    } else {
                        self.floorclip[xi] = (yh + 1) as i16;
                    }
                } else if w.markfloor {
                    self.floorclip[xi] = (yh + 1) as i16;
                }
                if w.masked {
                    self.openings[(w.maskbase + x) as usize] = texcol as i16;
                }
            }
            w.scale += w.scalestep;
            w.topfrac += w.topstep;
            w.bottomfrac += w.bottomstep;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn wall_column(&mut self, x: i32, yl: i32, yh: i32, tex: u8, col: i32, texmid: Fixed, iscale: i32, cmap: usize) {
        if yl > yh || tex == 0 {
            return;
        }
        let src = wall_column(tex, col);
        let hmask = src.len() - 1;
        let cm = colormap(cmap);
        let mut frac = texmid.wrapping_add((yl - self.centery).wrapping_mul(iscale));
        let base = self.vy * self.sw + self.vx + x as usize;
        for y in yl..=yh {
            let t = src[(frac >> FRACBITS) as usize & hmask];
            self.screen[base + y as usize * self.sw] = cm[t as usize];
            frac = frac.wrapping_add(iscale);
        }
    }

    fn plane_column(&mut self, x: i32, y1: i32, y2: i32, height: Fixed, pic: u8, light: u8) {
        if pic == SKY_FLAT {
            self.sky_column(x, y1, y2);
            return;
        }
        let planeheight = (height - self.viewz).abs();
        let src = flat(self.flat_xlat.get(pic as usize).copied().unwrap_or(pic));
        let lrow = ((light >> LIGHTSEGSHIFT) as i32 + self.extralight).clamp(0, LIGHTLEVELS as i32 - 1) as usize;
        let xi = x as usize;
        let (dx, dy) = (self.pdirx[xi], self.pdiry[xi]);
        let base = self.vy * self.sw + self.vx + xi;
        for y in y1..=y2 {
            let dist = fmul(planeheight, self.yslope[y as usize]);
            let u = self.viewx.wrapping_add(fmul(dx, dist));
            let v = (-self.viewy).wrapping_sub(fmul(dy, dist));
            let cm = if self.fixedcolormap >= 0 {
                self.fixedcolormap as usize
            } else {
                self.zlight[lrow][((dist >> LIGHTZSHIFT) as usize).min(MAXLIGHTZ - 1)] as usize
            };
            let t = src[((((v >> FRACBITS) & 63) << 6) | ((u >> FRACBITS) & 63)) as usize];
            self.screen[base + y as usize * self.sw] = COLORMAPS[cm * 256 + t as usize];
        }
    }

    fn sky_column(&mut self, x: i32, y1: i32, y2: i32) {
        let a = self.viewangle.wrapping_add(self.xtoviewangle[x as usize]);
        let col = (a >> 22) as usize; // 1024 columns per turn
        let horizon = self.skyprofile[(col >> 2) & 255] as i32;
        let base = self.vy * self.sw + self.vx + x as usize;
        let invul = self.fixedcolormap == NUM_COLORMAPS as i32;
        for y in y1..=y2 {
            // Sky texels: 0 (zenith) .. 127 (below horizon), horizon near row 100.
            let t = (100 + (y - self.centery) * 200 / self.vh as i32).clamp(0, 127);
            let mut c = if t >= 127 - horizon {
                let d = (t - (127 - horizon)).min(15);
                sky_mountain(self.sky, d, col)
            } else {
                if self.sky == 1 { self.skyrow_red[t as usize] } else { self.skyrow[t as usize] }
            };
            if self.sky == 0 && t < 70 && star(col, t) {
                c = 15;
            }
            if invul {
                c = COLORMAPS[NUM_COLORMAPS * 256 + c as usize];
            }
            self.screen[base + y as usize * self.sw] = c;
        }
    }

    // ================================================================ sprites

    fn add_sprites(&mut self, lv: &Level, mobjs: &[Mobj], sec: usize) {
        let light = lv.sectors[sec].light;
        let mut m = lv.sectors[sec].thinglist;
        let mut guard = 0;
        while m != NONE && guard < MAX_MOBJS {
            let mo = &mobjs[m as usize];
            if m != self.player {
                self.project_sprite(mo, light);
            }
            m = mo.snext;
            guard += 1;
        }
    }

    fn project_sprite(&mut self, mo: &Mobj, light: u8) {
        if self.nvis >= MAX_VISSPRITES || mo.sprite == Spr::None {
            return;
        }
        let tr_x = mo.x - self.viewx;
        let tr_y = mo.y - self.viewy;
        let tz = fmul(tr_x, self.viewcos) + fmul(tr_y, self.viewsin);
        if tz < MINZ {
            return;
        }
        let xscale = fdiv(self.projection, tz);
        let yscale = fdiv(self.yprojection, tz);
        let tx = -(fmul(tr_y, self.viewcos) - fmul(tr_x, self.viewsin));
        if tx.abs() > tz << 2 {
            return;
        }
        let frame = mo.frame & !FF_BRIGHT;
        let rot = if models::rotates(mo.sprite) {
            let ang = point_to_angle(tr_x, tr_y);
            (ang.wrapping_sub(mo.angle).wrapping_add(ANG45 / 4) >> 28) as u8
        } else {
            0
        };
        let dims = models::dims(mo.sprite, frame);
        let txl = tx - fx(dims.left);
        let x1 = (self.centerxfrac + fmul(txl, xscale)) >> FRACBITS;
        if x1 >= self.vw as i32 {
            return;
        }
        let txr = txl + fx(dims.w);
        let x2 = ((self.centerxfrac + fmul(txr, xscale)) >> FRACBITS) - 1;
        if x2 < 0 {
            return;
        }
        let gzt = mo.z + fx(dims.top);
        let iscale = fdiv(FRACUNIT, xscale);
        let vx1 = x1.max(0);
        let mut vis = VisSprite {
            x1: vx1,
            x2: x2.min(self.vw as i32 - 1),
            gx: mo.x,
            gy: mo.y,
            gz: mo.z,
            gzt,
            startfrac: iscale.wrapping_mul(vx1 - x1),
            xiscale: iscale,
            scale: xscale,
            yscale,
            texturemid: gzt - self.viewz,
            colormap: 0,
            shadow: mo.flags & MF_SHADOW != 0,
            sprite: mo.sprite,
            frame,
            rot,
            anim: (self.leveltime & 0xffff) as u16,
        };
        vis.colormap = if self.fixedcolormap >= 0 {
            self.fixedcolormap as i16
        } else if mo.frame & FF_BRIGHT != 0 || mo.flags & MF_ALWAYSBRIGHT != 0 {
            0
        } else {
            let row = ((light >> LIGHTSEGSHIFT) as i32 + self.extralight).clamp(0, LIGHTLEVELS as i32 - 1) as usize;
            let idx = ((yscale >> LIGHTSCALESHIFT) as usize).min(MAXLIGHTSCALE - 1);
            self.scalelight[row][idx] as i16
        };
        self.vissprites[self.nvis] = vis;
        self.nvis += 1;
    }

    fn draw_masked(&mut self, lv: &Level, _mobjs: &[Mobj]) {
        // Sort far to near (insertion sort on a small index list).
        let n = self.nvis;
        for i in 0..n {
            self.order[i] = i as u8;
        }
        for i in 1..n {
            let mut j = i;
            while j > 0 && self.vissprites[self.order[j - 1] as usize].scale > self.vissprites[self.order[j] as usize].scale {
                self.order.swap(j - 1, j);
                j -= 1;
            }
        }
        for i in 0..n {
            let vs = self.vissprites[self.order[i] as usize];
            self.draw_sprite(lv, &vs);
        }
        // Remaining masked mid textures, near to far order doesn't matter now.
        for d in (0..self.ndrawsegs).rev() {
            let ds = self.drawsegs[d];
            if let Clip::At(_) = ds.masked {
                self.masked_seg_range(lv, &ds, ds.x1, ds.x2);
            }
        }
    }

    fn draw_sprite(&mut self, lv: &Level, vs: &VisSprite) {
        let mut clipbot = [-2i16; MAX_W];
        let mut cliptop = [-2i16; MAX_W];
        let (x1, x2) = (vs.x1, vs.x2);
        for d in (0..self.ndrawsegs).rev() {
            let ds = self.drawsegs[d];
            if ds.x1 > x2 || ds.x2 < x1 || (ds.silhouette == 0 && ds.masked == Clip::None) {
                continue;
            }
            let r1 = ds.x1.max(x1);
            let r2 = ds.x2.min(x2);
            let (lowscale, scale) = if ds.scale1 > ds.scale2 { (ds.scale2, ds.scale1) } else { (ds.scale1, ds.scale2) };
            let seg = &lv.map.segs[ds.seg as usize];
            let v1 = lv.vert(seg.v1);
            let v2 = lv.vert(seg.v2);
            if scale < vs.yscale
                || (lowscale < vs.yscale && point_on_side(vs.gx, vs.gy, v1.x, v1.y, v2.x - v1.x, v2.y - v1.y) == 0)
            {
                // The wall is behind the sprite: only its masked texture is drawn now.
                if let Clip::At(_) = ds.masked {
                    self.masked_seg_range(lv, &ds, r1, r2);
                }
                continue;
            }
            let mut sil = ds.silhouette;
            if vs.gz >= ds.bsilheight {
                sil &= !SIL_BOTTOM;
            }
            if vs.gzt <= ds.tsilheight {
                sil &= !SIL_TOP;
            }
            for x in r1..=r2 {
                let xi = x as usize;
                if sil & SIL_BOTTOM != 0 && clipbot[xi] == -2 {
                    clipbot[xi] = self.clip_value(ds.sprbottomclip, x, -2);
                }
                if sil & SIL_TOP != 0 && cliptop[xi] == -2 {
                    cliptop[xi] = self.clip_value(ds.sprtopclip, x, -2);
                }
            }
        }
        for x in x1..=x2 {
            let xi = x as usize;
            if clipbot[xi] == -2 {
                clipbot[xi] = self.vh as i16;
            }
            if cliptop[xi] == -2 {
                cliptop[xi] = -1;
            }
        }
        // Render the sprite image, then draw it column by column.
        let dims = models::draw(&mut self.canvas, vs.sprite, vs.frame, vs.rot, vs.anim);
        let cm = vs.colormap as usize;
        let sprtop = self.centeryfrac - fmul(vs.texturemid, vs.yscale);
        let yiscale = fdiv(FRACUNIT, vs.yscale);
        let mut frac = vs.startfrac;
        for x in x1..=x2 {
            let col = frac >> FRACBITS;
            frac = frac.wrapping_add(vs.xiscale);
            if col < 0 || col >= dims.w {
                continue;
            }
            let xi = x as usize;
            let top = ((sprtop + FRACUNIT - 1) >> FRACBITS).max(cliptop[xi] as i32 + 1);
            let bottom = ((sprtop + vs.yscale.wrapping_mul(dims.h)) >> FRACBITS).min(clipbot[xi] as i32 - 1);
            if top > bottom {
                continue;
            }
            let column = self.canvas.column(col as usize);
            let base = self.vy * self.sw + self.vx + xi;
            let mut tf = vs.texturemid.wrapping_add((top - self.centery).wrapping_mul(yiscale));
            let h = dims.h;
            for y in top..=bottom {
                let row = tf >> FRACBITS;
                tf = tf.wrapping_add(yiscale);
                if row < 0 || row >= h {
                    continue;
                }
                let t = column[row as usize];
                if t != 0 {
                    let p = base + y as usize * self.sw;
                    self.screen[p] = if vs.shadow {
                        COLORMAPS[6 * 256 + self.screen[p] as usize]
                    } else {
                        COLORMAPS[cm * 256 + t as usize]
                    };
                }
            }
        }
    }

    fn clip_value(&self, c: Clip, x: i32, none: i16) -> i16 {
        match c {
            Clip::None => none,
            Clip::Screen => self.vh as i16,
            Clip::NegOne => -1,
            Clip::At(base) => self.openings[(base + x) as usize],
        }
    }

    fn masked_seg_range(&mut self, lv: &Level, ds: &DrawSeg, x1: i32, x2: i32) {
        let Clip::At(mbase) = ds.masked else { return };
        let seg = &lv.map.segs[ds.seg as usize];
        let line = lv.line(seg.line as usize);
        let side = lv.sides[line.side[seg.side as usize] as usize];
        let (fi, bi) = self.seg_sectors(lv, seg);
        let Some(bi) = bi else { return };
        let (front, back) = (lv.sectors[fi], lv.sectors[bi]);
        let tex = side.mid;
        if tex == 0 {
            return;
        }
        let v1 = lv.vert(seg.v1);
        let v2 = lv.vert(seg.v2);
        let row = self.light_row(front.light, v1.y == v2.y, v1.x == v2.x);
        let lflags = lv.lines[seg.line as usize].flags;
        let texh = fx(WALLS[tex as usize].h as i32);
        let texmid = if lflags & ML_DONTPEGBOTTOM != 0 {
            front.floor.max(back.floor) + texh - self.viewz
        } else {
            front.ceil.min(back.ceil) - self.viewz
        } + side.yoff;
        let mut scale = ds.scale1 + (x1 - ds.x1) * ds.scalestep;
        for x in x1..=x2 {
            let xi = x as usize;
            let col = self.openings[(mbase + x) as usize];
            if col != i16::MAX {
                let cm = if self.fixedcolormap >= 0 {
                    self.fixedcolormap as usize
                } else {
                    self.scalelight[row][((scale >> LIGHTSCALESHIFT) as usize).min(MAXLIGHTSCALE - 1)] as usize
                };
                let top_clip = self.clip_value(ds.sprtopclip, x, -1) as i32;
                let bot_clip = self.clip_value(ds.sprbottomclip, x, self.vh as i16) as i32;
                let sprtop = self.centeryfrac - fmul(texmid, scale);
                let top = ((sprtop + FRACUNIT - 1) >> FRACBITS).max(top_clip + 1);
                let bottom = ((sprtop + fmul(texh, scale)) >> FRACBITS).min(bot_clip - 1);
                let iscale = (0xffff_ffffu32 / (scale as u32).max(1)) as i32;
                let src = wall_column(tex, col as i32);
                let base = self.vy * self.sw + self.vx + xi;
                let mut frac = texmid.wrapping_add((top - self.centery).wrapping_mul(iscale));
                for y in top..=bottom {
                    let r = frac >> FRACBITS;
                    frac = frac.wrapping_add(iscale);
                    if r < 0 || r >= src.len() as i32 {
                        continue;
                    }
                    let t = src[r as usize];
                    if t != 0 {
                        self.screen[base + y as usize * self.sw] = COLORMAPS[cm * 256 + t as usize];
                    }
                }
                self.openings[(mbase + x) as usize] = i16::MAX;
            }
            scale += ds.scalestep;
        }
    }

    // ================================================================ weapon overlay

    fn draw_psprites(&mut self, lv: &Level, mobjs: &[Mobj], view: &ViewParams) {
        let light = if view.player != NONE {
            lv.sectors[mobjs[view.player as usize].sector as usize].light
        } else {
            255
        };
        for ps in view.psprites.iter().flatten() {
            if ps.sprite == Spr::None {
                continue;
            }
            let bright = ps.frame & FF_BRIGHT != 0;
            let dims = models::draw(&mut self.canvas, ps.sprite, ps.frame & !FF_BRIGHT, 0, ps.anim);
            // Weapons are authored for a 320-wide, 168-high view; scale uniformly to fit.
            let s = fdiv(fx((self.vw << self.detail) as i32), fx(320)).min(fdiv(fx(self.vh as i32), fx(168)));
            let xs = s >> self.detail;
            let ys = if self.square_pixels { s * 6 / 5 } else { s };
            let cm = if self.fixedcolormap >= 0 {
                self.fixedcolormap as usize
            } else if bright {
                0
            } else {
                let row = ((light >> LIGHTSEGSHIFT) as i32 + self.extralight).clamp(0, LIGHTLEVELS as i32 - 1) as usize;
                self.scalelight[row][MAXLIGHTSCALE - 1] as usize
            };
            // (sx, sy) offsets the canvas anchor (left, top) from the bottom centre of the view.
            let sx1 = self.centerx + (fmul(ps.sx - fx(dims.left), xs) >> FRACBITS);
            let sx2 = sx1 + (fmul(fx(dims.w), xs) >> FRACBITS);
            let sy1 = self.vh as i32 + (fmul(ps.sy - fx(dims.top), ys) >> FRACBITS);
            let sy2 = sy1 + (fmul(fx(dims.h), ys) >> FRACBITS);
            let xstep = fdiv(fx(dims.w), fx((sx2 - sx1).max(1)));
            let ystep = fdiv(fx(dims.h), fx((sy2 - sy1).max(1)));
            for x in sx1.max(0)..sx2.min(self.vw as i32) {
                let col = (((x - sx1) * xstep) >> FRACBITS).clamp(0, dims.w - 1) as usize;
                let column = self.canvas.column(col);
                let base = self.vy * self.sw + self.vx + x as usize;
                for y in sy1.max(0)..sy2.min(self.vh as i32) {
                    let row = (((y - sy1) * ystep) >> FRACBITS).clamp(0, dims.h - 1) as usize;
                    let t = column[row];
                    if t != 0 {
                        let p = base + y as usize * self.sw;
                        self.screen[p] = if view.shadow_weapon {
                            COLORMAPS[6 * 256 + self.screen[p] as usize]
                        } else {
                            COLORMAPS[cm * 256 + t as usize]
                        };
                    }
                }
            }
        }
    }
}

const SIL_BOTH: u8 = SIL_TOP | SIL_BOTTOM;

// ---------------------------------------------------------------- sky

fn hash32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^ (x >> 16)
}

fn star(col: usize, t: i32) -> bool {
    hash32((col as u32) * 131 + t as u32 * 7919) % 197 == 0
}

fn init_sky(r: &mut Renderer) {
    // Mountain silhouette: sum of a few "sines" built from integer triangle waves.
    for i in 0..256 {
        let tri = |p: i32, a: i32| {
            let v = ((i as i32 * 256 / p) & 255) - 128;
            (128 - v.abs()) * a / 128
        };
        let h = 18 + tri(64, 10) + tri(37, 7) + tri(23, 4) + (hash32(i as u32) % 3) as i32;
        r.skyprofile[i] = h.clamp(4, 60) as u8;
    }
    for t in 0..128 {
        // Hell sky: dark maroon overhead burning to orange at the horizon.
        r.skyrow_red[t] = if t < 50 {
            14 * 16 + 2 + (t / 12) as u8
        } else if t < 95 {
            4 * 16 + 3 + ((t - 50) / 8) as u8
        } else {
            5 * 16 + 5 + ((t - 95) / 5) as u8
        };
        // Night sky: deep blue at the zenith glowing to ember red at the horizon.
        r.skyrow[t] = if t < 40 {
            9 * 16 + 1 + (t / 14) as u8
        } else if t < 80 {
            11 * 16 + 2 + ((t - 40) / 10) as u8
        } else {
            4 * 16 + 4 + ((t - 80) / 6) as u8
        };
    }
}

fn sky_mountain(sky: i32, d: i32, col: usize) -> u8 {
    let ramp: u8 = if sky == 1 { 14 } else { 1 };
    let shade = (1 + d / 4 + ((hash32(col as u32 >> 1) >> 7) & 1) as i32).clamp(1, 5) as u8;
    ramp * 16 + shade
}
