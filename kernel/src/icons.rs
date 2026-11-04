//! Procedurally drawn icons and the Aeris logo. Everything scales with the target size.

use crate::gfx::*;
use crate::theme::Exp;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Icon {
    Aeris,
    Files,
    Terminal,
    PowerShell,
    Cmd,
    Settings,
    Notes,
    Trash,
    WinStart,
}

/// Draw an app icon filling `r` (square).
pub fn app(cv: &mut Canvas, icon: Icon, r: Rect, exp: Exp) {
    let s = r.w.min(r.h);
    let x = r.x;
    let y = r.y;
    let f = |n: i32| n * s / 64;
    let sq = f(14); // squircle radius
    match icon {
        Icon::Aeris => logo(cv, r),
        Icon::WinStart => {
            let g = f(4);
            let q = (s - g - f(8)) / 2;
            for (i, j) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                let rr = Rect::new(x + f(4) + i * (q + g), y + f(4) + j * (q + g), q, q);
                cv.rrect_dgrad(rr, f(2), 0x4CC2FF, 0x0A6CDC);
            }
        }
        Icon::Files => {
            if exp == Exp::Win {
                // classic yellow folder
                cv.rrect(Rect::new(x + f(5), y + f(10), f(22), f(14)), f(4), 0xE0A800, 255);
                cv.rrect_vgrad(Rect::new(x + f(4), y + f(18), f(56), f(36)), f(5), 0xFFD865, 0xF7B500);
                cv.rrect(Rect::new(x + f(4), y + f(18), f(56), f(5)), f(2), 0xFFFFFF, 70);
            } else {
                cv.shadow(Rect::new(x + f(3), y + f(3), s - f(6), s - f(6)), sq, f(3), f(1), 90);
                cv.rrect_vgrad(Rect::new(x + f(3), y + f(3), s - f(6), s - f(6)), sq, 0x6CC3FF, 0x1379F2);
                // white folder glyph
                cv.rrect(Rect::new(x + f(16), y + f(19), f(14), f(9)), f(3), 0xFFFFFF, 255);
                cv.rrect(Rect::new(x + f(16), y + f(24), f(32), f(22)), f(4), 0xFFFFFF, 255);
            }
        }
        Icon::Terminal | Icon::PowerShell | Icon::Cmd => {
            let (c0, c1) = match icon {
                Icon::PowerShell => (0x2E6FD6, 0x0B3A8F),
                Icon::Cmd => (0x2B2B2B, 0x0C0C0C),
                _ => {
                    if exp == Exp::Mac {
                        (0x3A3A3C, 0x0F0F10)
                    } else {
                        (0x3B3B3B, 0x161616)
                    }
                }
            };
            let body = Rect::new(x + f(3), y + f(5), s - f(6), s - f(10));
            if exp == Exp::Mac {
                cv.shadow(body, sq, f(3), f(1), 90);
            }
            cv.rrect_vgrad(body, if exp == Exp::Mac { sq } else { f(8) }, c0, c1);
            if icon == Icon::Terminal && exp == Exp::Win {
                cv.rrect(Rect::new(x + f(3), y + f(5), s - f(6), f(7)), f(3), 0x4CC2FF, 255);
            }
            cv.line(x + f(18), y + f(26), x + f(28), y + f(33), f(5).max(2), 0xFFFFFF, 255);
            cv.line(x + f(28), y + f(33), x + f(18), y + f(40), f(5).max(2), 0xFFFFFF, 255);
            cv.line(x + f(33), y + f(42), x + f(46), y + f(42), f(5).max(2), 0xFFFFFF, 255);
        }
        Icon::Settings => {
            let body = Rect::new(x + f(3), y + f(3), s - f(6), s - f(6));
            let (c0, c1) = if exp == Exp::Mac { (0xB6B8C2, 0x6E7080) } else { (0x7C8799, 0x4B5666) };
            if exp == Exp::Mac {
                cv.shadow(body, sq, f(3), f(1), 90);
                cv.rrect_vgrad(body, sq, c0, c1);
            }
            let cx = x + s / 2;
            let cy = y + s / 2;
            let col = if exp == Exp::Mac { 0xFFFFFF } else { 0x9AA7BC };
            for (dx, dy) in [(1, 0), (0, 1), (-1, 0), (0, -1)] {
                cv.line(cx + dx * f(15), cy + dy * f(15), cx + dx * f(22), cy + dy * f(22), f(9), col, 255);
            }
            for (dx, dy) in [(1, 1), (-1, 1), (-1, -1), (1, -1)] {
                cv.line(cx + dx * f(11), cy + dy * f(11), cx + dx * f(16), cy + dy * f(16), f(9), col, 255);
            }
            cv.circle(cx, cy, f(18), col, 255);
            cv.circle(cx, cy, f(8), if exp == Exp::Mac { c1 } else { 0x39424F }, 255);
        }
        Icon::Notes => {
            let body = Rect::new(x + f(8), y + f(4), s - f(16), s - f(8));
            if exp == Exp::Mac {
                cv.shadow(body, f(10), f(3), f(1), 90);
            }
            cv.rrect(body, f(8), 0xFFFFFF, 255);
            let top = if exp == Exp::Mac { 0xFFC83D } else { 0x3AA0F0 };
            cv.rrect(Rect::new(body.x, body.y, body.w, f(14)), f(8), top, 255);
            cv.fill(Rect::new(body.x, body.y + f(8), body.w, f(6)), top);
            for i in 0..4 {
                cv.rrect(Rect::new(body.x + f(8), body.y + f(24) + i * f(9), body.w - f(16) - if i == 3 { f(12) } else { 0 }, f(3).max(1)), 1, 0xC4C7CE, 255);
            }
        }
        Icon::Trash => {
            let cx = x + s / 2;
            cv.rrect(Rect::new(cx - f(14), y + f(18), f(28), f(36)), f(6), 0xD5DAE3, 235);
            cv.rrect(Rect::new(cx - f(18), y + f(12), f(36), f(6)), f(3), 0xB8C0CE, 255);
            cv.rrect(Rect::new(cx - f(5), y + f(7), f(10), f(6)), f(2), 0xB8C0CE, 255);
            for i in -1..=1 {
                cv.rrect(Rect::new(cx + i * f(8) - f(1), y + f(24), f(3).max(1), f(24)), 1, 0x8E97A8, 200);
            }
        }
    }
}

