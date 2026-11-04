//! Built-in applications: Terminal, Files, Settings and Notes.

use crate::gfx::*;
use crate::icons;
use crate::input::{Key, KeyEv};
use crate::theme::{Exp, Pal, Settings, ACCENTS};
use crate::widgets::{self, BtnStyle};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use aeris_core::shell::{Flavor, Session, SysInfo, MONTHS};
use aeris_core::vfs::{Entry, Vfs};
use aeris_core::Stamp;

pub struct Ctx<'a> {
    pub pal: Pal,
    pub st: &'a Settings,
    pub mouse: (i32, i32),
    pub now: Stamp,
    pub ms: u64,
    pub sys: &'a SysInfo,
    pub vfs: &'a mut Vfs,
}

#[derive(Clone, Debug)]
pub enum Action {
    OpenNotes(String),
    SetExp(Exp),
    SetDark(bool),
    SetAccent(usize),
    Close,
    Shutdown,
    Reboot,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AppKind {
    Terminal(Flavor),
    Files,
    Settings,
    Notes,
}

pub enum App {
    Term(Terminal),
    Files(FilesApp),
    Settings(SettingsApp),
    Notes(NotesApp),
}

impl App {
    pub fn new(kind: AppKind, ctx: &Ctx) -> App {
        match kind {
            AppKind::Terminal(f) => App::Term(Terminal::new(f, ctx.now, ctx.st.exp == Exp::Mac)),
            AppKind::Files => App::Files(FilesApp::new()),
            AppKind::Settings => App::Settings(SettingsApp::new()),
            AppKind::Notes => App::Notes(NotesApp::new(None, "")),
        }
    }

    pub fn same_kind(&self, k: AppKind) -> bool {
        match (self, k) {
            (App::Term(_), AppKind::Terminal(_)) => true,
            (App::Files(_), AppKind::Files) => true,
            (App::Settings(_), AppKind::Settings) => true,
            (App::Notes(_), AppKind::Notes) => true,
            _ => false,
        }
    }

    pub fn title(&self) -> String {
        match self {
            App::Term(t) => String::from(match t.session.flavor {
                Flavor::Cmd => "Command Prompt",
                Flavor::PowerShell => "Windows PowerShell",
                Flavor::Mac => "Terminal \u{2014} zsh",
            }),
            App::Files(f) => {
                let n = Vfs::basename(&f.path);
                if f.path == "/" {
                    String::from("Files")
                } else {
                    n.to_string()
                }
            }
            App::Settings(_) => String::from("Settings"),
            App::Notes(n) => format!("{}{}", n.path.as_deref().map(Vfs::basename).unwrap_or("Untitled"), if n.dirty { " \u{2022}" } else { "" }),
        }
    }

    pub fn icon(&self) -> icons::Icon {
        match self {
            App::Term(t) => match t.session.flavor {
                Flavor::Cmd => icons::Icon::Cmd,
                Flavor::PowerShell => icons::Icon::PowerShell,
                Flavor::Mac => icons::Icon::Terminal,
            },
            App::Files(_) => icons::Icon::Files,
            App::Settings(_) => icons::Icon::Settings,
            App::Notes(_) => icons::Icon::Notes,
        }
    }

    pub fn draw(&mut self, cv: &mut Canvas, r: Rect, ctx: &mut Ctx) {
        match self {
            App::Term(a) => a.draw(cv, r, ctx),
            App::Files(a) => a.draw(cv, r, ctx),
            App::Settings(a) => a.draw(cv, r, ctx),
            App::Notes(a) => a.draw(cv, r, ctx),
        }
    }

    pub fn mouse_down(&mut self, x: i32, y: i32, ctx: &mut Ctx) -> Option<Action> {
        match self {
            App::Term(_) => None,
            App::Files(a) => a.mouse_down(x, y, ctx),
            App::Settings(a) => a.mouse_down(x, y, ctx),
            App::Notes(a) => a.mouse_down(x, y, ctx),
        }
    }

    pub fn key(&mut self, k: KeyEv, ctx: &mut Ctx) -> Option<Action> {
        match self {
            App::Term(a) => a.key(k, ctx),
            App::Files(a) => a.key(k, ctx),
            App::Settings(_) => None,
            App::Notes(a) => a.key(k, ctx),
        }
    }

    pub fn scroll(&mut self, d: i32) {
        match self {
            App::Term(a) => {
                if d > 0 {
                    a.scroll = a.scroll.saturating_add(3 * d as usize);
                } else {
                    a.scroll = a.scroll.saturating_sub(3 * (-d) as usize);
                }
            }
            App::Files(a) => a.scroll = (a.scroll - d * 40).max(0),
            App::Notes(a) => {
                a.top = (a.top as i32 - d * 3).max(0) as usize;
            }
            _ => {}
        }
    }

    /// Whether this app wants the blinking-cursor redraw.
    pub fn animated(&self) -> bool {
        matches!(self, App::Term(_) | App::Notes(_))
    }
}

// =============================================================================
// Terminal
// =============================================================================

pub struct Terminal {
    pub session: Session,
    lines: Vec<(String, u8)>,
    input: String,
    cur: usize,
    pub scroll: usize,
    hist: Option<usize>,
}

impl Terminal {
    pub fn new(flavor: Flavor, now: Stamp, mac: bool) -> Self {
        let session = Session::new(flavor, now, mac);
        let mut t = Self { session, lines: Vec::new(), input: String::new(), cur: 0, scroll: 0, hist: None };
        let b = t.session.banner();
        t.print(&b, 0);
        t.lines.push((String::new(), 0));
        t
    }

    fn print(&mut self, s: &str, kind: u8) {
        if s.is_empty() {
            return;
        }
        let s = s.strip_suffix('\n').unwrap_or(s);
        for l in s.split('\n') {
            self.lines.push((l.replace('\r', "").replace('\t', "  "), kind));
        }
        if self.lines.len() > 2000 {
            let n = self.lines.len() - 2000;
            self.lines.drain(..n);
        }
    }

