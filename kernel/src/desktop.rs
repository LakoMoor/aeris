//! The desktop: window manager plus the two shells (Windows 11 style, macOS 27 style).

use crate::apps::{Action, App, AppKind, Ctx, NotesApp};
use crate::gfx::*;
use crate::icons::{self, Icon};
use crate::input::{Key, KeyEv};
use crate::theme::{Exp, Settings};
use crate::wallpaper;
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use aeris_core::shell::{Flavor, SysInfo, DAYS, MONTHS};
use aeris_core::vfs::Vfs;
use aeris_core::Stamp;

const TASKBAR_H: i32 = 48;
const MENUBAR_H: i32 = 28;

pub struct Win {
    id: u32,
    app: App,
    r: Rect,
    min: bool,
    max: bool,
    restore: Rect,
}

enum Drag {
    Move { id: u32, dx: i32, dy: i32 },
    Resize { id: u32 },
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Item {
    Start,
    App(AppKind),
    Trash,
    Sep,
}

pub struct Desktop {
    pub st: Settings,
    pub w: i32,
    pub h: i32,
    wins: Vec<Win>,
    next_id: u32,
    wall: Vec<u32>,
    pub vfs: Vfs,
    pub sys: SysInfo,
    pub mouse: (i32, i32),
    drag: Option<Drag>,
    start_open: bool,
    power_open: bool,
    apple_open: bool,
    last_title_click: (u32, u64),
    pub power: Option<Action>,
}

fn pinned(exp: Exp) -> Vec<Item> {
    match exp {
        Exp::Win => alloc::vec![
            Item::Start,
            Item::App(AppKind::Files),
            Item::App(AppKind::Terminal(Flavor::PowerShell)),
            Item::App(AppKind::Notes),
            Item::App(AppKind::Settings),
        ],
        Exp::Mac => alloc::vec![
            Item::App(AppKind::Files),
            Item::App(AppKind::Terminal(Flavor::Mac)),
            Item::App(AppKind::Notes),
            Item::App(AppKind::Settings),
            Item::Sep,
            Item::Trash,
        ],
    }
}

fn item_icon(i: Item) -> Icon {
    match i {
        Item::Start => Icon::WinStart,
        Item::Trash => Icon::Trash,
        Item::Sep => Icon::Aeris,
        Item::App(AppKind::Files) => Icon::Files,
        Item::App(AppKind::Settings) => Icon::Settings,
        Item::App(AppKind::Notes) => Icon::Notes,
        Item::App(AppKind::Terminal(Flavor::Cmd)) => Icon::Cmd,
        Item::App(AppKind::Terminal(Flavor::PowerShell)) => Icon::PowerShell,
        Item::App(AppKind::Terminal(Flavor::Mac)) => Icon::Terminal,
    }
}

fn item_name(i: Item) -> &'static str {
    match i {
        Item::Start => "Start",
        Item::Trash => "Trash",
        Item::Sep => "",
        Item::App(AppKind::Files) => "Files",
        Item::App(AppKind::Settings) => "Settings",
        Item::App(AppKind::Notes) => "Notes",
        Item::App(AppKind::Terminal(Flavor::Cmd)) => "Command Prompt",
        Item::App(AppKind::Terminal(Flavor::PowerShell)) => "Terminal",
        Item::App(AppKind::Terminal(Flavor::Mac)) => "Terminal",
    }
}

fn title_h(exp: Exp) -> i32 {
    if exp == Exp::Win {
        34
    } else {
        40
    }
}

fn win_rad(exp: Exp) -> i32 {
    if exp == Exp::Win {
        8
    } else {
        12
    }
}

impl Desktop {
    pub fn new(w: i32, h: i32, st: Settings, vfs: Vfs) -> Self {
        let wall = wallpaper::generate(w, h, st.exp, st.dark);
        let sys = SysInfo { width: w as u32, height: h as u32, ..SysInfo::default() };
        Self {
            st,
            w,
            h,
            wins: Vec::new(),
            next_id: 1,
            wall,
            vfs,
            sys,
            mouse: (w / 2, h / 2),
            drag: None,
            start_open: false,
            power_open: false,
            apple_open: false,
            last_title_click: (0, 0),
            power: None,
        }
    }

    fn work(&self) -> Rect {
        match self.st.exp {
            Exp::Win => Rect::new(0, 0, self.w, self.h - TASKBAR_H),
            Exp::Mac => Rect::new(0, MENUBAR_H, self.w, self.h - MENUBAR_H),
        }
    }

    fn regen_wall(&mut self) {
        self.wall = wallpaper::generate(self.w, self.h, self.st.exp, self.st.dark);
    }

    fn focus_idx(&self) -> Option<usize> {
        self.wins.iter().rposition(|w| !w.min)
    }

    // ---- launching ---------------------------------------------------------

    pub fn launch(&mut self, kind: AppKind, now: Stamp, ms: u64) {
        let ctx = Ctx { pal: self.st.pal(), st: &self.st, mouse: self.mouse, now, ms, sys: &self.sys, vfs: &mut self.vfs };
        let app = App::new(kind, &ctx);
        self.push_window(app, kind);
    }