/// The Aeris mark: a gradient squircle with a white "A".
pub fn logo(cv: &mut Canvas, r: Rect) {
    let s = r.w.min(r.h);
    let f = |n: i32| n * s / 64;
    let (x, y) = (r.x, r.y);
    let body = Rect::new(x + f(2), y + f(2), s - f(4), s - f(4));
    let (x0, y0, w, h) = (body.x, body.y, body.w.max(1), body.h.max(1));
    cv.rrect_with(body, f(16), 255, move |px, py| {
        let t = ((px - x0) * 255 / w + (py - y0) * 255 / h) / 2;
        let t = t.clamp(0, 255) as u32;
        if t < 128 {
            mix(0x22D3EE, 0x6366F1, t * 2)
        } else {
            mix(0x6366F1, 0xA855F7, (t - 128) * 2)
        }
    });
    let th = f(7).max(2);
    cv.line(x + f(32), y + f(15), x + f(19), y + f(48), th, 0xFFFFFF, 255);
    cv.line(x + f(32), y + f(15), x + f(45), y + f(48), th, 0xFFFFFF, 255);
    cv.line(x + f(24), y + f(37), x + f(40), y + f(37), th, 0xFFFFFF, 255);
}

// ---- small monochrome glyphs used by shell chrome and apps -------------------

pub fn wifi(cv: &mut Canvas, cx: i32, cy: i32, s: i32, c: Color) {
    let base_y = cy + s / 3;
    for i in 1..=3 {
        cv.ring_part(cx, base_y, s * i / 3 + 1, (s / 8).max(2), c, |dx, dy| dy < 0 && -dy * 10 > dx.abs() * 7);
    }
    cv.circle(cx, base_y, (s / 9).max(1), c, 255);
}

pub fn volume(cv: &mut Canvas, cx: i32, cy: i32, s: i32, c: Color) {
    let u = s / 6;
    cv.fill(Rect::new(cx - 3 * u, cy - u, 2 * u, 2 * u), c);
    cv.line(cx - u, cy - u, cx + u, cy - 3 * u, 1, c, 255);
    cv.line(cx - u, cy + u, cx + u, cy + 3 * u, 1, c, 255);
    cv.fill(Rect::new(cx - u, cy - u, u + 1, 2 * u), c);
    cv.fill(Rect::new(cx + u - 1, cy - 3 * u, 2, 6 * u), c);
    cv.ring_part(cx + u, cy, 2 * u + 2, 2, c, |dx, _| dx > 0);
}

