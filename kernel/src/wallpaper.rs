//! Procedural wallpapers (no image assets needed).

use crate::gfx::{blend, mix, Color};
use crate::theme::Exp;
use alloc::vec;
use alloc::vec::Vec;

struct Blob {
    cx: i32, // per-mille of width
    cy: i32, // per-mille of height
    rx: i32, // per-mille of width
    ry: i32, // per-mille of height
    color: Color,
    strength: i32,
}

const fn b(cx: i32, cy: i32, rx: i32, ry: i32, color: Color, strength: i32) -> Blob {
    Blob { cx, cy, rx, ry, color, strength }
}

pub fn generate(w: i32, h: i32, exp: Exp, dark: bool) -> Vec<u32> {
    let (top, bot, blobs): (Color, Color, Vec<Blob>) = match (exp, dark) {
        (Exp::Win, false) => (
            0xD3E6FB,
            0xEEF4FC,
            vec![
                b(500, 520, 430, 640, 0x1B5FE6, 235),
                b(400, 450, 300, 470, 0x5B3FE0, 210),
                b(620, 620, 300, 420, 0x00B3FF, 210),
                b(280, 760, 260, 360, 0x7A5AF8, 130),
                b(790, 340, 230, 320, 0x3DD6F5, 150),
                b(500, 500, 110, 160, 0xFFFFFF, 150),
            ],
        ),
        (Exp::Win, true) => (
            0x060A18,
            0x0A1230,
            vec![
                b(500, 520, 430, 640, 0x0B3FB8, 220),
                b(400, 450, 300, 470, 0x4A2FD0, 190),
                b(620, 620, 300, 420, 0x0090E0, 190),
                b(280, 760, 260, 360, 0x5A3AE0, 110),
                b(790, 340, 230, 320, 0x1FB6E0, 120),
                b(500, 500, 110, 160, 0x7FB4FF, 120),
            ],
        ),
        (Exp::Mac, false) => (
            0xEFE7FB,
            0xE2F0FF,
            vec![
                b(180, 280, 520, 700, 0x72C6FF, 220),
                b(720, 180, 480, 640, 0xC69BFF, 220),
                b(520, 860, 560, 560, 0xFF9CC8, 200),
                b(900, 760, 420, 520, 0xFFC38A, 190),
                b(480, 480, 200, 260, 0xFFFFFF, 130),
            ],
        ),
        (Exp::Mac, true) => (
            0x0B0919,
            0x120C28,
            vec![
                b(180, 280, 520, 700, 0x2230E8, 200),
                b(720, 180, 480, 640, 0x8A2BDA, 190),
                b(520, 860, 560, 560, 0xD0307E, 170),
                b(900, 760, 420, 520, 0xF0803A, 150),
                b(480, 480, 200, 260, 0x7A6CFF, 70),
            ],
        ),
    };
    let mut buf = vec![0u32; (w * h) as usize];
    for y in 0..h {
        let base = mix(top, bot, (y * 255 / h.max(1)) as u32);
        for x in 0..w {
            let mut c = base;
            for bl in &blobs {
                let rx = (bl.rx * w / 1000).max(1) as i64;
                let ry = (bl.ry * h / 1000).max(1) as i64;
                let dx = (x - bl.cx * w / 1000) as i64;
                let dy = (y - bl.cy * h / 1000) as i64;
                // normalised squared distance, 0..=1024
                let d = (dx * dx * 1024) / (rx * rx) + (dy * dy * 1024) / (ry * ry);
                if d < 1024 {
                    let t = (1024 - d) as i32; // 0..1024
                    let wgt = (t * t) >> 10; // quadratic falloff
                    let a = (wgt * bl.strength) >> 10;
                    c = blend(c, bl.color, (a.min(255)) as u32);
                }
            }
            buf[(y * w + x) as usize] = c;
        }
    }
    buf
}