    fn push_window(&mut self, app: App, kind: AppKind) {
        let work = self.work();
        let (w, h) = match kind {
            AppKind::Terminal(_) => (780, 470),
            AppKind::Files => (880, 540),
            AppKind::Settings => (920, 600),
            AppKind::Notes => (680, 460),
        };
        let w = w.min(work.w - 20);
        let h = h.min(work.h - 20);
        let n = (self.wins.len() as i32 % 8) * 28;
        let x = ((work.w - w) / 2 - 60 + n).max(10);
        let y = (work.y + (work.h - h) / 2 - 30 + n).max(work.y + 4);
        let r = Rect::new(x, y, w, h);
        let id = self.next_id;
        self.next_id += 1;
        self.wins.push(Win { id, app, r, min: false, max: false, restore: r });
        self.start_open = false;
        self.apple_open = false;
    }

    fn open_kind(&mut self, kind: AppKind, now: Stamp, ms: u64) {
        // Taskbar/dock behaviour: focus, minimise or launch.
        let found = self.wins.iter().rposition(|w| w.app.same_kind(kind) && !matches!(kind, AppKind::Terminal(_)) || matches!((&w.app, kind), (App::Term(t), AppKind::Terminal(f)) if t.session.flavor == f));
        match found {
            Some(i) => {
                let top_visible = self.focus_idx() == Some(i);
                if top_visible {
                    self.wins[i].min = true;
                } else {
                    let mut w = self.wins.remove(i);
                    w.min = false;
                    self.wins.push(w);
                }
            }
            None => self.launch(kind, now, ms),
        }
        self.start_open = false;
        self.apple_open = false;
    }

    fn open_notes(&mut self, path: String) {
        let text = self.vfs.read(&path).unwrap_or("").to_string();
        let app = App::Notes(NotesApp::new(Some(path), &text));
        self.push_window(app, AppKind::Notes);
    }

    fn apply(&mut self, a: Action, id: Option<u32>) {
        match a {
            Action::OpenNotes(p) => self.open_notes(p),
            Action::SetExp(e) => {
                if self.st.exp != e {
                    self.st.exp = e;
                    self.regen_wall();
                    let work = self.work();
                    for w in &mut self.wins {
                        if w.max {
                            w.r = work;
                        }
                        w.r.y = w.r.y.max(work.y);
                        w.restore.y = w.restore.y.max(work.y);
                    }
                }
            }
            Action::SetDark(d) => {
                self.st.dark = d;
                self.regen_wall();
            }
            Action::SetAccent(i) => self.st.accent = i,
            Action::Close => {
                if let Some(id) = id {
                    self.wins.retain(|w| w.id != id);
                }
            }
            Action::Shutdown | Action::Reboot => self.power = Some(a),
        }
    }

    // ---- geometry helpers -------------------------------------------------

    fn win_buttons(r: Rect, exp: Exp) -> [Rect; 3] {
        // [close, max, min]
        let th = title_h(exp);
        match exp {
            Exp::Win => [Rect::new(r.right() - 46, r.y, 46, th), Rect::new(r.right() - 92, r.y, 46, th), Rect::new(r.right() - 138, r.y, 46, th)],
            Exp::Mac => [
                Rect::new(r.x + 14, r.y + th / 2 - 8, 16, 16),
                Rect::new(r.x + 54, r.y + th / 2 - 8, 16, 16),
                Rect::new(r.x + 34, r.y + th / 2 - 8, 16, 16),
            ],
        }
    }

    fn content_rect(r: Rect, exp: Exp) -> Rect {
        let th = title_h(exp);
        Rect::new(r.x, r.y + th, r.w, r.h - th)
    }

    fn taskbar_layout(&self) -> Vec<(Rect, Item)> {
        let items = pinned(Exp::Win);
        let total = items.len() as i32 * 48;
        let x0 = (self.w - total) / 2;
        items.into_iter().enumerate().map(|(i, it)| (Rect::new(x0 + i as i32 * 48, self.h - TASKBAR_H + 4, 44, 40), it)).collect()
    }

    /// Returns (panel, items with their (magnified) rects).
    fn dock_layout(&self) -> (Rect, Vec<(Rect, Item)>) {
        let items = pinned(Exp::Mac);
        let base = 54;
        let gap = 10;
        let n = items.len() as i32;
        let seps = items.iter().filter(|i| **i == Item::Sep).count() as i32;
        let base_total = (n - seps) * base + seps * 12 + (n - 1) * gap;
        let bx0 = (self.w - base_total) / 2;
        let in_zone = self.mouse.1 > self.h - 150;
        let mut sizes = Vec::new();
        let mut cx = bx0;
        let mut centers = Vec::new();
        for it in &items {
            let sw = if *it == Item::Sep { 12 } else { base };
            centers.push(cx + sw / 2);
            cx += sw + gap;
        }
        for (i, it) in items.iter().enumerate() {
            if *it == Item::Sep {
                sizes.push(12);
                continue;
            }
            let mut s = base;
            if in_zone {
                let d = (centers[i] - self.mouse.0).abs();
                if d < 130 {
                    let t = 130 - d;
                    s += 40 * (t * t) / (130 * 130);
                }
            }
            sizes.push(s);
        }
        let total: i32 = sizes.iter().sum::<i32>() + (n - 1) * gap;
        let panel = Rect::new((self.w - total) / 2 - 14, self.h - 82, total + 28, 70);
        let mut x = panel.x + 14;
        let mut out = Vec::new();
        for (i, it) in items.into_iter().enumerate() {
            let s = sizes[i];
            out.push((Rect::new(x, panel.bottom() - 8 - s, s, s), it));
            x += s + gap;
        }
        (panel, out)
    }