    fn with_session<R>(&mut self, ctx: &mut Ctx, f: impl FnOnce(&mut Session) -> R) -> R {
        core::mem::swap(&mut self.session.vfs, ctx.vfs);
        self.session.set_time(ctx.now);
        let mut sys = ctx.sys.clone();
        sys.mac_desktop = ctx.st.exp == Exp::Mac;
        self.session.sys = sys;
        let r = f(&mut self.session);
        core::mem::swap(&mut self.session.vfs, ctx.vfs);
        r
    }

    fn colors(&self, dark: bool) -> (Color, Color, Color) {
        match self.session.flavor {
            Flavor::PowerShell => (0x012456, 0xEEEDF0, 0xFF6B6B),
            Flavor::Cmd => (0x0C0C0C, 0xCCCCCC, 0xCCCCCC),
            Flavor::Mac => {
                if dark {
                    (0x1E1E1E, 0xE8E8E8, 0xFF7B72)
                } else {
                    (0xFFFFFF, 0x1A1A1A, 0xC8321E)
                }
            }
        }
    }

    fn rows(&self, cols: usize) -> (Vec<(String, u8)>, (usize, usize)) {
        let mut rows: Vec<(String, u8)> = Vec::new();
        let first = self.lines.len().saturating_sub(400);
        // The last stored line is always the (empty) live line placeholder: skip it.
        let hist = &self.lines[first..self.lines.len().saturating_sub(1)];
        for (l, k) in hist {
            let cs: Vec<char> = l.chars().collect();
            if cs.is_empty() {
                rows.push((String::new(), *k));
            }
            for ch in cs.chunks(cols) {
                rows.push((ch.iter().collect(), *k));
            }
        }
        let prompt = self.session.prompt();
        let live: Vec<char> = prompt.chars().chain(self.input.chars()).collect();
        let pos = prompt.chars().count() + self.input[..self.byte_idx()].chars().count();
        let live_start = rows.len();
        let nrows = (live.len().max(pos + 1)).div_ceil(cols).max(1);
        for i in 0..nrows {
            let a = i * cols;
            let b = ((i + 1) * cols).min(live.len());
            rows.push((if a < live.len() { live[a..b].iter().collect() } else { String::new() }, 0));
        }
        (rows, (live_start + pos / cols, pos % cols))
    }

    fn byte_idx(&self) -> usize {
        self.input.char_indices().nth(self.cur).map(|(i, _)| i).unwrap_or(self.input.len())
    }

    pub fn draw(&mut self, cv: &mut Canvas, r: Rect, ctx: &mut Ctx) {
        let (bg, fg, errc) = self.colors(ctx.pal.dark);
        cv.fill(r, bg);
        let size = 15;
        let (cw, ch) = cv.mono_cell(size);
        let pad = 12;
        let cols = (((r.w - 2 * pad) / cw).max(10)) as usize;
        let vis = (((r.h - 2 * pad) / ch).max(1)) as usize;
        self.session.cols = cols;
        let (rows, (crow, ccol)) = self.rows(cols);
        let total = rows.len();
        self.scroll = self.scroll.min(total.saturating_sub(vis));
        let start = total.saturating_sub(vis + self.scroll);
        let end = (start + vis).min(total);
        let prev = cv.set_clip(r.inset(1));
        for (i, (txt, kind)) in rows[start..end].iter().enumerate() {
            let y = r.y + pad + i as i32 * ch + ch / 2;
            cv.text(F::Mono, size, r.x + pad, y, txt, if *kind == 1 { errc } else { fg }, 255);
        }
        if crow >= start && crow < end {
            let blink = (ctx.ms / 530) % 2 == 0;
            let cx = r.x + pad + ccol as i32 * cw;
            let cy = r.y + pad + (crow - start) as i32 * ch;
            if blink {
                cv.fill_a(Rect::new(cx, cy + 1, cw, ch - 2), fg, 230);
                let c = self.session_char_at(crow, ccol, cols, &rows);
                if let Some(c) = c {
                    let mut b = [0u8; 4];
                    cv.text(F::Mono, size, cx, cy + ch / 2, c.encode_utf8(&mut b), bg, 255);
                }
            }
        }
        cv.clip = prev;
    }

    fn session_char_at(&self, row: usize, col: usize, _cols: usize, rows: &[(String, u8)]) -> Option<char> {
        rows.get(row).and_then(|(s, _)| s.chars().nth(col))
    }

    fn submit(&mut self, ctx: &mut Ctx) -> Option<Action> {
        let prompt = self.session.prompt();
        let line = core::mem::take(&mut self.input);
        self.cur = 0;
        self.hist = None;
        self.scroll = 0;
        let n = self.lines.len();
        self.lines.truncate(n.saturating_sub(1));
        self.lines.push((format!("{prompt}{line}"), 0));
        let out = match crate::programs::try_run(&line, self.session.flavor, ctx.vfs, &self.session.cwd) {
            Some((o, code)) => {
                let var = if self.session.flavor == Flavor::PowerShell { "LASTEXITCODE" } else { "ERRORLEVEL" };
                self.session.vars.insert(var.to_string(), code.to_string());
                self.session.history.push(line.clone());
                o
            }
            None => self.with_session(ctx, |s| s.exec(&line)),
        };
        if out.clear {
            self.lines.clear();
        }
        self.print(&out.out, 0);
        self.print(&out.err, 1);
        self.lines.push((String::new(), 0));
        if out.exit {
            return Some(Action::Close);
        }
        None
    }

