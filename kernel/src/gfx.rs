//! Software 2D renderer: a back buffer, anti-aliased shapes, blur and text.

use ab_glyph::{Font, FontRef, PxScale, ScaleFont};
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

pub type Color = u32; // 0x00RRGGBB

pub const fn rgb(r: u8, g: u8, b: u8) -> Color {
    ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}
pub const fn hex(c: u32) -> Color {
    c & 0xFF_FFFF
}

#[inline]
fn div255(x: u32) -> u32 {
    let x = x + 128;
    (x + (x >> 8)) >> 8
}

#[inline]
pub fn blend(dst: Color, src: Color, a: u32) -> Color {
    if a >= 255 {
        return src;
    }
    if a == 0 {
        return dst;
    }
    let ia = 255 - a;
    let r = div255(((src >> 16) & 255) * a + ((dst >> 16) & 255) * ia);
    let g = div255(((src >> 8) & 255) * a + ((dst >> 8) & 255) * ia);
    let b = div255((src & 255) * a + (dst & 255) * ia);
    (r << 16) | (g << 8) | b
}

/// Linear mix of two colors, `t` in 0..=255 (0 = a, 255 = b).
pub fn mix(a: Color, b: Color, t: u32) -> Color {
    blend(a, b, t)
}

pub fn lighten(c: Color, t: u32) -> Color {
    mix(c, 0xFFFFFF, t)
}
pub fn darken(c: Color, t: u32) -> Color {
    mix(c, 0x000000, t)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Self {
        Self { x, y, w, h }
    }
    pub fn contains(&self, px: i32, py: i32) -> bool {
        px >= self.x && py >= self.y && px < self.x + self.w && py < self.y + self.h
    }
    pub fn right(&self) -> i32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }
    pub fn cx(&self) -> i32 {
        self.x + self.w / 2
    }
    pub fn cy(&self) -> i32 {
        self.y + self.h / 2
    }
    pub fn inset(&self, d: i32) -> Rect {
        Rect::new(self.x + d, self.y + d, (self.w - 2 * d).max(0), (self.h - 2 * d).max(0))
    }
    pub fn intersect(&self, o: &Rect) -> Rect {
        let x0 = self.x.max(o.x);
        let y0 = self.y.max(o.y);
        let x1 = self.right().min(o.right());
        let y1 = self.bottom().min(o.bottom());
        Rect::new(x0, y0, (x1 - x0).max(0), (y1 - y0).max(0))
    }
    pub fn offset(&self, dx: i32, dy: i32) -> Rect {
        Rect::new(self.x + dx, self.y + dy, self.w, self.h)
    }
}

fn isqrt(n: u32) -> u32 {
    if n < 2 {
        return n;
    }
    let mut x = n;
    let mut y = (x + 1) / 2;
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    x
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum F {
    Ui,
    Bold,
    Mono,
}

struct Glyph {
    bm: Vec<u8>,
    w: i32,
    h: i32,
    left: i32,
    top: i32,
    adv: f32,
}

pub struct Fonts {
    ui: FontRef<'static>,
    bold: FontRef<'static>,
    mono: FontRef<'static>,
    cache: BTreeMap<(F, u16, char), Glyph>,
}

impl Fonts {
    fn new() -> Self {
        Self {
            ui: FontRef::try_from_slice(include_bytes!("../assets/Inter-Regular.otf")).expect("ui font"),
            bold: FontRef::try_from_slice(include_bytes!("../assets/Inter-SemiBold.otf")).expect("bold font"),
            mono: FontRef::try_from_slice(include_bytes!("../assets/JetBrainsMono-Regular.ttf")).expect("mono font"),
            cache: BTreeMap::new(),
        }
    }

    fn glyph(&mut self, f: F, size: u16, ch: char) -> &Glyph {
        if !self.cache.contains_key(&(f, size, ch)) {
            let font: &FontRef<'static> = match f {
                F::Ui => &self.ui,
                F::Bold => &self.bold,
                F::Mono => &self.mono,
            };
            let sc = PxScale::from(size as f32);
            let sf = font.as_scaled(sc);
            let g = sf.scaled_glyph(ch);
            let adv = sf.h_advance(g.id);
            let mut glyph = Glyph { bm: Vec::new(), w: 0, h: 0, left: 0, top: 0, adv };
            if let Some(og) = font.outline_glyph(g) {
                let b = og.px_bounds();
                let w = libm::ceilf(b.max.x - b.min.x) as i32 + 1;
                let h = libm::ceilf(b.max.y - b.min.y) as i32 + 1;
                let mut bm = vec![0u8; (w * h) as usize];
                og.draw(|x, y, c| {
                    let i = (y as i32 * w + x as i32) as usize;
                    if i < bm.len() {
                        bm[i] = (c * 255.0) as u8;
                    }
                });
                glyph = Glyph { bm, w, h, left: b.min.x as i32, top: b.min.y as i32, adv };
            }
            self.cache.insert((f, size, ch), glyph);
        }
        &self.cache[&(f, size, ch)]
    }
}