    fn start_rect(&self) -> Rect {
        let w = 640.min(self.w - 20);
        let h = 560.min(self.h - TASKBAR_H - 24);
        Rect::new((self.w - w) / 2, self.h - TASKBAR_H - 12 - h, w, h)
    }

    fn start_tiles(&self) -> Vec<(Rect, Item, &'static str)> {
        let s = self.start_rect();
        let tiles: [(Item, &str); 6] = [
            (Item::App(AppKind::Files), "Files"),
            (Item::App(AppKind::Terminal(Flavor::PowerShell)), "PowerShell"),
            (Item::App(AppKind::Terminal(Flavor::Cmd)), "Command Prompt"),
            (Item::App(AppKind::Settings), "Settings"),
            (Item::App(AppKind::Notes), "Notes"),
            (Item::App(AppKind::Terminal(Flavor::Mac)), "Terminal (zsh)"),
        ];
        let cw = (s.w - 64) / 6;
        tiles.iter().enumerate().map(|(i, (it, n))| (Rect::new(s.x + 32 + i as i32 * cw, s.y + 118, cw, 86), *it, *n)).collect()
    }

    fn apple_items() -> [&'static str; 4] {
        ["About This Aeris", "System Settings\u{2026}", "Restart\u{2026}", "Shut Down\u{2026}"]
    }

    fn apple_rect(&self) -> Rect {
        Rect::new(8, MENUBAR_H + 2, 230, 14 + 4 * 28 + 6)
    }

    fn power_menu_rect(&self) -> Rect {
        let s = self.start_rect();
        Rect::new(s.right() - 170, s.bottom() - 56 - 76, 150, 72)
    }

    // ---- drawing -----------------------------------------------------------

    pub fn draw(&mut self, cv: &mut Canvas, now: Stamp, ms: u64) {
        let exp = self.st.exp;
        let pal = self.st.pal();
        cv.set_clip(cv.screen());
        let wall = core::mem::take(&mut self.wall);
        cv.buf.copy_from_slice(&wall);
        self.wall = wall;
        let top_idx = self.focus_idx();
        for i in 0..self.wins.len() {
            if self.wins[i].min {
                continue;
            }
            let focused = Some(i) == top_idx;
            self.draw_window(cv, i, focused, now, ms);
        }
        match exp {
            Exp::Win => self.draw_taskbar(cv, now),
            Exp::Mac => {
                self.draw_menubar(cv, now);
                self.draw_dock(cv);
            }
        }
        if self.start_open && exp == Exp::Win {
            self.draw_start(cv);
        }
        if self.apple_open && exp == Exp::Mac {
            self.draw_apple(cv);
        }
        let _ = pal;
    }