    pub fn key(&mut self, k: KeyEv, ctx: &mut Ctx) -> Option<Action> {
        if k.ctrl {
            if let Key::Char(c) = k.key {
                match c.to_ascii_lowercase() {
                    'l' => {
                        self.lines.clear();
                        self.lines.push((String::new(), 0));
                    }
                    'c' => {
                        let p = self.session.prompt();
                        let n = self.lines.len();
                        self.lines.truncate(n.saturating_sub(1));
                        self.lines.push((format!("{p}{}^C", self.input), 0));
                        self.lines.push((String::new(), 0));
                        self.input.clear();
                        self.cur = 0;
                    }
                    'u' => {
                        self.input.clear();
                        self.cur = 0;
                    }
                    'a' => self.cur = 0,
                    'e' => self.cur = self.input.chars().count(),
                    _ => {}
                }
            }
            return None;
        }
        self.scroll = 0;
        match k.key {
            Key::Char(c) => {
                let i = self.byte_idx();
                self.input.insert(i, c);
                self.cur += 1;
            }
            Key::Backspace => {
                if self.cur > 0 {
                    self.cur -= 1;
                    let i = self.byte_idx();
                    self.input.remove(i);
                }
            }
            Key::Delete => {
                if self.cur < self.input.chars().count() {
                    let i = self.byte_idx();
                    self.input.remove(i);
                }
            }
            Key::Left => self.cur = self.cur.saturating_sub(1),
            Key::Right => self.cur = (self.cur + 1).min(self.input.chars().count()),
            Key::Home => self.cur = 0,
            Key::End => self.cur = self.input.chars().count(),
            Key::Up => {
                let n = self.session.history.len();
                if n > 0 {
                    let i = match self.hist {
                        None => n - 1,
                        Some(i) => i.saturating_sub(1),
                    };
                    self.hist = Some(i);
                    self.input = self.session.history[i].clone();
                    self.cur = self.input.chars().count();
                }
            }
            Key::Down => {
                let n = self.session.history.len();
                match self.hist {
                    Some(i) if i + 1 < n => {
                        self.hist = Some(i + 1);
                        self.input = self.session.history[i + 1].clone();
                    }
                    _ => {
                        self.hist = None;
                        self.input.clear();
                    }
                }
                self.cur = self.input.chars().count();
            }
            Key::PageUp => self.scroll += 10,
            Key::PageDown => self.scroll = self.scroll.saturating_sub(10),
            Key::Enter => return self.submit(ctx),
            Key::Tab => self.complete(ctx),
            _ => {}
        }
        None
    }

    fn complete(&mut self, ctx: &mut Ctx) {
        let input = self.input.clone();
        let cands = self.with_session(ctx, |s| s.complete(&input));
        if cands.is_empty() {
            return;
        }
        let word = input.rsplit(' ').next().unwrap_or("").to_string();
        let head = &input[..input.len() - word.len()];
        if cands.len() == 1 {
            let c = &cands[0];
            let sep = c.ends_with('\\') || c.ends_with('/');
            self.input = format!("{head}{c}{}", if sep { "" } else { " " });
        } else {
            // longest common prefix (case-insensitive)
            let first: Vec<char> = cands[0].chars().collect();
            let mut n = first.len();
            for c in &cands[1..] {
                let cc: Vec<char> = c.chars().collect();
                n = n.min(first.iter().zip(cc.iter()).take_while(|(a, b)| a.to_ascii_lowercase() == b.to_ascii_lowercase()).count());
            }
            let prefix: String = first[..n].iter().collect();
            if prefix.chars().count() > word.chars().count() {
                self.input = format!("{head}{prefix}");
            } else {
                let p = self.session.prompt();
                let m = self.lines.len();
                self.lines.truncate(m.saturating_sub(1));
                self.lines.push((format!("{p}{input}"), 0));
                self.print(&cands.join("  "), 0);
                self.lines.push((String::new(), 0));
            }
        }
        self.cur = self.input.chars().count();
    }
}

// =============================================================================
// Files
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
enum FHit {
    Back,
    Up,
    New,
    Place(String),
    Row(usize),
}

pub struct FilesApp {
    pub path: String,
    selected: Option<usize>,
    pub scroll: i32,
    last_click: (usize, u64),
    hits: Vec<(Rect, FHit)>,
    back: Vec<String>,
}

fn fmt_date(t: Stamp, exp: Exp) -> String {
    let h = if t.hour % 12 == 0 { 12 } else { t.hour % 12 };
    let ap = if t.hour >= 12 { "PM" } else { "AM" };
    match exp {
        Exp::Win => format!("{}/{}/{} {}:{:02} {}", t.month, t.day, t.year, h, t.minute, ap),
        Exp::Mac => format!("{} {}, {} at {}:{:02} {}", &MONTHS[(t.month.clamp(1, 12) - 1) as usize][..3], t.day, t.year, h, t.minute, ap),
    }
}

fn fmt_size(n: usize) -> String {
    if n < 1024 {
        format!("{} KB", 1)
    } else {
        format!("{} KB", n.div_ceil(1024))
    }
}

impl FilesApp {
    pub fn new() -> Self {
        Self { path: "/Users/aeris".to_string(), selected: None, scroll: 0, last_click: (usize::MAX, 0), hits: Vec::new(), back: Vec::new() }
    }

    fn entries(&self, vfs: &Vfs) -> Vec<Entry> {
        let mut v = vfs.list(&self.path).unwrap_or_default();
        v.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then(a.name.to_lowercase().cmp(&b.name.to_lowercase())));
        v
    }

    fn go(&mut self, p: &str) {
        if p != self.path {
            self.back.push(core::mem::replace(&mut self.path, p.to_string()));
        }
        self.selected = None;
        self.scroll = 0;
    }

    fn child(&self, name: &str) -> String {
        format!("{}/{}", self.path.trim_end_matches('/'), name)
    }

