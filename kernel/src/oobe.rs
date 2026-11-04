//! Boot splash and first-run Setup, where the user picks the experience.

use crate::gfx::*;
use crate::icons;
use crate::input::{Key, KeyEv};
use crate::theme::{Exp, Pal, Settings, ACCENTS};
use crate::wallpaper;
use crate::widgets::{self, BtnStyle};
use alloc::string::{String, ToString};
use alloc::vec::Vec;

pub fn draw_splash(cv: &mut Canvas, ms: u64) {
    let (w, h) = (cv.w, cv.h);
    cv.clear(0x05060B);
    // soft glow behind the logo
    for i in (1..14).rev() {
        cv.circle(w / 2, h / 2 - 20, 40 + i * 9, 0x3B4CD8, (2 + (14 - i) as u32 / 2).min(14));
    }
    icons::logo(cv, Rect::new(w / 2 - 56, h / 2 - 76, 112, 112));
    cv.text_c(F::Bold, 22, w / 2, h / 2 + 62, "Aeris", 0xF2F4FA, 255);
    let phase = (ms / 110) as i32;
    for i in 0..8 {
        let a = (i - phase).rem_euclid(8);
        let ang = i as f32 * core::f32::consts::TAU / 8.0;
        let (s, c) = (libm::sinf(ang), libm::cosf(ang));
        let alpha = 255 - a * 28;
        cv.circle(w / 2 + (c * 14.0) as i32, h / 2 + 130 + (s * 14.0) as i32, 3, 0xFFFFFF, alpha.max(30) as u32);
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Step {
    Welcome,
    Experience,
    Account,
    Installing,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Hit {
    Next,
    Back,
    Exp(Exp),
    Dark(bool),
    Accent(usize),
}

pub struct Oobe {
    step: Step,
    exp: Exp,
    dark: bool,
    accent: usize,
    name: String,
    t0: u64,
    wall: Vec<u32>,
    hits: Vec<(Rect, Hit)>,
    pub mouse: (i32, i32),
    pub done: bool,
}

const PHASES: [&str; 5] = [
    "Preparing your device",
    "Creating the in-memory file system",
    "Starting the window compositor",
    "Applying your experience",
    "Finishing up",
];
const INSTALL_MS: u64 = 5200;

impl Oobe {
    pub fn new(w: i32, h: i32) -> Self {
        Self {
            step: Step::Welcome,
            exp: Exp::Win,
            dark: false,
            accent: 0,
            name: "Aeris".to_string(),
            t0: 0,
            wall: wallpaper::generate(w, h, Exp::Win, true),
            hits: Vec::new(),
            mouse: (0, 0),
            done: false,
        }
    }

    pub fn settings(&self) -> Settings {
        let user = if self.name.trim().is_empty() { "Aeris".to_string() } else { self.name.trim().to_string() };
        Settings { exp: self.exp, dark: self.dark, accent: self.accent, user }
    }

    pub fn animating(&self) -> bool {
        self.step == Step::Installing || self.step == Step::Account
    }

    fn next(&mut self, ms: u64) {
        self.step = match self.step {
            Step::Welcome => Step::Experience,
            Step::Experience => Step::Account,
            Step::Account => {
                self.t0 = ms;
                Step::Installing
            }
            Step::Installing => Step::Installing,
        };
    }

    fn back(&mut self) {
        self.step = match self.step {
            Step::Experience => Step::Welcome,
            Step::Account => Step::Experience,
            s => s,
        };
    }

    pub fn mouse_down(&mut self, x: i32, y: i32, ms: u64) {
        let hit = self.hits.iter().rev().find(|(r, _)| r.contains(x, y)).map(|(_, h)| *h);
        match hit {
            Some(Hit::Next) => self.next(ms),
            Some(Hit::Back) => self.back(),
            Some(Hit::Exp(e)) => self.exp = e,
            Some(Hit::Dark(d)) => self.dark = d,
            Some(Hit::Accent(a)) => self.accent = a,
            None => {}
        }
    }

    pub fn key(&mut self, k: KeyEv, ms: u64) {
        match k.key {
            Key::Enter => self.next(ms),
            Key::Esc => self.back(),
            Key::Left | Key::Right if self.step == Step::Experience => {
                self.exp = if self.exp == Exp::Win { Exp::Mac } else { Exp::Win };
            }
            Key::Tab if self.step == Step::Account => self.dark = !self.dark,
            Key::Backspace if self.step == Step::Account => {
                self.name.pop();
            }
            Key::Char(c) if self.step == Step::Account && !k.ctrl && (c.is_alphanumeric() || c == ' ' || c == '_') && self.name.chars().count() < 18 => {
                self.name.push(c);
            }
            _ => {}
        }
    }

    pub fn draw(&mut self, cv: &mut Canvas, ms: u64) {
        cv.set_clip(cv.screen());
        let wall = core::mem::take(&mut self.wall);
        cv.buf.copy_from_slice(&wall);
        self.wall = wall;
        let pal = Pal::new(Exp::Mac, true, ACCENTS[self.accent].1);
        let (mx, my) = self.mouse;
        let mut hits: Vec<(Rect, Hit)> = Vec::new();
        let (cw, ch) = (cv.w.min(820), cv.h.min(520));
        let card = Rect::new((cv.w - cw) / 2, (cv.h - ch) / 2, cw, ch);
        cv.shadow(card, 22, 24, 10, 160);
        cv.glass(card, 22, 0x0E1017, 205, 22);
        cv.rrect_stroke(card, 22, 1, 0xFFFFFF, 36);
        let white = 0xF4F6FB;
        let dim = 0xA3AABB;
        let cx = card.cx();
        match self.step {
            Step::Welcome => {
                icons::logo(cv, Rect::new(cx - 52, card.y + 70, 104, 104));
                cv.text_c(F::Bold, 34, cx, card.y + 226, "Welcome to Aeris", white, 255);
                cv.text_c(F::Ui, 16, cx, card.y + 266, "A new operating system, written in Rust.", dim, 255);
                cv.text_c(F::Ui, 13, cx, card.y + 292, "Windows 11 or macOS 27 \u{2014} you decide how it looks and feels.", dim, 255);
            }
            Step::Experience => {
                cv.text_c(F::Bold, 26, cx, card.y + 46, "Choose your experience", white, 255);
                cv.text_c(F::Ui, 14, cx, card.y + 76, "You can switch any time in Settings \u{203A} Personalization.", dim, 255);
                let tw = ((card.w - 120) / 2).min(330);
                let th = tw * 5 / 8 + 4;
                let gap = 32;
                let x0 = cx - tw - gap / 2;
                for (i, (exp, desc)) in [
                    (Exp::Win, "Centred taskbar, Start menu, Fluent surfaces."),
                    (Exp::Mac, "Menu bar, magnifying Dock, frosted glass."),
                ]
                .iter()
                .enumerate()
                {
                    let r = Rect::new(x0 + i as i32 * (tw + gap), card.y + 110, tw, th);
                    widgets::exp_preview(cv, r, *exp, self.dark, pal.accent);
                    if self.exp == *exp {
                        cv.rrect_stroke(r.inset(-4), 14, 3, pal.accent, 255);
                        widgets::check_badge(cv, r.right() - 16, r.y + 16, 11, pal.accent);
                    } else if r.contains(mx, my) {
                        cv.rrect_stroke(r.inset(-4), 14, 2, 0x8890A4, 255);
                    }
                    cv.text(F::Bold, 16, r.x, r.bottom() + 28, exp.name(), white, 255);
                    cv.text(F::Ui, 13, r.x, r.bottom() + 50, desc, dim, 255);
                    hits.push((r, Hit::Exp(*exp)));
                }
            }
            Step::Account => {
                cv.text_c(F::Bold, 26, cx, card.y + 46, "Make it yours", white, 255);
                cv.text_c(F::Ui, 14, cx, card.y + 76, "Your name, appearance and accent colour.", dim, 255);
                let nb = Rect::new(cx - 190, card.y + 112, 380, 44);
                cv.rrect(nb, 10, 0x1B1E27, 255);
                cv.rrect_stroke(nb, 10, 2, pal.accent, 255);
                cv.text(F::Ui, 12, nb.x, nb.y - 12, "Display name", dim, 255);
                let end = cv.text(F::Ui, 17, nb.x + 16, nb.cy(), &self.name, white, 255);
                if (ms / 530) % 2 == 0 {
                    cv.fill(Rect::new(end + 1, nb.cy() - 10, 2, 20), pal.accent);
                }
                let sw = 150;
                let sh = sw * 5 / 8;
                let ax = cx - sw - 16;
                for (i, dark) in [false, true].iter().enumerate() {
                    let r = Rect::new(ax + i as i32 * (sw + 32), card.y + 196, sw, sh);
                    widgets::exp_preview(cv, r, self.exp, *dark, pal.accent);
                    if self.dark == *dark {
                        cv.rrect_stroke(r.inset(-4), 14, 3, pal.accent, 255);
                    } else if r.contains(mx, my) {
                        cv.rrect_stroke(r.inset(-4), 14, 2, 0x8890A4, 255);
                    }
                    cv.text_c(F::Ui, 13, r.cx(), r.bottom() + 20, if *dark { "Dark" } else { "Light" }, white, 255);
                    hits.push((r, Hit::Dark(*dark)));
                }
                let ay = card.y + 196 + sh + 62;
                let total = 6 * 40;
                for (i, (_, col)) in ACCENTS.iter().enumerate() {
                    let x = cx - total / 2 + 20 + i as i32 * 40;
                    cv.circle(x, ay, 13, *col, 255);
                    if self.accent == i {
                        cv.ring(x, ay, 17, 2, white, 255);
                    }
                    hits.push((Rect::new(x - 17, ay - 17, 34, 34), Hit::Accent(i)));
                }
            }
            Step::Installing => {
                let el = ms.saturating_sub(self.t0);
                let p = (el * 100 / INSTALL_MS).min(100) as i32;
                icons::logo(cv, Rect::new(cx - 36, card.y + 90, 72, 72));
                cv.text_c(F::Bold, 24, cx, card.y + 200, "Setting up Aeris", white, 255);
                let phase = ((p as usize) * PHASES.len() / 101).min(PHASES.len() - 1);
                cv.text_c(F::Ui, 14, cx, card.y + 232, PHASES[phase], dim, 255);
                let bar = Rect::new(cx - 220, card.y + 270, 440, 8);
                cv.rrect(bar, 4, 0x2A2E3A, 255);
                cv.rrect(Rect::new(bar.x, bar.y, (bar.w * p / 100).max(8), bar.h), 4, pal.accent, 255);
                cv.text_c(F::Ui, 13, cx, card.y + 300, &alloc::format!("{p}%"), dim, 255);
                if el >= INSTALL_MS {
                    self.done = true;
                }
            }
        }
        // footer buttons
        if self.step != Step::Installing {
            let label = match self.step {
                Step::Welcome => "Get started",
                Step::Account => "Install",
                _ => "Next",
            };
            let nb = Rect::new(card.right() - 150, card.bottom() - 70, 118, 40);
            widgets::button(cv, nb, label, BtnStyle::Primary, &pal, Exp::Mac, nb.contains(mx, my));
            hits.push((nb, Hit::Next));
            if self.step != Step::Welcome {
                let bb = Rect::new(card.x + 32, card.bottom() - 70, 90, 40);
                widgets::button(cv, bb, "Back", BtnStyle::Ghost, &pal, Exp::Mac, bb.contains(mx, my));
                cv.text_c(F::Ui, 1, 0, 0, "", 0, 0);
                hits.push((bb, Hit::Back));
            }
            cv.text_c(F::Ui, 11, cx, card.bottom() - 50, "Press Enter to continue", 0x6C7388, 255);
        }
        self.hits = hits;
    }
}