    fn draw_window(&mut self, cv: &mut Canvas, i: usize, focused: bool, now: Stamp, ms: u64) {
        let exp = self.st.exp;
        let pal = self.st.pal();
        let r = self.wins[i].r;
        let rad = win_rad(exp);
        let th = title_h(exp);
        cv.shadow(r, rad, if focused { 18 } else { 9 }, 6, if focused { 130 } else { 70 });
        let saved = cv.grab_bottom(r, rad);
        cv.rrect(r, rad, pal.title, 255);
        // title bar
        let (mx, my) = self.mouse;
        let btns = Self::win_buttons(r, exp);
        match exp {
            Exp::Win => {
                let icon = self.wins[i].app.icon();
                icons::app(cv, icon, Rect::new(r.x + 10, r.y + 9, 16, 16), exp);
                let t = self.wins[i].app.title();
                cv.text(F::Ui, 12, r.x + 34, r.y + th / 2, &t, if focused { pal.text } else { pal.dim }, 255);
                for (k, b) in btns.iter().enumerate() {
                    let hov = b.contains(mx, my);
                    if hov {
                        if k == 0 {
                            cv.fill(*b, 0xC42B1C);
                            // keep the window corner round
                            cv.rrect(Rect::new(b.right() - 12, b.y, 12, 12), 0, 0xC42B1C, 0);
                        } else {
                            cv.fill_a(*b, pal.text, 20);
                        }
                    }
                    let (cx, cy) = (b.cx(), b.cy());
                    let c = if k == 0 && hov { 0xFFFFFF } else { pal.text };
                    match k {
                        0 => {
                            cv.line(cx - 5, cy - 5, cx + 5, cy + 5, 1, c, 255);
                            cv.line(cx - 5, cy + 5, cx + 5, cy - 5, 1, c, 255);
                        }
                        1 => {
                            if self.wins[i].max {
                                cv.rrect_stroke(Rect::new(cx - 4, cy - 2, 8, 8), 1, 1, c, 255);
                                cv.fill(Rect::new(cx - 2, cy - 5, 7, 1), c);
                                cv.fill(Rect::new(cx + 5, cy - 5, 1, 7), c);
                            } else {
                                cv.rrect_stroke(Rect::new(cx - 5, cy - 5, 10, 10), 1, 1, c, 255);
                            }
                        }
                        _ => cv.fill(Rect::new(cx - 5, cy, 10, 1), c),
                    }
                }
            }
            Exp::Mac => {
                let cols = [0xFF5F57, 0x28C840, 0xFEBC2E];
                let any_hov = btns.iter().any(|b| b.inflate(4).contains(mx, my));
                for (k, b) in btns.iter().enumerate() {
                    let col = if focused { cols[k] } else if pal.dark { 0x4A4A4E } else { 0xCFCFD3 };
                    cv.circle(b.cx(), b.cy(), 6, col, 255);
                    cv.ring(b.cx(), b.cy(), 6, 1, 0x000000, 40);
                    if any_hov && focused {
                        let g = 0x4D0000;
                        match k {
                            0 => {
                                cv.line(b.cx() - 2, b.cy() - 2, b.cx() + 2, b.cy() + 2, 1, g, 200);
                                cv.line(b.cx() - 2, b.cy() + 2, b.cx() + 2, b.cy() - 2, 1, g, 200);
                            }
                            1 => cv.line(b.cx() - 3, b.cy(), b.cx() + 3, b.cy(), 1, 0x995700, 220),
                            _ => {
                                cv.line(b.cx() - 3, b.cy(), b.cx() + 3, b.cy(), 1, 0x006500, 220);
                                cv.line(b.cx(), b.cy() - 3, b.cx(), b.cy() + 3, 1, 0x006500, 220);
                            }
                        }
                    }
                }
                let t = self.wins[i].app.title();
                cv.text_c(F::Bold, 13, r.cx(), r.y + th / 2, &t, if focused { pal.text } else { pal.dim }, 255);
            }
        }
        cv.fill(Rect::new(r.x, r.y + th - 1, r.w, 1), pal.line);
        // content
        let content = Self::content_rect(r, exp);
        let prev = cv.set_clip(content);
        let id_focus = focused;
        let _ = id_focus;
        {
            let mut ctx = Ctx { pal, st: &self.st, mouse: (mx, my), now, ms, sys: &self.sys, vfs: &mut self.vfs };
            self.wins[i].app.draw(cv, content, &mut ctx);
        }
        cv.clip = prev;
        cv.round_bottom(r, rad, &saved);
        let edge = if exp == Exp::Win { (pal.line, 255) } else if pal.dark { (0xFFFFFF, 40) } else { (0x000000, 50) };
        cv.rrect_stroke(r, rad, 1, edge.0, edge.1);
        // resize grip hint
        if !self.wins[i].max && Rect::new(r.right() - 14, r.bottom() - 14, 14, 14).contains(mx, my) {
            cv.line(r.right() - 10, r.bottom() - 4, r.right() - 4, r.bottom() - 10, 1, pal.dim, 255);
        }
    }

    fn glass_tint(&self) -> (Color, u32) {
        if self.st.dark {
            (0x15171C, 190)
        } else {
            (0xF4F6FA, 180)
        }
    }

    fn running(&self, kind: AppKind) -> (bool, bool) {
        let top = self.focus_idx();
        let mut any = false;
        let mut foc = false;
        for (i, w) in self.wins.iter().enumerate() {
            let m = match (&w.app, kind) {
                (App::Term(t), AppKind::Terminal(f)) => t.session.flavor == f,
                _ => w.app.same_kind(kind) && !matches!(kind, AppKind::Terminal(_)),
            };
            if m {
                any = true;
                if Some(i) == top {
                    foc = true;
                }
            }
        }
        (any, foc)
    }