    pub fn draw(&mut self, cv: &mut Canvas, r: Rect, ctx: &mut Ctx) {
        let pal = ctx.pal;
        let exp = ctx.st.exp;
        let (mx, my) = ctx.mouse;
        let mut hits: Vec<(Rect, FHit)> = Vec::new();
        cv.fill(r, pal.content);
        // toolbar
        let tb = Rect::new(r.x, r.y, r.w, 46);
        cv.fill(tb, pal.win);
        cv.fill(Rect::new(r.x, tb.bottom() - 1, r.w, 1), pal.line);
        let bb = Rect::new(r.x + 10, r.y + 8, 30, 30);
        let ub = Rect::new(r.x + 44, r.y + 8, 30, 30);
        for (b, dir, hit, enabled) in [(bb, 3, FHit::Back, !self.back.is_empty()), (ub, 0, FHit::Up, self.path != "/")] {
            if enabled && b.contains(mx, my) {
                cv.rrect(b, 6, pal.text, 24);
            }
            icons::chevron(cv, b.cx(), b.cy(), 14, pal.text, dir);
            if !enabled {
                cv.rrect(b, 6, pal.win, 140);
            }
            hits.push((b, hit));
        }
        let new_w = if exp == Exp::Win { 104 } else { 34 };
        let nb = Rect::new(r.right() - new_w - 10, r.y + 8, new_w, 30);
        let pb = Rect::new(r.x + 82, r.y + 8, (nb.x - 12) - (r.x + 82), 30);
        cv.rrect(pb, if exp == Exp::Win { 5 } else { 15 }, pal.content, 255);
        cv.rrect_stroke(pb, if exp == Exp::Win { 5 } else { 15 }, 1, pal.line, 255);
        let crumb = if exp == Exp::Win {
            let mut s = String::from("This PC");
            for p in self.path.split('/').filter(|x| !x.is_empty()) {
                s.push_str("  \u{203A}  ");
                s.push_str(p);
            }
            s
        } else {
            let mut s = String::from("aeris");
            for p in self.path.split('/').filter(|x| !x.is_empty()).skip(2) {
                s.push_str("  \u{203A}  ");
                s.push_str(p);
            }
            s
        };
        let crumb = cv.fit(F::Ui, 14, &crumb, pb.w - 24);
        cv.text(F::Ui, 14, pb.x + 12, pb.cy(), &crumb, pal.text, 255);
        if exp == Exp::Win {
            widgets::button(cv, nb, "+  New folder", BtnStyle::Secondary, &pal, exp, nb.contains(mx, my));
        } else {
            widgets::button(cv, nb, "+", BtnStyle::Secondary, &pal, exp, nb.contains(mx, my));
        }
        hits.push((nb, FHit::New));
        // sidebar
        let sw = 176;
        let side = Rect::new(r.x, tb.bottom(), sw, r.h - 46 - 26);
        cv.fill(side, pal.side);
        cv.fill(Rect::new(side.right() - 1, side.y, 1, side.h), pal.line);
        let places = [
            ("Home", "/Users/aeris"),
            ("Desktop", "/Users/aeris/Desktop"),
            ("Documents", "/Users/aeris/Documents"),
            ("Downloads", "/Users/aeris/Downloads"),
            ("Pictures", "/Users/aeris/Pictures"),
            ("Music", "/Users/aeris/Music"),
            ("System", "/System"),
        ];
        if exp == Exp::Mac {
            cv.text(F::Bold, 11, side.x + 16, side.y + 18, "Favourites", pal.dim, 255);
        }
        let top = side.y + if exp == Exp::Mac { 36 } else { 10 };
        for (i, (name, p)) in places.iter().enumerate() {
            let row = Rect::new(side.x + 8, top + i as i32 * 32, sw - 16, 30);
            let sel = self.path == *p;
            if sel {
                if exp == Exp::Win {
                    cv.rrect(row, 5, pal.text, 22);
                    cv.rrect(Rect::new(row.x, row.y + 7, 3, 16), 2, pal.accent, 255);
                } else {
                    cv.rrect(row, 7, pal.accent, 255);
                }
            } else if row.contains(mx, my) {
                cv.rrect(row, if exp == Exp::Win { 5 } else { 7 }, pal.text, 14);
            }
            icons::folder_small(cv, Rect::new(row.x + 12, row.y + 5, 20, 20), exp);
            let tc = if sel && exp == Exp::Mac { 0xFFFFFF } else { pal.text };
            cv.text(F::Ui, 14, row.x + 42, row.cy(), name, tc, 255);
            hits.push((row, FHit::Place(p.to_string())));
        }
        // list
        let list = Rect::new(side.right(), tb.bottom(), r.w - sw, r.h - 46 - 26);
        let prev = cv.set_clip(list);
        let entries = self.entries(ctx.vfs);
        let colw = [list.w - 420, 170, 120, 90];
        let hdr_y = list.y + 15;
        let mut x = list.x + 48;
        for (i, h) in ["Name", "Date modified", "Type", "Size"].iter().enumerate() {
            cv.text(F::Ui, 12, x, hdr_y, h, pal.dim, 255);
            x += if i == 0 { colw[0].max(120) - 20 } else { colw[i] };
        }
        cv.fill(Rect::new(list.x, list.y + 30, list.w, 1), pal.line);
        let row_h = 34;
        let max_scroll = (entries.len() as i32 * row_h - (list.h - 36)).max(0);
        self.scroll = self.scroll.clamp(0, max_scroll);
        for (i, e) in entries.iter().enumerate() {
            let ry = list.y + 36 + i as i32 * row_h - self.scroll;
            if ry + row_h < list.y + 31 || ry > list.bottom() {
                continue;
            }
            let row = Rect::new(list.x + 6, ry, list.w - 12, row_h - 2);
            let sel = self.selected == Some(i);
            let hov = row.contains(mx, my) && list.contains(mx, my);
            if sel {
                if exp == Exp::Win {
                    cv.rrect(row, 5, pal.accent, 46);
                } else {
                    cv.rrect(row, 7, pal.accent, 255);
                }
            } else if hov {
                cv.rrect(row, if exp == Exp::Win { 5 } else { 7 }, pal.text, 12);
            }
            let tc = if sel && exp == Exp::Mac { 0xFFFFFF } else { pal.text };
            let dc = if sel && exp == Exp::Mac { 0xE6EEFF } else { pal.dim };
            if e.is_dir {
                icons::folder_small(cv, Rect::new(row.x + 10, row.y + 5, 22, 22), exp);
            } else {
                icons::file_small(cv, Rect::new(row.x + 10, row.y + 5, 22, 22), pal.accent, pal.dark);
            }
            let name = cv.fit(F::Ui, 14, &e.name, colw[0].max(120) - 50);
            cv.text(F::Ui, 14, list.x + 48, row.cy(), &name, tc, 255);
            let mut x = list.x + 48 + colw[0].max(120) - 20;
            cv.text(F::Ui, 13, x, row.cy(), &fmt_date(e.stamp, exp), dc, 255);
            x += colw[1];
            let ty = match (e.is_dir, exp) {
                (true, Exp::Win) => "File folder",
                (true, Exp::Mac) => "Folder",
                (false, Exp::Win) => "Text Document",
                (false, Exp::Mac) => "Plain Text",
            };
            cv.text(F::Ui, 13, x, row.cy(), ty, dc, 255);
            x += colw[2];
            if !e.is_dir {
                cv.text(F::Ui, 13, x, row.cy(), &fmt_size(e.size), dc, 255);
            }
            if list.contains(mx, my) || true {
                hits.push((row.intersect(&list), FHit::Row(i)));
            }
        }
        if entries.is_empty() {
            cv.text_c(F::Ui, 14, list.cx(), list.cy(), "This folder is empty", pal.dim, 255);
        }
        cv.clip = prev;
        // status bar
        let sb = Rect::new(r.x, r.bottom() - 26, r.w, 26);
        cv.fill(sb, pal.win);
        cv.fill(Rect::new(sb.x, sb.y, sb.w, 1), pal.line);
        let status = match self.selected.and_then(|i| entries.get(i)) {
            Some(e) => format!("{} items   |   1 item selected  {}", entries.len(), if e.is_dir { String::new() } else { fmt_size(e.size) }),
            None => format!("{} items", entries.len()),
        };
        cv.text(F::Ui, 12, sb.x + 14, sb.cy(), &status, pal.dim, 255);
        self.hits = hits;
    }

