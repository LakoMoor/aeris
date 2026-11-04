//! The shell session: parsing, variables, pipelines and dialect handling.
//! Individual commands live in `cmds.rs`.

use crate::vfs::{Stamp, Vfs, HOME};
use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// Which command-line dialect a session speaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Flavor {
    /// Windows Command Prompt (`cmd.exe`).
    Cmd,
    /// Windows PowerShell.
    PowerShell,
    /// macOS Terminal (zsh).
    Mac,
}

impl Flavor {
    pub fn title(self) -> &'static str {
        match self {
            Flavor::Cmd => "Command Prompt",
            Flavor::PowerShell => "Windows PowerShell",
            Flavor::Mac => "Terminal \u{2014} zsh",
        }
    }
}

/// What a command produced.
#[derive(Default, Debug, Clone)]
pub struct Output {
    pub out: String,
    pub err: String,
    pub clear: bool,
    /// The terminal window should close.
    pub exit: bool,
    /// The dialect changed (a banner is already in `out`).
    pub switched: bool,
}

/// Live system facts, refreshed by the kernel.
#[derive(Clone, Debug)]
pub struct SysInfo {
    pub mem_total_mb: u32,
    pub mem_used_mb: u32,
    pub uptime_s: u64,
    pub width: u32,
    pub height: u32,
    pub mac_desktop: bool,
}

impl Default for SysInfo {
    fn default() -> Self {
        Self { mem_total_mb: 512, mem_used_mb: 96, uptime_s: 0, width: 1280, height: 800, mac_desktop: false }
    }
}

pub struct Session {
    pub flavor: Flavor,
    pub vfs: Vfs,
    pub cwd: String,
    pub vars: BTreeMap<String, String>,
    pub history: Vec<String>,
    pub cols: usize,
    pub sys: SysInfo,
    pub(crate) parents: Vec<Flavor>,
    pub(crate) cmd_echo: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct Tok {
    pub s: String,
    pub quoted: bool,
    pub single: bool,
}

#[derive(Default, Debug)]
pub(crate) struct Args {
    pub pos: Vec<String>,
    pub flags: Vec<String>,
    pub named: Vec<(String, String)>,
}

impl Args {
    /// Flag present? Dialects allow abbreviations (`-r` for `-Recurse`).
    pub fn flag(&self, full: &str) -> bool {
        self.flags.iter().any(|f| !f.is_empty() && full.starts_with(f.as_str()))
    }
    pub fn flag_exact(&self, f: &str) -> bool {
        self.flags.iter().any(|x| x == f)
    }
    pub fn named(&self, full: &str) -> Option<&str> {
        self.named.iter().find(|(k, _)| !k.is_empty() && full.starts_with(k.as_str())).map(|(_, v)| v.as_str())
    }
}

pub const MONTHS: [&str; 12] =
    ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];
pub const DAYS: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

impl Session {
    pub fn new(flavor: Flavor, now: Stamp, mac_desktop: bool) -> Self {
        let mut s = Self {
            flavor,
            vfs: Vfs::new(now),
            cwd: HOME.to_string(),
            vars: BTreeMap::new(),
            history: Vec::new(),
            cols: 80,
            sys: SysInfo { mac_desktop, ..SysInfo::default() },
            parents: Vec::new(),
            cmd_echo: true,
        };
        for (k, v) in [
            ("USERNAME", "aeris"),
            ("USER", "aeris"),
            ("COMPUTERNAME", if mac_desktop { "aeris-mac" } else { "AERIS-PC" }),
            ("OS", "Aeris_NT"),
            ("SHELL", "/bin/zsh"),
            ("HOME", HOME),
            ("USERPROFILE", "C:\\Users\\aeris"),
            ("SystemRoot", "C:\\System"),
            ("PATH", "C:\\System;C:\\System\\Aeris;C:\\Applications"),
            ("TERM", "aeris-256color"),
            ("LANG", "en_US.UTF-8"),
        ] {
            s.vars.insert(k.to_string(), v.to_string());
        }
        s
    }

    pub fn set_time(&mut self, now: Stamp) {
        self.vfs.now = now;
    }