    fn draw_taskbar(&mut self, cv: &mut Canvas, now: Stamp) {
        let pal = self.st.pal();
        let bar = Rect::new(0, self.h - TASKBAR_H, self.w, TASKBAR_H);
        let (tint, a) = self.glass_tint();
        cv.glass(bar, 0, tint, a, 14);
        cv.fill(Rect::new(0, bar.y, self.w, 1), if pal.dark { 0x3A3D45 } else { 0xD5D9E0 });
        let (mx, my) = self.mouse;
        for (r, it) in self.taskbar_layout() {
            let hov = r.contains(mx, my);
            let active = self.start_open && it == Item::Start;
            if hov || active {
                cv.rrect(r, 6, if pal.dark { 0xFFFFFF } else { 0x000000 }, if active { 18 } else { 12 });
            }
            let sz = if hov { 28 } else { 27 };
            icons::app(cv, item_icon(it), Rect::new(r.cx() - sz / 2, r.cy() - sz / 2 - 1, sz, sz), Exp::Win);
            if let Item::App(k) = it {
                let (any, foc) = self.running(k);
                if any {
                    let w = if foc { 16 } else { 6 };
                    cv.rrect(Rect::new(r.cx() - w / 2, bar.bottom() - 6, w, 3), 1, if foc { pal.accent } else { pal.dim }, 255);
                }
            }
        }
        // tray
        let tc = pal.text;
        let right = self.w - 14;
        let h12 = if now.hour % 12 == 0 { 12 } else { now.hour % 12 };
        let ap = if now.hour >= 12 { "PM" } else { "AM" };
        cv.text_r(F::Ui, 12, right, bar.y + 17, &format!("{}:{:02} {}", h12, now.minute, ap), tc, 255);
        cv.text_r(F::Ui, 12, right, bar.y + 33, &format!("{}/{}/{}", now.month, now.day, now.year), tc, 255);
        let tx = right - 96;
        icons::battery(cv, tx - 6, bar.cy(), 18, tc, 82);
        icons::volume(cv, tx - 38, bar.cy(), 18, tc);
        icons::wifi(cv, tx - 66, bar.cy() - 1, 16, tc);
        icons::chevron(cv, tx - 96, bar.cy(), 10, tc, 0);
    }

    fn draw_start(&mut self, cv: &mut Canvas) {
        let pal = self.st.pal();
        let s = self.start_rect();
        let (mx, my) = self.mouse;
        cv.shadow(s, 8, 20, 8, 150);
        let (tint, _) = self.glass_tint();
        cv.glass(s, 8, tint, if pal.dark { 215 } else { 205 }, 16);
        cv.rrect_stroke(s, 8, 1, if pal.dark { 0x4A4D55 } else { 0xD0D4DC }, 255);
        // search box
        let sb = Rect::new(s.x + 32, s.y + 28, s.w - 64, 36);
        cv.rrect(sb, 18, if pal.dark { 0x2E3037 } else { 0xFFFFFF }, 240);
        cv.rrect_stroke(sb, 18, 1, pal.line, 255);
        icons::search(cv, sb.x + 20, sb.cy(), 14, pal.dim);
        cv.text(F::Ui, 13, sb.x + 40, sb.cy(), "Search for apps, settings, and documents", pal.dim, 255);
        cv.text(F::Bold, 14, s.x + 32, s.y + 94, "Pinned", pal.text, 255);
        for (r, it, name) in self.start_tiles() {
            if r.contains(mx, my) {
                cv.rrect(r.inset(2), 6, pal.text, 16);
            }
            icons::app(cv, item_icon(it), Rect::new(r.cx() - 16, r.y + 10, 32, 32), Exp::Win);
            cv.text_c(F::Ui, 12, r.cx(), r.y + 62, name, pal.text, 255);
        }
        let ry = s.y + 232;
        cv.text(F::Bold, 14, s.x + 32, ry, "Recommended", pal.text, 255);
        let files: Vec<String> = self.vfs.list("/Users/aeris/Documents").unwrap_or_default().into_iter().filter(|e| !e.is_dir).map(|e| e.name).collect();
        for (i, f) in files.iter().take(6).enumerate() {
            let col = (i % 2) as i32;
            let row = (i / 2) as i32;
            let r = Rect::new(s.x + 32 + col * ((s.w - 64) / 2), ry + 22 + row * 52, (s.w - 64) / 2 - 8, 46);
            if r.contains(mx, my) {
                cv.rrect(r, 6, pal.text, 16);
            }
            icons::file_small(cv, Rect::new(r.x + 8, r.y + 9, 28, 28), pal.accent, pal.dark);
            cv.text(F::Ui, 13, r.x + 46, r.y + 17, f, pal.text, 255);
            cv.text(F::Ui, 11, r.x + 46, r.y + 33, "Documents", pal.dim, 255);
        }
        // footer
        let fb = Rect::new(s.x, s.bottom() - 56, s.w, 56);
        cv.fill_a(fb, if pal.dark { 0x000000 } else { 0xE6E9EF }, if pal.dark { 60 } else { 140 });
        cv.circle(s.x + 52, fb.cy(), 14, pal.accent, 255);
        let init: String = self.st.user.chars().next().unwrap_or('A').to_uppercase().collect();
        cv.text_c(F::Bold, 13, s.x + 52, fb.cy(), &init, 0xFFFFFF, 255);
        cv.text(F::Ui, 13, s.x + 74, fb.cy(), &self.st.user, pal.text, 255);
        let pb = Rect::new(s.right() - 56, fb.y + 10, 36, 36);
        if pb.contains(mx, my) || self.power_open {
            cv.rrect(pb, 6, pal.text, 18);
        }
        icons::power(cv, pb.cx(), pb.cy(), 16, pal.text);
        if self.power_open {
            let p = self.power_menu_rect();
            cv.shadow(p, 8, 10, 4, 120);
            cv.rrect(p, 8, if pal.dark { 0x2B2D33 } else { 0xFFFFFF }, 255);
            cv.rrect_stroke(p, 8, 1, pal.line, 255);
            for (i, t) in ["Restart", "Shut down"].iter().enumerate() {
                let r = Rect::new(p.x + 6, p.y + 6 + i as i32 * 32, p.w - 12, 30);
                if r.contains(mx, my) {
                    cv.rrect(r, 5, pal.text, 18);
                }
                cv.text(F::Ui, 13, r.x + 12, r.cy(), t, pal.text, 255);
            }
        }
    }