    pub fn mouse_down(&mut self, x: i32, y: i32, ctx: &mut Ctx) -> Option<Action> {
        let hit = self.hits.iter().rev().find(|(r, _)| r.contains(x, y)).map(|(_, h)| h.clone())?;
        match hit {
            FHit::Back => {
                if let Some(p) = self.back.pop() {
                    self.path = p;
                    self.selected = None;
                }
            }
            FHit::Up => {
                let p = Vfs::parent(&self.path).to_string();
                self.go(&p);
            }
            FHit::New => self.new_folder(ctx),
            FHit::Place(p) => self.go(&p),
            FHit::Row(i) => {
                let entries = self.entries(ctx.vfs);
                let dbl = self.last_click.0 == i && ctx.ms.saturating_sub(self.last_click.1) < 450;
                self.last_click = (i, ctx.ms);
                self.selected = Some(i);
                if dbl {
                    return self.open_selected(&entries);
                }
            }
        }
        None
    }

    fn open_selected(&mut self, entries: &[Entry]) -> Option<Action> {
        let e = entries.get(self.selected?)?;
        let p = self.child(&e.name);
        if e.is_dir {
            self.go(&p);
            None
        } else {
            Some(Action::OpenNotes(p))
        }
    }

    fn new_folder(&mut self, ctx: &mut Ctx) {
        let mut n = 1;
        loop {
            let name = if n == 1 { "New folder".to_string() } else { format!("New folder ({n})") };
            let p = self.child(&name);
            if !ctx.vfs.exists(&p) {
                ctx.vfs.now = ctx.now;
                let _ = ctx.vfs.mkdir(&p, false);
                break;
            }
            n += 1;
        }
    }

    pub fn key(&mut self, k: KeyEv, ctx: &mut Ctx) -> Option<Action> {
        let entries = self.entries(ctx.vfs);
        match k.key {
            Key::Down => self.selected = Some(self.selected.map_or(0, |i| (i + 1).min(entries.len().saturating_sub(1)))),
            Key::Up => self.selected = Some(self.selected.map_or(0, |i| i.saturating_sub(1))),
            Key::Enter => return self.open_selected(&entries),
            Key::Backspace => {
                let p = Vfs::parent(&self.path).to_string();
                self.go(&p);
            }
            Key::Delete => {
                if let Some(e) = self.selected.and_then(|i| entries.get(i)) {
                    let p = self.child(&e.name);
                    let _ = ctx.vfs.remove(&p, true);
                    self.selected = None;
                }
            }
            _ => {}
        }
        None
    }
}

// =============================================================================
// Settings
// =============================================================================

#[derive(Clone, Debug, PartialEq)]
enum SHit {
    Page(usize),
    Exp(Exp),
    Dark(bool),
    Accent(usize),
    Shutdown,
    Reboot,
}

pub struct SettingsApp {
    pub page: usize,
    hits: Vec<(Rect, SHit)>,
}

impl SettingsApp {
    pub fn new() -> Self {
        Self { page: 1, hits: Vec::new() }
    }

    pub fn open_about(&mut self) {
        self.page = 3;
    }

    pub fn draw(&mut self, cv: &mut Canvas, r: Rect, ctx: &mut Ctx) {
        let pal = ctx.pal;
        let exp = ctx.st.exp;
        let (mx, my) = ctx.mouse;
        let mut hits: Vec<(Rect, SHit)> = Vec::new();
        cv.fill(r, pal.win);
        let sw = 220;
        let side = Rect::new(r.x, r.y, sw, r.h);
        cv.fill(side, pal.side);
        cv.fill(Rect::new(side.right() - 1, r.y, 1, r.h), pal.line);
        // profile
        let av = Rect::new(side.x + 16, side.y + 16, 44, 44);
        cv.circle(av.cx(), av.cy(), 22, pal.accent, 255);
        let initial: String = ctx.st.user.chars().next().unwrap_or('A').to_uppercase().collect();
        cv.text_c(F::Bold, 20, av.cx(), av.cy(), &initial, 0xFFFFFF, 255);
        cv.text(F::Bold, 14, av.right() + 12, av.y + 13, &ctx.st.user, pal.text, 255);
        cv.text(F::Ui, 12, av.right() + 12, av.y + 32, "Local account", pal.dim, 255);
        let pages = ["System", "Personalization", "Power", "About"];
        for (i, p) in pages.iter().enumerate() {
            let row = Rect::new(side.x + 8, side.y + 82 + i as i32 * 38, sw - 16, 34);
            let sel = i + usize::from(false) == self.page.saturating_sub(0) && false || self.page == i + 0 && false;
            let _ = sel;
            let active = self.page == [0, 1, 2, 3][i];
            if active {
                if exp == Exp::Win {
                    cv.rrect(row, 5, pal.text, 20);
                    cv.rrect(Rect::new(row.x, row.y + 9, 3, 16), 2, pal.accent, 255);
                } else {
                    cv.rrect(row, 8, pal.accent, 255);
                }
            } else if row.contains(mx, my) {
                cv.rrect(row, 6, pal.text, 12);
            }
            let tc = if active && exp == Exp::Mac { 0xFFFFFF } else { pal.text };
            cv.text(F::Ui, 14, row.x + 16, row.cy(), p, tc, 255);
            hits.push((row, SHit::Page(i)));
        }
        // page
        let c = Rect::new(side.right() + 28, r.y + 22, r.w - sw - 56, r.h - 44);
        let prev = cv.set_clip(Rect::new(side.right(), r.y, r.w - sw, r.h));
        match self.page {
            0 => self.page_system(cv, c, ctx),
            1 => self.page_personalize(cv, c, ctx, &mut hits),
            2 => self.page_power(cv, c, ctx, &mut hits),
            _ => self.page_about(cv, c, ctx),
        }
        cv.clip = prev;
        self.hits = hits;
    }