pub struct Canvas {
    pub w: i32,
    pub h: i32,
    pub buf: Vec<u32>,
    pub clip: Rect,
    pub fonts: Fonts,
}

impl Canvas {
    pub fn new(w: i32, h: i32) -> Self {
        Self { w, h, buf: vec![0; (w * h) as usize], clip: Rect::new(0, 0, w, h), fonts: Fonts::new() }
    }

    pub fn screen(&self) -> Rect {
        Rect::new(0, 0, self.w, self.h)
    }

    pub fn set_clip(&mut self, r: Rect) -> Rect {
        let old = self.clip;
        self.clip = r.intersect(&self.screen());
        old
    }

    #[inline]
    pub fn px(&mut self, x: i32, y: i32, c: Color, a: u32) {
        if x < self.clip.x || y < self.clip.y || x >= self.clip.right() || y >= self.clip.bottom() {
            return;
        }
        let i = (y * self.w + x) as usize;
        self.buf[i] = blend(self.buf[i], c, a);
    }

    pub fn get(&self, x: i32, y: i32) -> Color {
        let x = x.clamp(0, self.w - 1);
        let y = y.clamp(0, self.h - 1);
        self.buf[(y * self.w + x) as usize]
    }

    pub fn clear(&mut self, c: Color) {
        self.buf.fill(c);
    }

    pub fn fill(&mut self, r: Rect, c: Color) {
        let r = r.intersect(&self.clip);
        for y in r.y..r.bottom() {
            let o = (y * self.w) as usize;
            self.buf[o + r.x as usize..o + r.right() as usize].fill(c);
        }
    }

    pub fn fill_a(&mut self, r: Rect, c: Color, a: u32) {
        if a >= 255 {
            return self.fill(r, c);
        }
        let r = r.intersect(&self.clip);
        for y in r.y..r.bottom() {
            let o = (y * self.w) as usize;
            for p in &mut self.buf[o + r.x as usize..o + r.right() as usize] {
                *p = blend(*p, c, a);
            }
        }
    }

    pub fn vgrad(&mut self, r: Rect, top: Color, bot: Color) {
        let rr = r.intersect(&self.clip);
        for y in rr.y..rr.bottom() {
            let t = (((y - r.y) * 255) / r.h.max(1)) as u32;
            let c = mix(top, bot, t);
            let o = (y * self.w) as usize;
            self.buf[o + rr.x as usize..o + rr.right() as usize].fill(c);
        }
    }