    fn draw_menubar(&mut self, cv: &mut Canvas, now: Stamp) {
        let pal = self.st.pal();
        let bar = Rect::new(0, 0, self.w, MENUBAR_H);
        let (tint, a) = self.glass_tint();
        cv.glass(bar, 0, tint, a.saturating_sub(30), 12);
        let tc = pal.text;
        let (mx, my) = self.mouse;
        let ar = Rect::new(8, 2, 30, 24);
        if self.apple_open || ar.contains(mx, my) {
            cv.rrect(ar, 5, tc, 25);
        }
        icons::logo(cv, Rect::new(14, 6, 16, 16));
        let name = match self.focus_idx().map(|i| &self.wins[i].app) {
            Some(App::Term(_)) => "Terminal",
            Some(App::Files(_)) => "Files",
            Some(App::Settings(_)) => "System Settings",
            Some(App::Notes(_)) => "Notes",
            None => "Aeris",
        };
        let mut x = 52;
        x = cv.text(F::Bold, 13, x, 14, name, tc, 255) + 20;
        for m in ["File", "Edit", "View", "Window", "Help"] {
            x = cv.text(F::Ui, 13, x, 14, m, tc, 255) + 20;
        }
        let right = self.w - 14;
        let h12 = if now.hour % 12 == 0 { 12 } else { now.hour % 12 };
        let ap = if now.hour >= 12 { "PM" } else { "AM" };
        let s = format!("{} {} {}   {}:{:02} {}", &DAYS[now.weekday() % 7][..3], &MONTHS[(now.month.clamp(1, 12) - 1) as usize][..3], now.day, h12, now.minute, ap);
        cv.text_r(F::Ui, 13, right, 14, &s, tc, 255);
        let tw = cv.text_width(F::Ui, 13, &s);
        let ix = right - tw - 28;
        icons::search(cv, ix, 14, 14, tc);
        icons::wifi(cv, ix - 30, 13, 14, tc);
        icons::battery(cv, ix - 62, 14, 20, tc, 82);
    }

    fn draw_dock(&mut self, cv: &mut Canvas) {
        let pal = self.st.pal();
        let (panel, items) = self.dock_layout();
        let (tint, a) = self.glass_tint();
        cv.shadow(panel, 20, 12, 5, 90);
        cv.glass(panel, 20, tint, a.saturating_sub(40), 14);
        cv.rrect_stroke(panel, 20, 1, if pal.dark { 0xFFFFFF } else { 0xFFFFFF }, if pal.dark { 40 } else { 160 });
        let (mx, my) = self.mouse;
        let mut label: Option<(Rect, &str)> = None;
        for (r, it) in &items {
            if *it == Item::Sep {
                cv.fill(Rect::new(r.cx(), panel.y + 14, 1, panel.h - 28), pal.dim);
                continue;
            }
            icons::app(cv, item_icon(*it), *r, Exp::Mac);
            if let Item::App(k) = it {
                if self.running(*k).0 {
                    cv.circle(r.cx(), panel.bottom() - 5, 2, pal.text, 220);
                }
            }
            if r.inflate(5).contains(mx, my) {
                label = Some((*r, item_name(*it)));
            }
        }
        if let Some((r, n)) = label {
            let w = cv.text_width(F::Ui, 13, n) + 24;
            let lr = Rect::new(r.cx() - w / 2, r.y - 38, w, 26);
            cv.glass(lr, 8, 0x202226, 190, 8);
            cv.text_c(F::Ui, 13, lr.cx(), lr.cy(), n, 0xFFFFFF, 255);
        }
    }

    fn draw_apple(&mut self, cv: &mut Canvas) {
        let pal = self.st.pal();
        let r = self.apple_rect();
        let (mx, my) = self.mouse;
        let (tint, a) = self.glass_tint();
        cv.shadow(r, 10, 14, 6, 120);
        cv.glass(r, 10, tint, a.saturating_add(20).min(240), 14);
        cv.rrect_stroke(r, 10, 1, if pal.dark { 0xFFFFFF } else { 0x000000 }, if pal.dark { 40 } else { 40 });
        for (i, t) in Self::apple_items().iter().enumerate() {
            let ir = Rect::new(r.x + 6, r.y + 7 + i as i32 * 28, r.w - 12, 26);
            let hov = ir.contains(mx, my);
            if hov {
                cv.rrect(ir, 6, pal.accent, 255);
            }
            cv.text(F::Ui, 13, ir.x + 12, ir.cy(), t, if hov { 0xFFFFFF } else { pal.text }, 255);
        }
    }

