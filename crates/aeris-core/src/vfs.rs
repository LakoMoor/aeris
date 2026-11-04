//! In-memory file system shared by every shell dialect.
//!
//! Paths are stored canonically, Unix-style (`/Users/aeris/Documents`).
//! Windows dialects present them as `C:\Users\aeris\Documents`.

use alloc::collections::BTreeMap;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

/// A wall-clock timestamp (minute resolution).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Stamp {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl Stamp {
    pub const fn new(year: u16, month: u8, day: u8, hour: u8, minute: u8) -> Self {
        Self { year, month, day, hour, minute, second: 0 }
    }

    /// Day of week, 0 = Sunday.
    pub fn weekday(&self) -> usize {
        // Sakamoto's algorithm.
        const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        let mut y = self.year as i32;
        let m = (self.month.clamp(1, 12) - 1) as usize;
        if m < 2 {
            y -= 1;
        }
        ((y + y / 4 - y / 100 + y / 400 + T[m] + self.day as i32) % 7) as usize
    }
}

#[derive(Clone, Debug)]
pub enum Kind {
    Dir,
    File(String),
}

#[derive(Clone, Debug)]
pub struct Node {
    pub kind: Kind,
    pub stamp: Stamp,
}

/// One row of a directory listing.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub is_dir: bool,
    pub size: usize,
    pub stamp: Stamp,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum FsError {
    NotFound,
    Exists,
    NotDir,
    IsDir,
    NotEmpty,
    Invalid,
}

pub struct Vfs {
    map: BTreeMap<String, Node>,
    pub now: Stamp,
}

pub const HOME: &str = "/Users/aeris";

impl Vfs {
    pub fn new(now: Stamp) -> Self {
        let mut v = Self { map: BTreeMap::new(), now };
        v.map.insert("/".to_string(), Node { kind: Kind::Dir, stamp: now });
        for d in [
            "/Users",
            HOME,
            "/Users/aeris/Desktop",
            "/Users/aeris/Documents",
            "/Users/aeris/Downloads",
            "/Users/aeris/Pictures",
            "/Users/aeris/Music",
            "/System",
            "/System/Aeris",
            "/Applications",
            "/Temp",
        ] {
            let _ = v.mkdir(d, true);
        }
        let _ = v.write(
            "/Users/aeris/Documents/readme.txt",
            "Welcome to Aeris OS!\nThis is a Rust operating system.\nTry: dir, ls, Get-ChildItem, neofetch\n",
            false,
        );
        let _ = v.write("/Users/aeris/Documents/todo.txt", "- write more Rust\n- boot on real hardware\n- ship it\n", false);
        let _ = v.write("/Users/aeris/Desktop/hello.txt", "Hello from Aeris.\n", false);
        let _ = v.write("/System/Aeris/version", "Aeris OS 1.0 (build 26100.1)\n", false);
        v
    }

    /// Turn user input into a canonical absolute path.
    pub fn resolve(cwd: &str, input: &str) -> String {
        let s: String = input.chars().map(|c| if c == '\\' { '/' } else { c }).collect();
        let mut s = s.as_str();
        let b = s.as_bytes();
        if b.len() >= 2 && b[0].is_ascii_alphabetic() && b[1] == b':' {
            s = &s[2..];
            if !s.starts_with('/') {
                return Self::join(cwd, s);
            }
        }
        if s == "~" {
            return HOME.to_string();
        }
        if let Some(rest) = s.strip_prefix("~/") {
            return Self::join(HOME, rest);
        }
        if s.starts_with('/') {
            Self::join("/", s)
        } else {
            Self::join(cwd, s)
        }
    }

    fn join(base: &str, rel: &str) -> String {
        let mut parts: Vec<&str> = Vec::new();
        for p in base.split('/').chain(rel.split('/')) {
            match p {
                "" | "." => {}
                ".." => {
                    parts.pop();
                }
                x => parts.push(x),
            }
        }
        if parts.is_empty() {
            "/".to_string()
        } else {
            let mut out = String::new();
            for p in parts {
                out.push('/');
                out.push_str(p);
            }
            out
        }
    }

