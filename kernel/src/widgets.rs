//! Small shared UI pieces: buttons, switches and the experience preview.

use crate::gfx::*;
use crate::icons;
use crate::theme::{Exp, Pal};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum BtnStyle {
    Primary,
    Secondary,
    Ghost,
}

pub fn button(cv: &mut Canvas, r: Rect, label: &str, style: BtnStyle, pal: &Pal, exp: Exp, hover: bool) {
    let rad = if exp == Exp::Win { 5 } else { r.h / 2 };
    match style {
        BtnStyle::Primary => {
            let c = if hover { lighten(pal.accent, 28) } else { pal.accent };
            cv.rrect(r, rad, c, 255);
            cv.text_c(F::Bold, 14, r.cx(), r.cy(), label, 0xFFFFFF, 255);
        }
        BtnStyle::Secondary => {
            let bg = if pal.dark { 0x3A3A3E } else { 0xFFFFFF };
            let bg = if hover { mix(bg, pal.accent, 28) } else { bg };
            cv.rrect(r, rad, bg, 255);
            cv.rrect_stroke(r, rad, 1, pal.line, 255);
            cv.text_c(F::Ui, 14, r.cx(), r.cy(), label, pal.text, 255);
        }
        BtnStyle::Ghost => {
            if hover {
                cv.rrect(r, rad, pal.text, 22);
            }
            cv.text_c(F::Ui, 14, r.cx(), r.cy(), label, pal.text, 255);
        }
    }
}

pub fn switch(cv: &mut Canvas, r: Rect, on: bool, pal: &Pal) {
    let (bg, knob) = if on { (pal.accent, 0xFFFFFF) } else { (if pal.dark { 0x555559 } else { 0xC8C8CC }, 0xFFFFFF) };
    cv.rrect(r, r.h / 2, bg, 255);
    let k = r.h - 6;
    let kx = if on { r.right() - k - 3 } else { r.x + 3 };
    cv.circle(kx + k / 2, r.cy(), k / 2, knob, 255);
}

pub fn check_badge(cv: &mut Canvas, cx: i32, cy: i32, rad: i32, c: Color) {
    cv.circle(cx, cy, rad, c, 255);
    cv.line(cx - rad / 2, cy, cx - rad / 6, cy + rad / 2, 2, 0xFFFFFF, 255);
    cv.line(cx - rad / 6, cy + rad / 2, cx + rad / 2, cy - rad / 3, 2, 0xFFFFFF, 255);
}

/// A miniature of a whole desktop, used by Setup and Settings to show an experience.
pub fn exp_preview(cv: &mut Canvas, r: Rect, exp: Exp, dark: bool, accent: Color) {
    let rad = 10;
    let saved = cv.grab_rect(r);
    let (c0, c1) = match (exp, dark) {
        (Exp::Win, false) => (0x7FB6FF, 0x5B3FE0),
        (Exp::Win, true) => (0x0B2A78, 0x2A1A7A),
        (Exp::Mac, false) => (0xFFB0D0, 0x7AB8FF),
        (Exp::Mac, true) => (0xA02A70, 0x2A2FD0),
    };
    cv.rrect_dgrad(r, rad, c0, c1);
    let glass = if dark { 0x14161C } else { 0xFFFFFF };
    let prev = cv.set_clip(r);
    match exp {
        Exp::Win => {
            let bar = Rect::new(r.x, r.bottom() - r.h / 8, r.w, r.h / 8);
            cv.fill_a(bar, glass, if dark { 215 } else { 190 });
            let n = 5;
            let sz = bar.h - 6;
            let total = n * sz + (n - 1) * 4;
            for i in 0..n {
                let sx = r.cx() - total / 2 + i * (sz + 4);
                let col = if i == 0 { 0x2D8CFF } else { [0xF5B800, 0x2B2B2B, 0x7C8799, 0xFFFFFF][((i - 1) % 4) as usize] };
                cv.rrect(Rect::new(sx, bar.y + 3, sz, sz), 3, col, 255);
            }
            // window
            let w = Rect::new(r.x + r.w / 6, r.y + r.h / 7, r.w * 11 / 20, r.h * 5 / 9);
            cv.shadow(w, 5, 5, 2, 90);
            cv.rrect(w, 5, if dark { 0x2B2B2B } else { 0xFFFFFF }, 255);
            cv.rrect(Rect::new(w.x, w.y, w.w, 8), 5, if dark { 0x1F1F1F } else { 0xEEEEEE }, 255);
            cv.fill(Rect::new(w.x, w.y + 5, w.w, 4), if dark { 0x1F1F1F } else { 0xEEEEEE });
            for i in 0..3 {
                cv.rrect(Rect::new(w.right() - 8 - i * 7, w.y + 3, 4, 3), 1, if dark { 0x888888 } else { 0x999999 }, 255);
            }
            for i in 0..3 {
                cv.rrect(Rect::new(w.x + 8, w.y + 16 + i * 9, w.w * (7 - i * 2) / 10, 4), 2, if dark { 0x555555 } else { 0xD9D9D9 }, 255);
            }
            cv.rrect(Rect::new(w.x + 8, w.bottom() - 14, 26, 8), 4, accent, 255);
        }
        Exp::Mac => {
            let bar = Rect::new(r.x, r.y, r.w, 9);
            cv.fill_a(bar, glass, if dark { 170 } else { 150 });
            cv.circle(r.x + 8, r.y + 4, 2, if dark { 0xFFFFFF } else { 0x222222 }, 255);
            for i in 0..3 {
                cv.rrect(Rect::new(r.x + 16 + i * 12, r.y + 3, 8, 3), 1, if dark { 0xFFFFFF } else { 0x222222 }, 200);
            }
            let w = Rect::new(r.x + r.w / 7, r.y + r.h / 5, r.w * 5 / 9, r.h / 2);
            cv.shadow(w, 7, 7, 3, 110);
            cv.rrect(w, 7, if dark { 0x2A2A2D } else { 0xFFFFFF }, 255);
            for (i, c) in [0xFF5F57, 0xFEBC2E, 0x28C840].iter().enumerate() {
                cv.circle(w.x + 7 + i as i32 * 7, w.y + 6, 2, *c, 255);
            }
            for i in 0..3 {
                cv.rrect(Rect::new(w.x + 8, w.y + 16 + i * 9, w.w * (7 - i * 2) / 10, 4), 2, if dark { 0x555559 } else { 0xDCDCE0 }, 255);
            }
            let dw = r.w * 3 / 5;
            let dock = Rect::new(r.cx() - dw / 2, r.bottom() - 17, dw, 13);
            cv.rrect(dock, 6, glass, if dark { 150 } else { 120 });
            for i in 0..5 {
                let col = [0x2D8CFF, 0x2B2B2B, 0xFFC83D, 0x9AA3B5, 0xB8C0CE][i as usize];
                cv.rrect(Rect::new(dock.x + 6 + i * (dw - 12) / 5, dock.y + 2, 9, 9), 3, col, 255);
            }
        }
    }
    cv.clip = prev;
    cv.restore_corners(r, rad, &saved);
    cv.rrect_stroke(r, rad, 1, 0x000000, 40);
}

/// A Light/Dark tile for the Appearance choice.
pub fn appearance_tile(cv: &mut Canvas, r: Rect, dark: bool, exp: Exp, accent: Color) {
    exp_preview(cv, r, exp, dark, accent);
}

pub fn logo_small(cv: &mut Canvas, cx: i32, cy: i32, s: i32) {
    icons::logo(cv, Rect::new(cx - s / 2, cy - s / 2, s, s));
}