    // ---- input ---------------------------------------------------------------

    /// A cheap fingerprint of "what is hovered", so we redraw only when it changes.
    pub fn hover_key(&self) -> u64 {
        let (x, y) = self.mouse;
        let mut k: u64 = 0;
        if let Some(Drag::Move { .. } | Drag::Resize { .. }) = self.drag {
            return 1_000_000 + (x as u64) * 4000 + y as u64;
        }
        if self.st.exp == Exp::Mac && y > self.h - 150 {
            k = 1 + (x / 3) as u64 * 1000 + (y / 20) as u64;
        } else if self.st.exp == Exp::Win && y > self.h - TASKBAR_H {
            k = 2 + (x / 4) as u64;
        } else if self.start_open || self.apple_open || y < MENUBAR_H {
            k = 3 + (x / 6) as u64 * 1000 + (y / 6) as u64;
        } else if let Some(w) = self.wins.iter().rev().find(|w| !w.min && w.r.contains(x, y)) {
            if !matches!(w.app, App::Term(_)) || y < w.r.y + title_h(self.st.exp) {
                k = 4 + (x / 6) as u64 * 1000 + (y / 6) as u64;
            }
        }
        k
    }

    pub fn needs_blink(&self) -> bool {
        self.focus_idx().map_or(false, |i| self.wins[i].app.animated())
    }

    pub fn mouse_move(&mut self, x: i32, y: i32) {
        self.mouse = (x, y);
        let work = self.work();
        match self.drag {
            Some(Drag::Move { id, dx, dy }) => {
                if let Some(w) = self.wins.iter_mut().find(|w| w.id == id) {
                    w.r.x = (x - dx).clamp(-w.r.w + 80, self.w - 80);
                    w.r.y = (y - dy).clamp(work.y, self.h - 40);
                }
            }
            Some(Drag::Resize { id }) => {
                if let Some(w) = self.wins.iter_mut().find(|w| w.id == id) {
                    w.r.w = (x - w.r.x).max(380);
                    w.r.h = (y - w.r.y).max(260);
                }
            }
            None => {}
        }
    }

    pub fn mouse_up(&mut self) {
        self.drag = None;
    }

    pub fn scroll(&mut self, d: i32) {
        let (x, y) = self.mouse;
        if let Some(w) = self.wins.iter_mut().rev().find(|w| !w.min && w.r.contains(x, y)) {
            w.app.scroll(d);
        }
    }