    pub fn exists(&self, p: &str) -> bool {
        self.map.contains_key(p)
    }
    pub fn is_dir(&self, p: &str) -> bool {
        matches!(self.map.get(p), Some(Node { kind: Kind::Dir, .. }))
    }
    pub fn is_file(&self, p: &str) -> bool {
        matches!(self.map.get(p), Some(Node { kind: Kind::File(_), .. }))
    }

    pub fn parent(p: &str) -> &str {
        match p.rfind('/') {
            Some(0) | None => "/",
            Some(i) => &p[..i],
        }
    }
    pub fn basename(p: &str) -> &str {
        p.rsplit('/').next().unwrap_or(p)
    }

    pub fn mkdir(&mut self, p: &str, parents: bool) -> Result<(), FsError> {
        if self.exists(p) {
            return if parents && self.is_dir(p) { Ok(()) } else { Err(FsError::Exists) };
        }
        let par = Self::parent(p).to_string();
        if !self.exists(&par) {
            if parents {
                self.mkdir(&par, true)?;
            } else {
                return Err(FsError::NotFound);
            }
        } else if !self.is_dir(&par) {
            return Err(FsError::NotDir);
        }
        self.map.insert(p.to_string(), Node { kind: Kind::Dir, stamp: self.now });
        Ok(())
    }

    pub fn write(&mut self, p: &str, data: &str, append: bool) -> Result<(), FsError> {
        let par = Self::parent(p);
        if !self.is_dir(par) {
            return Err(if self.exists(par) { FsError::NotDir } else { FsError::NotFound });
        }
        let now = self.now;
        match self.map.get_mut(p) {
            Some(Node { kind: Kind::Dir, .. }) => Err(FsError::IsDir),
            Some(Node { kind: Kind::File(s), stamp }) => {
                if append {
                    s.push_str(data);
                } else {
                    *s = data.to_string();
                }
                *stamp = now;
                Ok(())
            }
            None => {
                self.map.insert(p.to_string(), Node { kind: Kind::File(data.to_string()), stamp: now });
                Ok(())
            }
        }
    }

    pub fn read(&self, p: &str) -> Result<&str, FsError> {
        match self.map.get(p) {
            Some(Node { kind: Kind::File(s), .. }) => Ok(s),
            Some(_) => Err(FsError::IsDir),
            None => Err(FsError::NotFound),
        }
    }

    pub fn list(&self, p: &str) -> Result<Vec<Entry>, FsError> {
        match self.map.get(p) {
            None => return Err(FsError::NotFound),
            Some(Node { kind: Kind::File(_), .. }) => return Err(FsError::NotDir),
            _ => {}
        }
        let prefix = if p == "/" { "/".to_string() } else { alloc::format!("{p}/") };
        let mut out = Vec::new();
        for (k, n) in self.map.range(prefix.clone()..) {
            if !k.starts_with(&prefix) {
                break;
            }
            let rest = &k[prefix.len()..];
            if rest.is_empty() || rest.contains('/') {
                continue;
            }
            out.push(Entry {
                name: rest.to_string(),
                is_dir: matches!(n.kind, Kind::Dir),
                size: if let Kind::File(s) = &n.kind { s.len() } else { 0 },
                stamp: n.stamp,
            });
        }
        Ok(out)
    }

    pub fn entry(&self, p: &str) -> Option<Entry> {
        self.map.get(p).map(|n| Entry {
            name: Self::basename(p).to_string(),
            is_dir: matches!(n.kind, Kind::Dir),
            size: if let Kind::File(s) = &n.kind { s.len() } else { 0 },
            stamp: n.stamp,
        })
    }

    pub fn remove(&mut self, p: &str, recursive: bool) -> Result<(), FsError> {
        if p == "/" {
            return Err(FsError::Invalid);
        }
        if !self.exists(p) {
            return Err(FsError::NotFound);
        }
        if self.is_dir(p) {
            let has_children = !self.list(p)?.is_empty();
            if has_children && !recursive {
                return Err(FsError::NotEmpty);
            }
            let prefix = alloc::format!("{p}/");
            let keys: Vec<String> = self.map.keys().filter(|k| k.starts_with(&prefix)).cloned().collect();
            for k in keys {
                self.map.remove(&k);
            }
        }
        self.map.remove(p);
        Ok(())
    }