    fn card(cv: &mut Canvas, r: Rect, pal: &Pal, exp: Exp) {
        let rad = if exp == Exp::Win { 6 } else { 12 };
        cv.rrect(r, rad, pal.card, 255);
        cv.rrect_stroke(r, rad, 1, pal.line, 255);
    }

    fn kv(cv: &mut Canvas, x: i32, right: i32, y: i32, k: &str, v: &str, pal: &Pal) {
        cv.text(F::Ui, 14, x, y, k, pal.text, 255);
        cv.text_r(F::Ui, 14, right, y, v, pal.dim, 255);
    }

    fn page_system(&mut self, cv: &mut Canvas, c: Rect, ctx: &mut Ctx) {
        let pal = ctx.pal;
        cv.text(F::Bold, 26, c.x, c.y + 18, "System", pal.text, 255);
        let card = Rect::new(c.x, c.y + 54, c.w, 214);
        Self::card(cv, card, &pal, ctx.st.exp);
        let s = ctx.sys;
        let rows = [
            ("Display", format!("{} \u{00D7} {}  (32-bit colour)", s.width, s.height)),
            ("Memory", format!("{} MB used of {} MB", s.mem_used_mb, s.mem_total_mb)),
            ("Storage", format!("RAM disk \u{00B7} {} files", ctx.vfs.total_files())),
            ("Uptime", format!("{} min {} s", s.uptime_s / 60, s.uptime_s % 60)),
            ("Date and time", format!("{}-{:02}-{:02}  {:02}:{:02}:{:02} UTC", ctx.now.year, ctx.now.month, ctx.now.day, ctx.now.hour, ctx.now.minute, ctx.now.second)),
        ];
        for (i, (k, v)) in rows.iter().enumerate() {
            let y = card.y + 28 + i as i32 * 36;
            Self::kv(cv, card.x + 20, card.right() - 20, y, k, v, &pal);
            if i + 1 < rows.len() {
                cv.fill(Rect::new(card.x + 16, y + 18, card.w - 32, 1), pal.line);
            }
        }
        // memory bar
        let bar = Rect::new(card.x + 20, card.bottom() + 20, card.w - 40, 8);
        cv.rrect(bar, 4, pal.line, 255);
        let used = bar.w * s.mem_used_mb.min(s.mem_total_mb.max(1)) as i32 / s.mem_total_mb.max(1) as i32;
        cv.rrect(Rect::new(bar.x, bar.y, used.max(8), bar.h), 4, pal.accent, 255);
    }

    fn page_personalize(&mut self, cv: &mut Canvas, c: Rect, ctx: &mut Ctx, hits: &mut Vec<(Rect, SHit)>) {
        let pal = ctx.pal;
        let st = ctx.st;
        let (mx, my) = ctx.mouse;
        cv.text(F::Bold, 26, c.x, c.y + 18, "Personalization", pal.text, 255);
        cv.text(F::Bold, 15, c.x, c.y + 66, "Experience", pal.text, 255);
        cv.text(F::Ui, 12, c.x, c.y + 86, "Switch the whole desktop between the two styles. Applies instantly.", pal.dim, 255);
        let tw = ((c.w - 20) / 2).min(300);
        let th = tw * 5 / 8;
        for (i, exp) in [Exp::Win, Exp::Mac].iter().enumerate() {
            let r = Rect::new(c.x + i as i32 * (tw + 20), c.y + 104, tw, th);
            widgets::exp_preview(cv, r, *exp, st.dark, st.accent_color());
            let sel = st.exp == *exp;
            if sel {
                cv.rrect_stroke(r.inset(-3), 13, 3, pal.accent, 255);
                widgets::check_badge(cv, r.right() - 14, r.y + 14, 10, pal.accent);
            } else if r.contains(mx, my) {
                cv.rrect_stroke(r.inset(-3), 13, 2, pal.dim, 255);
            }
            cv.text(F::Bold, 14, r.x, r.bottom() + 20, exp.name(), pal.text, 255);
            hits.push((r, SHit::Exp(*exp)));
        }
        let y2 = c.y + 104 + th + 52;
        cv.text(F::Bold, 15, c.x, y2, "Appearance", pal.text, 255);
        let sw = 150;
        let sh = sw * 5 / 8;
        for (i, dark) in [false, true].iter().enumerate() {
            let r = Rect::new(c.x + i as i32 * (sw + 20), y2 + 18, sw, sh);
            widgets::appearance_tile(cv, r, *dark, st.exp, st.accent_color());
            if st.dark == *dark {
                cv.rrect_stroke(r.inset(-3), 13, 3, pal.accent, 255);
            } else if r.contains(mx, my) {
                cv.rrect_stroke(r.inset(-3), 13, 2, pal.dim, 255);
            }
            cv.text(F::Ui, 13, r.x, r.bottom() + 18, if *dark { "Dark" } else { "Light" }, pal.text, 255);
            hits.push((r, SHit::Dark(*dark)));
        }
        let ax = c.x + 2 * (sw + 20) + 20;
        cv.text(F::Bold, 15, ax, y2, "Accent colour", pal.text, 255);
        for (i, (_, col)) in ACCENTS.iter().enumerate() {
            let cx = ax + 16 + (i as i32 % 3) * 44;
            let cy = y2 + 44 + (i as i32 / 3) * 44;
            cv.circle(cx, cy, 15, *col, 255);
            if st.accent == i {
                cv.ring(cx, cy, 19, 2, pal.text, 255);
                cv.line(cx - 5, cy, cx - 1, cy + 4, 2, 0xFFFFFF, 255);
                cv.line(cx - 1, cy + 4, cx + 6, cy - 4, 2, 0xFFFFFF, 255);
            } else if (mx - cx) * (mx - cx) + (my - cy) * (my - cy) < 19 * 19 {
                cv.ring(cx, cy, 19, 1, pal.dim, 255);
            }
            hits.push((Rect::new(cx - 18, cy - 18, 36, 36), SHit::Accent(i)));
        }
    }