    /// Coverage (0..=255) of pixel (x, y) inside a rounded rectangle.
    #[inline]
    fn rr_cov(r: &Rect, rad: i32, x: i32, y: i32) -> u32 {
        let lx = x - r.x;
        let ly = y - r.y;
        let (mut ox, mut oy) = (0, 0);
        if lx < rad {
            ox = rad - lx;
        } else if lx >= r.w - rad {
            ox = lx - (r.w - rad) + 1;
        }
        if ly < rad {
            oy = rad - ly;
        } else if ly >= r.h - rad {
            oy = ly - (r.h - rad) + 1;
        }
        if ox == 0 || oy == 0 {
            return 255;
        }
        // distance from the corner circle centre to the pixel centre, 1/16 px units.
        let dx = ox * 16 - 8;
        let dy = oy * 16 - 8;
        let d = isqrt((dx * dx + dy * dy) as u32) as i32;
        let c = (rad * 16 - d + 8).clamp(0, 16);
        (c * 255 / 16) as u32
    }

    /// After drawing square content into `r`, round its bottom corners again
    /// using `saved` (a copy of the bottom `rad` rows taken before drawing).
    pub fn round_bottom(&mut self, r: Rect, rad: i32, saved: &[u32]) {
        for j in 0..rad {
            let y = r.bottom() - rad + j;
            if y < 0 || y >= self.h {
                continue;
            }
            for i in (0..rad).chain(r.w - rad..r.w) {
                let x = r.x + i;
                if x < 0 || x >= self.w {
                    continue;
                }
                let cov = Self::rr_cov(&r, rad, x, y);
                if cov < 255 {
                    let old = saved[(j * r.w + i) as usize];
                    let idx = (y * self.w + x) as usize;
                    self.buf[idx] = blend(old, self.buf[idx], cov);
                }
            }
        }
    }

    /// Copy of the pixels under `r` (clamped to the screen edge).
    pub fn grab_rect(&self, r: Rect) -> Vec<u32> {
        let mut v = vec![0u32; (r.w * r.h).max(0) as usize];
        for j in 0..r.h {
            for i in 0..r.w {
                v[(j * r.w + i) as usize] = self.get(r.x + i, r.y + j);
            }
        }
        v
    }

    /// Put back the original pixels outside the rounded corners of `r`.
    pub fn restore_corners(&mut self, r: Rect, rad: i32, saved: &[u32]) {
        let rad = rad.min(r.w / 2).min(r.h / 2);
        for j in (0..rad).chain(r.h - rad..r.h) {
            for i in (0..rad).chain(r.w - rad..r.w) {
                let (x, y) = (r.x + i, r.y + j);
                if x < 0 || y < 0 || x >= self.w || y >= self.h {
                    continue;
                }
                let cov = Self::rr_cov(&r, rad, x, y);
                if cov < 255 {
                    let idx = (y * self.w + x) as usize;
                    self.buf[idx] = blend(saved[(j * r.w + i) as usize], self.buf[idx], cov);
                }
            }
        }
    }

    pub fn grab_bottom(&self, r: Rect, rad: i32) -> Vec<u32> {
        let mut v = vec![0u32; (rad * r.w) as usize];
        for j in 0..rad {
            for i in 0..r.w {
                v[(j * r.w + i) as usize] = self.get(r.x + i, r.bottom() - rad + j);
            }
        }
        v
    }

    /// Fill a rounded rectangle; the colour comes from `f(x, y)`.
    pub fn rrect_with(&mut self, r: Rect, rad: i32, alpha: u32, f: impl Fn(i32, i32) -> Color) {
        let rad = rad.min(r.w / 2).min(r.h / 2).max(0);
        let vis = r.intersect(&self.clip);
        for y in vis.y..vis.bottom() {
            let o = (y * self.w) as usize;
            let row_corner = y - r.y < rad || y >= r.bottom() - rad;
            for x in vis.x..vis.right() {
                let cov = if row_corner || x - r.x < rad || x >= r.right() - rad { Self::rr_cov(&r, rad, x, y) } else { 255 };
                if cov == 0 {
                    continue;
                }
                let a = if alpha >= 255 { cov } else { div255(cov * alpha) };
                let i = o + x as usize;
                self.buf[i] = blend(self.buf[i], f(x, y), a);
            }
        }
    }