    pub fn copy(&mut self, from: &str, to: &str) -> Result<(), FsError> {
        if !self.exists(from) {
            return Err(FsError::NotFound);
        }
        if self.is_dir(from) {
            let prefix = alloc::format!("{from}/");
            let items: Vec<(String, Node)> = self
                .map
                .iter()
                .filter(|(k, _)| k.as_str() == from || k.starts_with(&prefix))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            if !self.is_dir(Self::parent(to)) {
                return Err(FsError::NotFound);
            }
            for (k, v) in items {
                let nk = alloc::format!("{to}{}", &k[from.len()..]);
                self.map.insert(nk, v);
            }
            Ok(())
        } else {
            let data = self.read(from)?.to_string();
            self.write(to, &data, false)
        }
    }

    pub fn rename(&mut self, from: &str, to: &str) -> Result<(), FsError> {
        if to == from {
            return Ok(());
        }
        if to.starts_with(&alloc::format!("{from}/")) {
            return Err(FsError::Invalid);
        }
        self.copy(from, to)?;
        self.remove(from, true)
    }

    /// Serialise the whole tree (used to persist it on disk).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out: Vec<u8> = Vec::new();
        out.extend_from_slice(b"AVF1");
        out.extend_from_slice(&(self.map.len() as u32).to_le_bytes());
        for (path, n) in &self.map {
            out.extend_from_slice(&(path.len() as u16).to_le_bytes());
            out.extend_from_slice(path.as_bytes());
            let (kind, data): (u8, &[u8]) = match &n.kind {
                Kind::Dir => (0, &[]),
                Kind::File(s) => (1, s.as_bytes()),
            };
            out.push(kind);
            out.extend_from_slice(&n.stamp.year.to_le_bytes());
            out.extend_from_slice(&[n.stamp.month, n.stamp.day, n.stamp.hour, n.stamp.minute, n.stamp.second]);
            out.extend_from_slice(&(data.len() as u32).to_le_bytes());
            out.extend_from_slice(data);
        }
        out
    }

    /// Rebuild a tree from `to_bytes` output; `None` if the data is damaged.
    pub fn from_bytes(b: &[u8], now: Stamp) -> Option<Vfs> {
        fn take<'a>(b: &'a [u8], at: &mut usize, n: usize) -> Option<&'a [u8]> {
            let s = b.get(*at..at.checked_add(n)?)?;
            *at += n;
            Some(s)
        }
        let mut at = 0;
        if take(b, &mut at, 4)? != b"AVF1" {
            return None;
        }
        let count = u32::from_le_bytes(take(b, &mut at, 4)?.try_into().ok()?);
        let mut map = BTreeMap::new();
        for _ in 0..count {
            let pl = u16::from_le_bytes(take(b, &mut at, 2)?.try_into().ok()?) as usize;
            let path = core::str::from_utf8(take(b, &mut at, pl)?).ok()?.to_string();
            let kind = take(b, &mut at, 1)?[0];
            let year = u16::from_le_bytes(take(b, &mut at, 2)?.try_into().ok()?);
            let t = take(b, &mut at, 5)?;
            let stamp = Stamp { year, month: t[0], day: t[1], hour: t[2], minute: t[3], second: t[4] };
            let dl = u32::from_le_bytes(take(b, &mut at, 4)?.try_into().ok()?) as usize;
            let data = take(b, &mut at, dl)?;
            let kind = if kind == 0 { Kind::Dir } else { Kind::File(core::str::from_utf8(data).ok()?.to_string()) };
            map.insert(path, Node { kind, stamp });
        }
        if !map.contains_key("/") {
            return None;
        }
        Some(Vfs { map, now })
    }

    pub fn total_files(&self) -> usize {
        self.map.values().filter(|n| matches!(n.kind, Kind::File(_))).count()
    }
}