pub fn battery(cv: &mut Canvas, cx: i32, cy: i32, s: i32, c: Color, level: i32) {
    let w = s;
    let h = s * 5 / 11;
    let r = Rect::new(cx - w / 2, cy - h / 2, w, h);
    cv.rrect_stroke(r, 3, 1, c, 220);
    let inner = r.inset(3);
    cv.rrect(Rect::new(inner.x, inner.y, inner.w * level / 100, inner.h), 1, c, 255);
    cv.rrect(Rect::new(r.right() + 1, cy - h / 6, 2, h / 3), 1, c, 200);
}

pub fn chevron(cv: &mut Canvas, cx: i32, cy: i32, s: i32, c: Color, dir: i32) {
    // dir: 0 up, 1 right, 2 down, 3 left
    let (dx, dy) = match dir {
        0 => (0, s / 3),
        1 => (-s / 3, 0),
        2 => (0, -s / 3),
        _ => (s / 3, 0),
    };
    let cx = cx + dx;
    let cy = cy + dy;
    let a = s / 2;
    match dir {
        0 => {
            cv.line(cx - a, cy + a / 2, cx, cy - a / 2, 2, c, 255);
            cv.line(cx, cy - a / 2, cx + a, cy + a / 2, 2, c, 255);
        }
        2 => {
            cv.line(cx - a, cy - a / 2, cx, cy + a / 2, 2, c, 255);
            cv.line(cx, cy + a / 2, cx + a, cy - a / 2, 2, c, 255);
        }
        1 => {
            cv.line(cx - a / 2, cy - a, cx + a / 2, cy, 2, c, 255);
            cv.line(cx + a / 2, cy, cx - a / 2, cy + a, 2, c, 255);
        }
        _ => {
            cv.line(cx + a / 2, cy - a, cx - a / 2, cy, 2, c, 255);
            cv.line(cx - a / 2, cy, cx + a / 2, cy + a, 2, c, 255);
        }
    }
}

pub fn search(cv: &mut Canvas, cx: i32, cy: i32, s: i32, c: Color) {
    let r = s / 3;
    cv.ring(cx - s / 8, cy - s / 8, r, 2, c, 255);
    cv.line(cx + r - s / 8 - 1, cy + r - s / 8 - 1, cx + s / 2 - 1, cy + s / 2 - 1, 2, c, 255);
}

pub fn power(cv: &mut Canvas, cx: i32, cy: i32, s: i32, c: Color) {
    let r = s / 2 - 1;
    cv.ring_part(cx, cy, r, 2, c, |dx, dy| !(dy < -r / 3 && dx.abs() < r / 3 + 2));
    cv.line(cx, cy - r, cx, cy - 1, 2, c, 255);
}

pub fn folder_small(cv: &mut Canvas, r: Rect, exp: Exp) {
    let s = r.w.min(r.h);
    let f = |n: i32| n * s / 24;
    let (a, b) = if exp == Exp::Win { (0xFFD865, 0xF2B300) } else { (0x6CC3FF, 0x2486F5) };
    cv.rrect(Rect::new(r.x + f(1), r.y + f(4), f(10), f(6)), f(2).max(1), darken(b, 40), 255);
    cv.rrect_vgrad(Rect::new(r.x + f(1), r.y + f(7), f(22), f(15)), f(3).max(1), a, b);
}

pub fn file_small(cv: &mut Canvas, r: Rect, accent: Color, dark: bool) {
    let s = r.w.min(r.h);
    let f = |n: i32| n * s / 24;
    let body = Rect::new(r.x + f(4), r.y + f(1), f(16), f(22));
    cv.rrect(body, f(3).max(1), if dark { 0xD9DCE3 } else { 0xFFFFFF }, 255);
    cv.rrect_stroke(body, f(3).max(1), 1, if dark { 0x9097A3 } else { 0xB9BEC8 }, 255);
    for i in 0..3 {
        cv.fill(Rect::new(body.x + f(3), body.y + f(7) + i * f(4), f(10), 1.max(f(1))), mix(accent, 0xFFFFFF, 90));
    }
}