    pub fn rrect(&mut self, r: Rect, rad: i32, c: Color, a: u32) {
        self.rrect_with(r, rad, a, |_, _| c);
    }

    pub fn rrect_vgrad(&mut self, r: Rect, rad: i32, top: Color, bot: Color) {
        let (y0, h) = (r.y, r.h.max(1));
        self.rrect_with(r, rad, 255, move |_, y| mix(top, bot, (((y - y0) * 255) / h).clamp(0, 255) as u32));
    }

    pub fn rrect_dgrad(&mut self, r: Rect, rad: i32, c0: Color, c1: Color) {
        let (x0, y0, w, h) = (r.x, r.y, r.w.max(1), r.h.max(1));
        self.rrect_with(r, rad, 255, move |x, y| {
            let t = ((x - x0) * 255 / w + (y - y0) * 255 / h) / 2;
            mix(c0, c1, t.clamp(0, 255) as u32)
        });
    }

    pub fn rrect_stroke(&mut self, r: Rect, rad: i32, thick: i32, c: Color, a: u32) {
        let rad = rad.min(r.w / 2).min(r.h / 2).max(0);
        let inner = r.inset(thick);
        let irad = (rad - thick).max(0);
        let vis = r.intersect(&self.clip);
        for y in vis.y..vis.bottom() {
            let o = (y * self.w) as usize;
            let in_rows = y >= inner.y && y < inner.bottom();
            for x in vis.x..vis.right() {
                if in_rows && x >= inner.x + irad.max(thick) && x < inner.right() - irad.max(thick) && y >= inner.y + irad && y < inner.bottom() - irad {
                    continue;
                }
                let outer = Self::rr_cov(&r, rad, x, y);
                let inn = if inner.contains(x, y) { Self::rr_cov(&inner, irad, x, y) } else { 0 };
                let cov = outer.saturating_sub(inn);
                if cov == 0 {
                    continue;
                }
                let i = o + x as usize;
                self.buf[i] = blend(self.buf[i], c, div255(cov * a.min(255)));
            }
        }
    }

    pub fn circle(&mut self, cx: i32, cy: i32, rad: i32, c: Color, a: u32) {
        self.rrect(Rect::new(cx - rad, cy - rad, rad * 2, rad * 2), rad, c, a);
    }

    pub fn ring(&mut self, cx: i32, cy: i32, rad: i32, thick: i32, c: Color, a: u32) {
        self.rrect_stroke(Rect::new(cx - rad, cy - rad, rad * 2, rad * 2), rad, thick, c, a);
    }

    /// Ring sector keeping pixels where `keep(dx, dy)` holds (used for Wi-Fi arcs).
    pub fn ring_part(&mut self, cx: i32, cy: i32, rad: i32, thick: i32, c: Color, keep: impl Fn(i32, i32) -> bool) {
        for y in cy - rad..=cy + rad {
            for x in cx - rad..=cx + rad {
                if !keep(x - cx, y - cy) {
                    continue;
                }
                let d2 = ((x - cx) * 16) * ((x - cx) * 16) + ((y - cy) * 16) * ((y - cy) * 16);
                let d = isqrt(d2 as u32) as i32;
                let o = (rad * 16 - d + 8).clamp(0, 16);
                let i = ((rad - thick) * 16 - d + 8).clamp(0, 16);
                let cov = (o - i).max(0) * 255 / 16;
                if cov > 0 {
                    self.px(x, y, c, cov as u32);
                }
            }
        }
    }

    /// Soft drop shadow around a rounded rect.
    pub fn shadow(&mut self, r: Rect, rad: i32, spread: i32, dy: i32, strength: u32) {
        let n = spread.max(1);
        for i in (1..=n).rev() {
            let k = (n - i + 1) as u32;
            let a = strength * k * k / (n as u32 * n as u32 * 6);
            let rr = Rect::new(r.x - i, r.y - i + dy, r.w + 2 * i, r.h + 2 * i);
            self.rrect(rr, rad + i, 0x000000, a.min(255));
        }
    }