    fn page_power(&mut self, cv: &mut Canvas, c: Rect, ctx: &mut Ctx, hits: &mut Vec<(Rect, SHit)>) {
        let pal = ctx.pal;
        let exp = ctx.st.exp;
        let (mx, my) = ctx.mouse;
        cv.text(F::Bold, 26, c.x, c.y + 18, "Power", pal.text, 255);
        let card = Rect::new(c.x, c.y + 54, c.w, 130);
        Self::card(cv, card, &pal, exp);
        cv.text(F::Ui, 14, card.x + 20, card.y + 30, "Restart this device", pal.text, 255);
        cv.text(F::Ui, 12, card.x + 20, card.y + 50, "Settings are kept in memory in this preview build.", pal.dim, 255);
        let b1 = Rect::new(card.right() - 120, card.y + 22, 100, 32);
        widgets::button(cv, b1, "Restart", BtnStyle::Secondary, &pal, exp, b1.contains(mx, my));
        hits.push((b1, SHit::Reboot));
        cv.fill(Rect::new(card.x + 16, card.y + 72, card.w - 32, 1), pal.line);
        cv.text(F::Ui, 14, card.x + 20, card.y + 100, "Shut down", pal.text, 255);
        let b2 = Rect::new(card.right() - 120, card.y + 86, 100, 32);
        widgets::button(cv, b2, "Shut down", BtnStyle::Primary, &pal, exp, b2.contains(mx, my));
        hits.push((b2, SHit::Shutdown));
    }

    fn page_about(&mut self, cv: &mut Canvas, c: Rect, ctx: &mut Ctx) {
        let pal = ctx.pal;
        cv.text(F::Bold, 26, c.x, c.y + 18, "About", pal.text, 255);
        let hero = Rect::new(c.x, c.y + 54, c.w, 120);
        Self::card(cv, hero, &pal, ctx.st.exp);
        icons::logo(cv, Rect::new(hero.x + 20, hero.y + 20, 80, 80));
        cv.text(F::Bold, 22, hero.x + 118, hero.y + 42, "Aeris OS", pal.text, 255);
        cv.text(F::Ui, 14, hero.x + 118, hero.y + 70, "Version 1.0  \u{00B7}  build 26100.1", pal.dim, 255);
        cv.text(F::Ui, 13, hero.x + 118, hero.y + 92, "An operating system written entirely in Rust.", pal.dim, 255);
        let card = Rect::new(c.x, hero.bottom() + 16, c.w, 180);
        Self::card(cv, card, &pal, ctx.st.exp);
        let rows = [
            ("Device name", ctx.sys_host()),
            ("Kernel", "aeris-kernel 0.1.0  (x86_64, no_std)".to_string()),
            ("Experience", ctx.st.exp.name().to_string()),
            ("User", ctx.st.user.clone()),
            ("Source", "github.com/LakoMoor/aeris".to_string()),
        ];
        for (i, (k, v)) in rows.iter().enumerate() {
            let y = card.y + 26 + i as i32 * 32;
            Self::kv(cv, card.x + 20, card.right() - 20, y, k, v, &pal);
            if i + 1 < rows.len() {
                cv.fill(Rect::new(card.x + 16, y + 16, card.w - 32, 1), pal.line);
            }
        }
    }

    pub fn mouse_down(&mut self, x: i32, y: i32, _ctx: &mut Ctx) -> Option<Action> {
        let hit = self.hits.iter().rev().find(|(r, _)| r.contains(x, y)).map(|(_, h)| h.clone())?;
        match hit {
            SHit::Page(p) => self.page = p,
            SHit::Exp(e) => return Some(Action::SetExp(e)),
            SHit::Dark(d) => return Some(Action::SetDark(d)),
            SHit::Accent(a) => return Some(Action::SetAccent(a)),
            SHit::Shutdown => return Some(Action::Shutdown),
            SHit::Reboot => return Some(Action::Reboot),
        }
        None
    }
}

impl Ctx<'_> {
    pub fn sys_host(&self) -> String {
        if self.st.exp == Exp::Mac { "aeris-mac".to_string() } else { "AERIS-PC".to_string() }
    }
}

// =============================================================================
// Notes
// =============================================================================

pub struct NotesApp {
    pub path: Option<String>,
    lines: Vec<String>,
    row: usize,
    col: usize,
    pub top: usize,
    pub dirty: bool,
    msg: Option<(String, u64)>,
    save_rect: Rect,
}

impl NotesApp {
    pub fn new(path: Option<String>, text: &str) -> Self {
        let mut lines: Vec<String> = text.lines().map(|l| l.to_string()).collect();
        if lines.is_empty() {
            lines.push(String::new());
        }
        Self { path, lines, row: 0, col: 0, top: 0, dirty: false, msg: None, save_rect: Rect::default() }
    }

    fn save(&mut self, ctx: &mut Ctx) {
        let path = self.path.clone().unwrap_or_else(|| "/Users/aeris/Documents/Untitled.txt".to_string());
        ctx.vfs.now = ctx.now;
        let mut text = self.lines.join("\n");
        text.push('\n');
        let ok = ctx.vfs.write(&path, &text, false).is_ok();
        if ok {
            self.path = Some(path.clone());
            self.dirty = false;
        }
        self.msg = Some((if ok { format!("Saved {}", Vfs::basename(&path)) } else { "Could not save".to_string() }, ctx.ms));
    }