    pub fn hostname(&self) -> &'static str {
        if self.sys.mac_desktop {
            "aeris-mac"
        } else {
            "AERIS-PC"
        }
    }

    // ---- path presentation -------------------------------------------------

    pub fn win_path(p: &str) -> String {
        if p == "/" {
            return "C:\\".to_string();
        }
        let mut s = String::from("C:");
        for part in p.split('/').filter(|x| !x.is_empty()) {
            s.push('\\');
            s.push_str(part);
        }
        s
    }

    pub fn mac_path(p: &str) -> String {
        if p == HOME {
            "~".to_string()
        } else if let Some(r) = p.strip_prefix("/Users/aeris/") {
            alloc::format!("~/{r}")
        } else {
            p.to_string()
        }
    }

    /// Path as the current dialect would print it.
    pub fn show(&self, p: &str) -> String {
        match self.flavor {
            Flavor::Mac => p.to_string(),
            _ => Self::win_path(p),
        }
    }

    pub fn prompt(&self) -> String {
        match self.flavor {
            Flavor::Cmd => alloc::format!("{}>", Self::win_path(&self.cwd)),
            Flavor::PowerShell => alloc::format!("PS {}> ", Self::win_path(&self.cwd)),
            Flavor::Mac => {
                let dir = if self.cwd == HOME {
                    "~".to_string()
                } else if self.cwd == "/" {
                    "/".to_string()
                } else {
                    Vfs::basename(&self.cwd).to_string()
                };
                alloc::format!("aeris@{} {} % ", self.hostname(), dir)
            }
        }
    }

    pub fn banner(&self) -> String {
        match self.flavor {
            Flavor::Cmd => "Aeris OS [Version 1.0.26100.1]\n(c) Aeris Project. All rights reserved.\n".to_string(),
            Flavor::PowerShell => {
                "Windows PowerShell (Aeris edition)\nCopyright (C) Aeris Project. All rights reserved.\n\nInstall the latest PowerShell for new features and improvements!\n".to_string()
            }
            Flavor::Mac => {
                let t = self.vfs.now;
                alloc::format!(
                    "Last login: {} {} {:>2} {:02}:{:02}:{:02} on console\nThe default interactive shell is now zsh.\n",
                    &DAYS[t.weekday() % 7][..3],
                    &MONTHS[(t.month.clamp(1, 12) - 1) as usize][..3],
                    t.day,
                    t.hour,
                    t.minute,
                    t.second
                )
            }
        }
    }

    // ---- tokenising --------------------------------------------------------

    #[allow(unused_assignments)]
    pub(crate) fn tokenize(line: &str) -> Vec<Tok> {
        let mut toks: Vec<Tok> = Vec::new();
        let mut cur = String::new();
        let (mut have, mut quoted, mut single) = (false, false, false);
        let mut q: Option<char> = None;
        let mut chars = line.chars().peekable();
        macro_rules! flush {
            () => {
                if have {
                    toks.push(Tok { s: core::mem::take(&mut cur), quoted, single });
                    have = false;
                    quoted = false;
                    single = false;
                }
            };
        }
        while let Some(c) = chars.next() {
            match q {
                Some(qc) => {
                    if c == qc {
                        q = None;
                    } else {
                        cur.push(c);
                    }
                }
                None => match c {
                    '"' | '\'' => {
                        q = Some(c);
                        have = true;
                        quoted = true;
                        single = c == '\'';
                    }
                    c if c.is_whitespace() => flush!(),
                    '>' => {
                        flush!();
                        let mut s = String::from(">");
                        if chars.peek() == Some(&'>') {
                            chars.next();
                            s.push('>');
                        }
                        toks.push(Tok { s, quoted: false, single: false });
                    }
                    _ => {
                        cur.push(c);
                        have = true;
                    }
                },
            }
        }
        flush!();
        toks
    }

    /// Split on a separator outside quotes.
    fn split_top<'a>(line: &'a str, seps: &[&str]) -> Vec<&'a str> {
        let mut parts = Vec::new();
        let mut start = 0;
        let mut q: Option<char> = None;
        let b = line.as_bytes();
        let mut i = 0;
        'outer: while i < b.len() {
            let c = b[i] as char;
            if let Some(qc) = q {
                if c == qc {
                    q = None;
                }
                i += 1;
                continue;
            }
            if c == '"' || c == '\'' {
                q = Some(c);
                i += 1;
                continue;
            }
            for sep in seps {
                if line[i..].starts_with(sep) {
                    // `||` must not be split by a single `|`
                    if *sep == "|" && line[i..].starts_with("||") {
                        continue;
                    }
                    parts.push(&line[start..i]);
                    i += sep.len();
                    start = i;
                    continue 'outer;
                }
            }
            i += 1;
        }
        parts.push(&line[start..]);
        parts
    }

    fn expand(&self, t: &Tok) -> String {
        if t.single && self.flavor != Flavor::Cmd {
            return t.s.clone();
        }
        let s = &t.s;
        match self.flavor {
            Flavor::Cmd => {
                let mut out = String::new();
                let mut rest = s.as_str();
                while let Some(i) = rest.find('%') {
                    out.push_str(&rest[..i]);
                    let after = &rest[i + 1..];
                    if let Some(j) = after.find('%') {
                        let name = &after[..j];
                        if name.eq_ignore_ascii_case("cd") {
                            out.push_str(&Self::win_path(&self.cwd));
                        } else if let Some(v) = self.get_var(name) {
                            out.push_str(&v);
                        }
                        rest = &after[j + 1..];
                    } else {
                        out.push('%');
                        rest = after;
                    }
                }
                out.push_str(rest);
                out
            }
            _ => {
                let mut out = String::new();
                let cs: Vec<char> = s.chars().collect();
                let mut i = 0;
                while i < cs.len() {
                    if cs[i] == '$' && i + 1 < cs.len() && (cs[i + 1].is_alphabetic() || cs[i + 1] == '_' || cs[i + 1] == '{') {
                        let mut j = i + 1;
                        let braced = cs[j] == '{';
                        if braced {
                            j += 1;
                        }
                        let st = j;
                        while j < cs.len() && (cs[j].is_alphanumeric() || cs[j] == '_' || cs[j] == ':') {
                            j += 1;
                        }
                        let name: String = cs[st..j].iter().collect();
                        if braced && j < cs.len() && cs[j] == '}' {
                            j += 1;
                        }
                        let name = name.strip_prefix("env:").unwrap_or(&name).to_string();
                        if name.eq_ignore_ascii_case("pwd") {
                            out.push_str(&self.show(&self.cwd));
                        } else if let Some(v) = self.get_var(&name) {
                            out.push_str(&v);
                        }
                        i = j;
                    } else {
                        out.push(cs[i]);
                        i += 1;
                    }
                }
                out
            }
        }
    }

    pub(crate) fn get_var(&self, name: &str) -> Option<String> {
        if self.flavor == Flavor::Mac {
            self.vars.get(name).cloned()
        } else {
            self.vars.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.clone())
        }
    }

    pub(crate) fn parse_args(&self, toks: &[Tok]) -> Args {
        const PS_VALUED: [&str; 12] = [
            "path", "name", "value", "itemtype", "destination", "pattern", "literalpath", "filter", "first", "last", "newname", "stream",
        ];
        let mut a = Args::default();
        let mut i = 0;
        while i < toks.len() {
            let t = &toks[i];
            let s = self.expand(t);
            let is_flag = !t.quoted
                && match self.flavor {
                    Flavor::Cmd => s.starts_with('/') && s.len() > 1 && s.len() <= 3 && s[1..].chars().all(|c| c.is_ascii_alphabetic()),
                    Flavor::PowerShell => s.len() > 1 && s.starts_with('-') && s[1..].chars().next().map_or(false, |c| c.is_alphabetic()),
                    Flavor::Mac => s.len() > 1 && s.starts_with('-') && s != "--",
                };
            if is_flag {
                match self.flavor {
                    Flavor::Cmd => a.flags.push(s[1..].to_lowercase()),
                    Flavor::PowerShell => {
                        let name = s[1..].to_lowercase();
                        if PS_VALUED.iter().any(|v| v.starts_with(name.as_str())) && i + 1 < toks.len() {
                            a.named.push((name, self.expand(&toks[i + 1])));
                            i += 1;
                        } else {
                            a.flags.push(name);
                        }
                    }
                    Flavor::Mac => {
                        if let Some(long) = s.strip_prefix("--") {
                            a.flags.push(long.to_string());
                        } else {
                            for c in s[1..].chars() {
                                a.flags.push(c.to_string());
                            }
                        }
                    }
                }
            } else {
                a.pos.push(s);
            }
            i += 1;
        }
        a
    }

    // ---- execution ---------------------------------------------------------

    /// Run one line typed by the user.
    pub fn exec(&mut self, line: &str) -> Output {
        let line = line.trim();
        let mut total = Output::default();
        if line.is_empty() {
            return total;
        }
        self.history.push(line.to_string());
        let seps: &[&str] = match self.flavor {
            Flavor::Cmd => &["&&", "&"],
            _ => &["&&", ";"],
        };
        for stmt in Self::split_top(line, seps) {
            let o = self.exec_pipeline(stmt.trim());
            total.out.push_str(&o.out);
            total.err.push_str(&o.err);
            total.clear |= o.clear;
            total.switched |= o.switched;
            if o.clear {
                total.out = o.out.clone();
            }
            if o.exit {
                total.exit = true;
                break;
            }
        }
        total
    }

    fn exec_pipeline(&mut self, stmt: &str) -> Output {
        if stmt.is_empty() {
            return Output::default();
        }
        // Variable assignment.
        if let Some(o) = self.try_assign(stmt) {
            return o;
        }
        let stages = Self::split_top(stmt, &["|"]);
        let mut data: Option<String> = None;
        let mut result = Output::default();
        let n = stages.len();
        for (idx, st) in stages.iter().enumerate() {
            let mut toks = Self::tokenize(st.trim());
            if toks.is_empty() {
                continue;
            }
            // Redirection on the last stage.
            let mut redirect: Option<(String, bool)> = None;
            if idx + 1 == n {
                if let Some(p) = toks.iter().position(|t| !t.quoted && (t.s == ">" || t.s == ">>")) {
                    let append = toks[p].s == ">>";
                    if p + 1 < toks.len() {
                        redirect = Some((self.expand(&toks[p + 1]), append));
                    }
                    toks.truncate(p);
                    if toks.is_empty() {
                        continue;
                    }
                }
            }
            let name = self.expand(&toks[0]);
            let args = self.parse_args(&toks[1..]);
            let mut o = self.run(&name, &args, data.as_deref());
            if let Some((target, append)) = redirect {
                let path = Vfs::resolve(&self.cwd, &target);
                let mut text = core::mem::take(&mut o.out);
                if !text.is_empty() && !text.ends_with('\n') {
                    text.push('\n');
                }
                if let Err(e) = self.vfs.write(&path, &text, append) {
                    o.err.push_str(&alloc::format!("{}\n", self.fs_msg("redirect", &target, e)));
                }
            }
            if !o.err.is_empty() {
                result.err.push_str(&o.err);
            }
            result.clear |= o.clear;
            result.exit |= o.exit;
            result.switched |= o.switched;
            if idx + 1 < n {
                data = Some(o.out);
            } else {
                result.out = o.out;
            }
        }
        for s in [&mut result.out, &mut result.err] {
            if !s.is_empty() && !s.ends_with('\n') {
                s.push('\n');
            }
        }
        result
    }

    fn try_assign(&mut self, stmt: &str) -> Option<Output> {
        match self.flavor {
            Flavor::PowerShell => {
                let rest = stmt.strip_prefix('$')?;
                let eq = rest.find('=')?;
                let name = rest[..eq].trim();
                if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_' || c == ':') {
                    return None;
                }
                let name = name.strip_prefix("env:").unwrap_or(name);
                let val_src = rest[eq + 1..].trim();
                let toks = Self::tokenize(val_src);
                let val: Vec<String> = toks.iter().map(|t| self.expand(t)).collect();
                self.vars.insert(name.to_string(), val.join(" "));
                Some(Output::default())
            }
            Flavor::Mac => {
                let (rest, _) = match stmt.strip_prefix("export ") {
                    Some(r) => (r.trim(), true),
                    None => (stmt, false),
                };
                let eq = rest.find('=')?;
                let name = &rest[..eq];
                if name.is_empty() || !name.chars().all(|c| c.is_alphanumeric() || c == '_') || name.chars().next()?.is_ascii_digit() {
                    return None;
                }
                let toks = Self::tokenize(&rest[eq + 1..]);
                let val: Vec<String> = toks.iter().map(|t| self.expand(t)).collect();
                self.vars.insert(name.to_string(), val.join(" "));
                Some(Output::default())
            }
            Flavor::Cmd => None,
        }
    }

    // ---- completion --------------------------------------------------------

    /// Candidates completing the last word of `line`.
    pub fn complete(&self, line: &str) -> Vec<String> {
        let first = !line.trim_start().contains(' ');
        let word = line.rsplit(' ').next().unwrap_or("");
        let mut out: Vec<String> = Vec::new();
        if first {
            for c in crate::cmds::command_names(self.flavor) {
                if c.to_lowercase().starts_with(&word.to_lowercase()) {
                    out.push(c.to_string());
                }
            }
        } else {
            let (dir_part, name_part) = match word.rfind(|c| c == '/' || c == '\\') {
                Some(i) => (&word[..=i], &word[i + 1..]),
                None => ("", word),
            };
            let dir = if dir_part.is_empty() { self.cwd.clone() } else { Vfs::resolve(&self.cwd, dir_part) };
            if let Ok(list) = self.vfs.list(&dir) {
                for e in list {
                    if e.name.to_lowercase().starts_with(&name_part.to_lowercase()) {
                        let mut s = alloc::format!("{dir_part}{}", e.name);
                        if e.is_dir {
                            s.push(if self.flavor == Flavor::Mac { '/' } else { '\\' });
                        }
                        out.push(s);
                    }
                }
            }
        }
        out.sort();
        out
    }
}