    pub fn line(&mut self, x0: i32, y0: i32, x1: i32, y1: i32, thick: i32, c: Color, a: u32) {
        let (mut x, mut y) = (x0, y0);
        let dx = (x1 - x0).abs();
        let dy = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            if thick <= 1 {
                self.px(x, y, c, a);
            } else {
                self.circle(x, y, thick / 2, c, a);
            }
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x += sx;
            }
            if e2 <= dx {
                err += dx;
                y += sy;
            }
        }
    }

    /// Box blur of a screen region (used for frosted glass).
    pub fn blur(&mut self, r: Rect, radius: i32, passes: i32) {
        let r = r.intersect(&self.screen());
        if r.w < 2 || r.h < 2 || radius < 1 {
            return;
        }
        let (w, h) = (r.w as usize, r.h as usize);
        let mut tmp = vec![0u32; w * h];
        for _ in 0..passes {
            // horizontal
            for y in 0..h {
                let row = ((r.y as usize + y) * self.w as usize) + r.x as usize;
                let (mut sr, mut sg, mut sb) = (0i32, 0i32, 0i32);
                let rad = radius as isize;
                let get = |buf: &Vec<u32>, i: isize| -> u32 { buf[row + i.clamp(0, w as isize - 1) as usize] };
                for i in -rad..=rad {
                    let p = get(&self.buf, i);
                    sr += ((p >> 16) & 255) as i32;
                    sg += ((p >> 8) & 255) as i32;
                    sb += (p & 255) as i32;
                }
                let n = (2 * radius + 1) as i32;
                for x in 0..w {
                    tmp[y * w + x] = (((sr / n) as u32) << 16) | (((sg / n) as u32) << 8) | (sb / n) as u32;
                    let add = get(&self.buf, x as isize + rad + 1);
                    let sub = get(&self.buf, x as isize - rad);
                    sr += ((add >> 16) & 255) as i32 - ((sub >> 16) & 255) as i32;
                    sg += ((add >> 8) & 255) as i32 - ((sub >> 8) & 255) as i32;
                    sb += (add & 255) as i32 - (sub & 255) as i32;
                }
            }
            // vertical
            for x in 0..w {
                let (mut sr, mut sg, mut sb) = (0i32, 0i32, 0i32);
                let rad = radius as isize;
                let get = |t: &Vec<u32>, i: isize| -> u32 { t[i.clamp(0, h as isize - 1) as usize * w + x] };
                for i in -rad..=rad {
                    let p = get(&tmp, i);
                    sr += ((p >> 16) & 255) as i32;
                    sg += ((p >> 8) & 255) as i32;
                    sb += (p & 255) as i32;
                }
                let n = (2 * radius + 1) as i32;
                for y in 0..h {
                    let o = (r.y as usize + y) * self.w as usize + r.x as usize + x;
                    self.buf[o] = (((sr / n) as u32) << 16) | (((sg / n) as u32) << 8) | (sb / n) as u32;
                    let add = get(&tmp, y as isize + rad + 1);
                    let sub = get(&tmp, y as isize - rad);
                    sr += ((add >> 16) & 255) as i32 - ((sub >> 16) & 255) as i32;
                    sg += ((add >> 8) & 255) as i32 - ((sub >> 8) & 255) as i32;
                    sb += (add & 255) as i32 - (sub & 255) as i32;
                }
            }
        }
    }

    /// Frosted glass panel: blur what is behind, then tint.
    pub fn glass(&mut self, r: Rect, rad: i32, tint: Color, tint_a: u32, blur_r: i32) {
        let vis = r.intersect(&self.clip);
        if vis.w <= 0 || vis.h <= 0 {
            return;
        }
        let saved = if rad > 0 { Some(self.grab_rect(r)) } else { None };
        self.blur(vis, blur_r, 2);
        if let Some(sv) = saved {
            self.restore_corners(r, rad, &sv);
        }
        self.rrect(r, rad, tint, tint_a);
    }

    pub fn blit(&mut self, src: &[u32], sw: i32, sh: i32, dx: i32, dy: i32) {
        for y in 0..sh {
            let ty = dy + y;
            if ty < self.clip.y || ty >= self.clip.bottom() {
                continue;
            }
            for x in 0..sw {
                let tx = dx + x;
                if tx < self.clip.x || tx >= self.clip.right() {
                    continue;
                }
                self.buf[(ty * self.w + tx) as usize] = src[(y * sw + x) as usize];
            }
        }
    }

    // ---- text ---------------------------------------------------------------

    pub fn text_width(&mut self, f: F, size: u16, s: &str) -> i32 {
        let mut w = 0.0f32;
        for ch in s.chars() {
            w += self.fonts.glyph(f, size, ch).adv;
        }
        (w + 0.5) as i32
    }

    /// Draw text vertically centred on `cy`. Returns the x after the last glyph.
    pub fn text(&mut self, f: F, size: u16, x: i32, cy: i32, s: &str, col: Color, a: u32) -> i32 {
        let baseline = cy + (size as i32 * 36) / 100 + 1;
        let mut pen = x as f32;
        for ch in s.chars() {
            let (gw, gh, left, top, adv, ptr) = {
                let g = self.fonts.glyph(f, size, ch);
                (g.w, g.h, g.left, g.top, g.adv, g.bm.as_ptr())
            };
            if gw > 0 {
                let gx = pen as i32 + left;
                let gy = baseline + top;
                // SAFETY: the glyph cache is not touched while we read this bitmap.
                let bm = unsafe { core::slice::from_raw_parts(ptr, (gw * gh) as usize) };
                for yy in 0..gh {
                    let py = gy + yy;
                    if py < self.clip.y || py >= self.clip.bottom() {
                        continue;
                    }
                    for xx in 0..gw {
                        let cov = bm[(yy * gw + xx) as usize] as u32;
                        if cov == 0 {
                            continue;
                        }
                        let px = gx + xx;
                        if px < self.clip.x || px >= self.clip.right() {
                            continue;
                        }
                        let i = (py * self.w + px) as usize;
                        self.buf[i] = blend(self.buf[i], col, div255(cov * a.min(255)));
                    }
                }
            }
            pen += adv;
        }
        pen as i32
    }

    pub fn text_c(&mut self, f: F, size: u16, cx: i32, cy: i32, s: &str, col: Color, a: u32) {
        let w = self.text_width(f, size, s);
        self.text(f, size, cx - w / 2, cy, s, col, a);
    }

    pub fn text_r(&mut self, f: F, size: u16, right: i32, cy: i32, s: &str, col: Color, a: u32) {
        let w = self.text_width(f, size, s);
        self.text(f, size, right - w, cy, s, col, a);
    }

    /// Shorten `s` with an ellipsis so it fits `max_w`.
    pub fn fit(&mut self, f: F, size: u16, s: &str, max_w: i32) -> String {
        if self.text_width(f, size, s) <= max_w {
            return String::from(s);
        }
        let mut out = String::new();
        for ch in s.chars() {
            let mut t = out.clone();
            t.push(ch);
            t.push('\u{2026}');
            if self.text_width(f, size, &t) > max_w {
                break;
            }
            out.push(ch);
        }
        out.push('\u{2026}');
        out
    }

    pub fn mono_cell(&mut self, size: u16) -> (i32, i32) {
        let w = self.fonts.glyph(F::Mono, size, 'M').adv;
        ((w + 0.5) as i32, size as i32 + size as i32 / 3)
    }
}