    pub fn draw(&mut self, cv: &mut Canvas, r: Rect, ctx: &mut Ctx) {
        let pal = ctx.pal;
        let exp = ctx.st.exp;
        cv.fill(r, pal.content);
        let tb = Rect::new(r.x, r.y, r.w, 38);
        cv.fill(tb, pal.win);
        cv.fill(Rect::new(r.x, tb.bottom() - 1, r.w, 1), pal.line);
        let name = self.path.as_deref().map(Vfs::basename).unwrap_or("Untitled.txt");
        cv.text(F::Bold, 13, tb.x + 14, tb.cy(), name, pal.text, 255);
        self.save_rect = Rect::new(tb.right() - 84, tb.y + 5, 72, 28);
        widgets::button(cv, self.save_rect, "Save", BtnStyle::Secondary, &pal, exp, self.save_rect.contains(ctx.mouse.0, ctx.mouse.1));
        if let Some((m, t)) = &self.msg {
            if ctx.ms.saturating_sub(*t) < 2500 {
                cv.text_r(F::Ui, 12, self.save_rect.x - 12, tb.cy(), m, pal.dim, 255);
            }
        }
        let size = 15;
        let (cw, ch) = cv.mono_cell(size);
        let area = Rect::new(r.x + 12, tb.bottom() + 8, r.w - 24, r.h - 38 - 8 - 26);
        let vis = (area.h / ch).max(1) as usize;
        if self.row < self.top {
            self.top = self.row;
        }
        if self.row >= self.top + vis {
            self.top = self.row + 1 - vis;
        }
        self.top = self.top.min(self.lines.len().saturating_sub(1));
        let prev = cv.set_clip(area);
        let gutter = 4 * cw;
        for i in 0..vis {
            let li = self.top + i;
            let Some(l) = self.lines.get(li) else { break };
            let y = area.y + i as i32 * ch + ch / 2;
            cv.text_r(F::Mono, size, area.x + gutter - cw, y, &format!("{}", li + 1), pal.dim, 200);
            cv.text(F::Mono, size, area.x + gutter + cw / 2, y, l, pal.text, 255);
        }
        if (ctx.ms / 530) % 2 == 0 && self.row >= self.top && self.row < self.top + vis {
            let cx = area.x + gutter + cw / 2 + self.col as i32 * cw;
            let cy = area.y + (self.row - self.top) as i32 * ch;
            cv.fill(Rect::new(cx, cy + 2, 2, ch - 4), pal.accent);
        }
        cv.clip = prev;
        let sb = Rect::new(r.x, r.bottom() - 26, r.w, 26);
        cv.fill(sb, pal.win);
        cv.fill(Rect::new(sb.x, sb.y, sb.w, 1), pal.line);
        cv.text(F::Ui, 12, sb.x + 14, sb.cy(), &format!("Ln {}, Col {}", self.row + 1, self.col + 1), pal.dim, 255);
        cv.text_r(F::Ui, 12, sb.right() - 14, sb.cy(), "UTF-8   Ctrl+S to save", pal.dim, 255);
    }

    pub fn mouse_down(&mut self, x: i32, y: i32, ctx: &mut Ctx) -> Option<Action> {
        if self.save_rect.contains(x, y) {
            self.save(ctx);
        }
        None
    }

    fn line_len(&self) -> usize {
        self.lines[self.row].chars().count()
    }

    fn byte(&self, col: usize) -> usize {
        let l = &self.lines[self.row];
        l.char_indices().nth(col).map(|(i, _)| i).unwrap_or(l.len())
    }

    pub fn key(&mut self, k: KeyEv, ctx: &mut Ctx) -> Option<Action> {
        if k.ctrl {
            if let Key::Char(c) = k.key {
                if c.to_ascii_lowercase() == 's' {
                    self.save(ctx);
                }
            }
            return None;
        }
        match k.key {
            Key::Char(c) => {
                let b = self.byte(self.col);
                self.lines[self.row].insert(b, c);
                self.col += 1;
                self.dirty = true;
            }
            Key::Tab => {
                let b = self.byte(self.col);
                self.lines[self.row].insert_str(b, "    ");
                self.col += 4;
                self.dirty = true;
            }
            Key::Enter => {
                let b = self.byte(self.col);
                let rest = self.lines[self.row].split_off(b);
                self.lines.insert(self.row + 1, rest);
                self.row += 1;
                self.col = 0;
                self.dirty = true;
            }
            Key::Backspace => {
                if self.col > 0 {
                    self.col -= 1;
                    let b = self.byte(self.col);
                    self.lines[self.row].remove(b);
                    self.dirty = true;
                } else if self.row > 0 {
                    let cur = self.lines.remove(self.row);
                    self.row -= 1;
                    self.col = self.line_len();
                    self.lines[self.row].push_str(&cur);
                    self.dirty = true;
                }
            }
            Key::Delete => {
                if self.col < self.line_len() {
                    let b = self.byte(self.col);
                    self.lines[self.row].remove(b);
                    self.dirty = true;
                } else if self.row + 1 < self.lines.len() {
                    let next = self.lines.remove(self.row + 1);
                    self.lines[self.row].push_str(&next);
                    self.dirty = true;
                }
            }
            Key::Left => {
                if self.col > 0 {
                    self.col -= 1;
                } else if self.row > 0 {
                    self.row -= 1;
                    self.col = self.line_len();
                }
            }
            Key::Right => {
                if self.col < self.line_len() {
                    self.col += 1;
                } else if self.row + 1 < self.lines.len() {
                    self.row += 1;
                    self.col = 0;
                }
            }
            Key::Up => {
                if self.row > 0 {
                    self.row -= 1;
                    self.col = self.col.min(self.line_len());
                }
            }
            Key::Down => {
                if self.row + 1 < self.lines.len() {
                    self.row += 1;
                    self.col = self.col.min(self.line_len());
                }
            }
            Key::Home => self.col = 0,
            Key::End => self.col = self.line_len(),
            Key::PageUp => self.row = self.row.saturating_sub(15),
            Key::PageDown => self.row = (self.row + 15).min(self.lines.len() - 1),
            _ => {}
        }
        self.col = self.col.min(self.line_len());
        None
    }
}