    pub fn mouse_down(&mut self, x: i32, y: i32, now: Stamp, ms: u64) {
        self.mouse = (x, y);
        let exp = self.st.exp;
        // ---- overlays
        if self.start_open && exp == Exp::Win {
            let s = self.start_rect();
            let start_btn = self.taskbar_layout()[0].0;
            if self.power_open {
                let p = self.power_menu_rect();
                if p.contains(x, y) {
                    let idx = (y - p.y - 6) / 32;
                    self.power = Some(if idx == 0 { Action::Reboot } else { Action::Shutdown });
                    return;
                }
                self.power_open = false;
            }
            if s.contains(x, y) {
                for (r, it, _) in self.start_tiles() {
                    if r.contains(x, y) {
                        self.start_open = false;
                        if let Item::App(k) = it {
                            self.launch(k, now, ms);
                        }
                        return;
                    }
                }
                let files: Vec<String> = self.vfs.list("/Users/aeris/Documents").unwrap_or_default().into_iter().filter(|e| !e.is_dir).map(|e| e.name).collect();
                let ry = s.y + 232;
                for (i, f) in files.iter().take(6).enumerate() {
                    let r = Rect::new(s.x + 32 + (i as i32 % 2) * ((s.w - 64) / 2), ry + 22 + (i as i32 / 2) * 52, (s.w - 64) / 2 - 8, 46);
                    if r.contains(x, y) {
                        self.start_open = false;
                        self.open_notes(format!("/Users/aeris/Documents/{f}"));
                        return;
                    }
                }
                let pb = Rect::new(s.right() - 56, s.bottom() - 46, 36, 36);
                if pb.contains(x, y) {
                    self.power_open = !self.power_open;
                }
                return;
            }
            self.start_open = false;
            if start_btn.contains(x, y) {
                return;
            }
        }
        if self.apple_open && exp == Exp::Mac {
            let r = self.apple_rect();
            self.apple_open = false;
            if r.contains(x, y) {
                let idx = ((y - r.y - 7) / 28).clamp(0, 3);
                match idx {
                    0 => {
                        self.launch(AppKind::Settings, now, ms);
                        if let Some(Win { app: App::Settings(s), .. }) = self.wins.last_mut() {
                            s.open_about();
                        }
                    }
                    1 => self.open_kind(AppKind::Settings, now, ms),
                    2 => self.power = Some(Action::Reboot),
                    _ => self.power = Some(Action::Shutdown),
                }
                return;
            }
            if y < MENUBAR_H && x < 40 {
                return;
            }
        }
        // ---- shell chrome
        match exp {
            Exp::Win => {
                if y >= self.h - TASKBAR_H {
                    for (r, it) in self.taskbar_layout() {
                        if r.contains(x, y) {
                            match it {
                                Item::Start => self.start_open = !self.start_open,
                                Item::App(k) => self.open_kind(k, now, ms),
                                _ => {}
                            }
                        }
                    }
                    return;
                }
            }
            Exp::Mac => {
                if y < MENUBAR_H {
                    if x < 40 {
                        self.apple_open = true;
                    }
                    return;
                }
                let (panel, items) = self.dock_layout();
                if panel.inflate(6).contains(x, y) || items.iter().any(|(r, _)| r.contains(x, y)) {
                    for (r, it) in items {
                        if r.inflate(5).contains(x, y) {
                            if let Item::App(k) = it {
                                self.open_kind(k, now, ms);
                            }
                        }
                    }
                    return;
                }
            }
        }
        // ---- windows
        for i in (0..self.wins.len()).rev() {
            if self.wins[i].min || !self.wins[i].r.contains(x, y) {
                continue;
            }
            let mut w = self.wins.remove(i);
            let r = w.r;
            let id = w.id;
            let btns = Self::win_buttons(r, exp);
            let th = title_h(exp);
            let mut action: Option<Action> = None;
            let mut keep = true;
            if y < r.y + th {
                let hit = if exp == Exp::Mac { btns.iter().position(|b| b.inflate(3).contains(x, y)) } else { btns.iter().position(|b| b.contains(x, y)) };
                match hit {
                    Some(0) => keep = false,
                    Some(1) => self.toggle_max(&mut w),
                    Some(2) => w.min = true,
                    _ => {
                        let dbl = self.last_title_click.0 == id && ms.saturating_sub(self.last_title_click.1) < 400;
                        self.last_title_click = (id, ms);
                        if dbl {
                            self.toggle_max(&mut w);
                        } else if !w.max {
                            self.drag = Some(Drag::Move { id, dx: x - r.x, dy: y - r.y });
                        }
                    }
                }
            } else if !w.max && Rect::new(r.right() - 16, r.bottom() - 16, 16, 16).contains(x, y) {
                self.drag = Some(Drag::Resize { id });
            } else {
                let mut ctx = Ctx { pal: self.st.pal(), st: &self.st, mouse: (x, y), now, ms, sys: &self.sys, vfs: &mut self.vfs };
                action = w.app.mouse_down(x, y, &mut ctx);
            }
            if keep {
                self.wins.push(w);
            }
            if let Some(a) = action {
                self.apply(a, Some(id));
            }
            return;
        }
    }

    fn toggle_max(&self, w: &mut Win) {
        if w.max {
            w.r = w.restore;
            w.max = false;
        } else {
            w.restore = w.r;
            w.r = self.work();
            w.max = true;
        }
    }

    pub fn key(&mut self, k: KeyEv, now: Stamp, ms: u64) {
        if k.key == Key::Meta {
            if self.st.exp == Exp::Win {
                self.start_open = !self.start_open;
                self.power_open = false;
            } else {
                self.apple_open = !self.apple_open;
            }
            return;
        }
        if k.key == Key::Esc && (self.start_open || self.apple_open) {
            self.start_open = false;
            self.apple_open = false;
            self.power_open = false;
            return;
        }
        if k.alt {
            match k.key {
                Key::Tab => {
                    let vis: Vec<usize> = (0..self.wins.len()).filter(|i| !self.wins[*i].min).collect();
                    if vis.len() > 1 {
                        let w = self.wins.remove(vis[0]);
                        self.wins.insert(vis[0], w);
                        // bring the second-from-top to the front
                        let n = self.wins.len();
                        let idx = vis[vis.len() - 2];
                        let w = self.wins.remove(idx);
                        let _ = n;
                        self.wins.push(w);
                    }
                    return;
                }
                Key::Char('w') | Key::Char('q') => {
                    if let Some(i) = self.focus_idx() {
                        self.wins.remove(i);
                    }
                    return;
                }
                _ => {}
            }
        }
        if let Some(i) = self.focus_idx() {
            let id = self.wins[i].id;
            let action = {
                let mut ctx = Ctx { pal: self.st.pal(), st: &self.st, mouse: self.mouse, now, ms, sys: &self.sys, vfs: &mut self.vfs };
                self.wins[i].app.key(k, &mut ctx)
            };
            if let Some(a) = action {
                self.apply(a, Some(id));
            }
        }
    }

    pub fn tick_sys(&mut self, uptime_s: u64) {
        self.sys.uptime_s = uptime_s;
        let files = self.vfs.total_files() as u32;
        self.sys.mem_used_mb = 64 + self.wins.len() as u32 * 12 + files;
        self.sys.mem_total_mb = 1024;
    }
}

impl Rect {
    pub fn inflate(&self, d: i32) -> Rect {
        self.inset(-d)
    }
}
