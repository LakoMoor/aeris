//! Look & feel: the two "experiences" and their palettes.

use crate::gfx::Color;
use alloc::string::String;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Exp {
    /// Windows 11 style: centred taskbar, Start menu, Fluent surfaces.
    Win,
    /// macOS 27 style: menu bar, Dock, glass windows.
    Mac,
}

impl Exp {
    pub fn name(self) -> &'static str {
        match self {
            Exp::Win => "Windows 11 style",
            Exp::Mac => "macOS 27 style",
        }
    }
}

pub const ACCENTS: [(&str, Color); 6] = [
    ("Blue", 0x0A6CD8),
    ("Violet", 0x7B52E0),
    ("Rose", 0xD93B76),
    ("Orange", 0xE8791A),
    ("Green", 0x1F9D63),
    ("Graphite", 0x66707F),
];

#[derive(Clone)]
pub struct Settings {
    pub exp: Exp,
    pub dark: bool,
    pub accent: usize,
    pub user: String,
}

impl Settings {
    pub fn accent_color(&self) -> Color {
        ACCENTS[self.accent % ACCENTS.len()].1
    }
    pub fn pal(&self) -> Pal {
        Pal::new(self.exp, self.dark, self.accent_color())
    }
}

#[derive(Clone, Copy)]
pub struct Pal {
    pub dark: bool,
    pub accent: Color,
    /// Window / app background.
    pub win: Color,
    /// Content surface (lists, editors).
    pub content: Color,
    pub side: Color,
    pub card: Color,
    pub title: Color,
    pub text: Color,
    pub dim: Color,
    pub line: Color,
}

impl Pal {
    pub fn new(exp: Exp, dark: bool, accent: Color) -> Self {
        match (exp, dark) {
            (Exp::Win, false) => Self {
                dark,
                accent,
                win: 0xF3F3F3,
                content: 0xFFFFFF,
                side: 0xEEEEEE,
                card: 0xFBFBFB,
                title: 0xEEF0F4,
                text: 0x1A1A1A,
                dim: 0x5F5F5F,
                line: 0xE2E2E2,
            },
            (Exp::Win, true) => Self {
                dark,
                accent,
                win: 0x202020,
                content: 0x272727,
                side: 0x1C1C1C,
                card: 0x2D2D2D,
                title: 0x1F2229,
                text: 0xFFFFFF,
                dim: 0xA6A6A6,
                line: 0x3A3A3A,
            },
            (Exp::Mac, false) => Self {
                dark,
                accent,
                win: 0xF5F5F7,
                content: 0xFFFFFF,
                side: 0xE9E9EC,
                card: 0xFFFFFF,
                title: 0xE6E6EA,
                text: 0x1D1D1F,
                dim: 0x7C7C82,
                line: 0xD8D8DC,
            },
            (Exp::Mac, true) => Self {
                dark,
                accent,
                win: 0x1E1E20,
                content: 0x242426,
                side: 0x2A2A2D,
                card: 0x2C2C2F,
                title: 0x2B2B2E,
                text: 0xF5F5F7,
                dim: 0x9A9AA0,
                line: 0x3C3C40,
            },
        }
    }
}
