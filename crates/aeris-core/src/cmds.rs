//! Built-in commands for the three dialects.

use crate::shell::{Args, Flavor, Output, Session, DAYS, MONTHS};
use crate::vfs::{Entry, FsError, Stamp, Vfs, HOME};
use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

const CMD_NAMES: &[&str] = &[
    "cd", "cls", "color", "copy", "date", "del", "dir", "echo", "exit", "find", "findstr", "help", "hostname", "md", "mkdir", "more", "move",
    "powershell", "pwsh", "rd", "ren", "rename", "rmdir", "set", "sort", "systeminfo", "tasklist", "time", "title", "tree", "type", "ver",
    "vol", "where", "whoami",
];
const PS_NAMES: &[&str] = &[
    "Add-Content", "cat", "cd", "Clear-Host", "cls", "cmd", "Copy-Item", "dir", "Get-ChildItem", "Get-Command", "Get-Content", "Get-Date",
    "Get-Help", "Get-History", "Get-Location", "Get-Process", "hostname", "ls", "Measure-Object", "mkdir", "Move-Item", "New-Item", "pwd",
    "Remove-Item", "Rename-Item", "Select-Object", "Select-String", "Set-Content", "Set-Location", "Sort-Object", "Test-Path", "tree",
    "whoami", "Write-Host", "Write-Output",
];
const MAC_NAMES: &[&str] = &[
    "cat", "cd", "clear", "cp", "date", "echo", "env", "export", "grep", "head", "help", "history", "hostname", "ls", "man", "mkdir",
    "mv", "neofetch", "aerisfetch", "pwd", "pwsh", "ps", "rm", "rmdir", "sort", "sw_vers", "tail", "touch", "tree", "uname", "uptime",
    "wc", "which", "whoami",
];

pub fn command_names(f: Flavor) -> &'static [&'static str] {
    match f {
        Flavor::Cmd => CMD_NAMES,
        Flavor::PowerShell => PS_NAMES,
        Flavor::Mac => MAC_NAMES,
    }
}

fn ok(s: impl Into<String>) -> Output {
    Output { out: s.into(), ..Output::default() }
}
fn fail(s: impl Into<String>) -> Output {
    Output { err: s.into(), ..Output::default() }
}

/// Map a typed command name to a canonical operation.
fn canon(f: Flavor, name: &str) -> Option<&'static str> {
    let n = name.to_lowercase();
    let n = n.as_str();
    Some(match f {
        Flavor::Cmd => match n {
            "dir" => "ls",
            "cd" | "chdir" => "cd",
            "type" | "more" => "cat",
            "md" | "mkdir" => "mkdir",
            "rd" | "rmdir" => "rmdir",
            "del" | "erase" => "rm",
            "copy" | "xcopy" => "cp",
            "move" | "ren" | "rename" => "mv",
            "echo" => "echo",
            "cls" => "clear",
            "ver" => "ver",
            "set" => "set",
            "date" => "date",
            "time" => "time",
            "hostname" => "hostname",
            "whoami" => "whoami",
            "tree" => "tree",
            "help" => "help",
            "exit" => "exit",
            "powershell" | "pwsh" => "to_ps",
            "find" | "findstr" => "grep",
            "sort" => "sort",
            "color" | "title" | "prompt" | "pause" | "rem" | "mode" | "chcp" => "nop",
            "vol" => "vol",
            "tasklist" => "ps",
            "systeminfo" => "fetch",
            "where" => "which",
            _ => return None,
        },
        Flavor::PowerShell => match n {
            "ls" | "dir" | "gci" | "get-childitem" => "ls",
            "cd" | "sl" | "set-location" | "chdir" => "cd",
            "pwd" | "gl" | "get-location" => "pwd",
            "cat" | "type" | "gc" | "get-content" | "more" => "cat",
            "mkdir" | "md" => "mkdir",
            "ni" | "new-item" => "ni",
            "rm" | "del" | "ri" | "rmdir" | "rd" | "erase" | "remove-item" => "rm",
            "cp" | "copy" | "cpi" | "copy-item" => "cp",
            "mv" | "move" | "mi" | "move-item" | "ren" | "rni" | "rename-item" => "mv",
            "echo" | "write" | "write-output" | "write-host" => "echo",
            "cls" | "clear" | "clear-host" => "clear",
            "get-date" => "date",
            "hostname" => "hostname",
            "whoami" => "whoami",
            "get-process" | "ps" | "gps" => "ps",
            "set-content" | "sc" => "setcontent",
            "add-content" | "ac" => "addcontent",
            "select-string" | "sls" | "findstr" => "grep",
            "measure-object" | "measure" => "measure",
            "sort-object" | "sort" => "sort",
            "select-object" | "select" => "select",
            "test-path" => "testpath",
            "get-help" | "help" | "man" => "help",
            "get-history" | "history" | "h" => "history",
            "get-command" | "gcm" => "which",
            "tree" => "tree",
            "exit" => "exit",
            "cmd" => "to_cmd",
            "powershell" | "pwsh" => "nop",
            "get-computerinfo" | "systeminfo" | "neofetch" => "fetch",
            "start-sleep" | "sleep" | "color" | "set-executionpolicy" => "nop",
            "$psversiontable" => "psver",
            "ver" => "ver",
            _ => return None,
        },
        Flavor::Mac => match n {
            "ls" | "ll" => "ls",
            "cd" => "cd",
            "pwd" => "pwd",
            "cat" | "more" | "less" => "cat",
            "mkdir" => "mkdir",
            "rm" => "rm",
            "rmdir" => "rmdir",
            "touch" => "touch",
            "cp" => "cp",
            "mv" => "mv",
            "echo" | "printf" => "echo",
            "clear" => "clear",
            "exit" | "logout" => "exit",
            "date" => "date",
            "whoami" => "whoami",
            "hostname" => "hostname",
            "uname" => "uname",
            "sw_vers" => "swvers",
            "neofetch" | "aerisfetch" | "fastfetch" => "fetch",
            "ps" | "top" => "ps",
            "env" | "printenv" | "export" => "env",
            "grep" | "egrep" => "grep",
            "sort" => "sort",
            "wc" => "wc",
            "head" => "head",
            "tail" => "tail",
            "tree" => "tree",
            "which" | "type" => "which",
            "help" => "help",
            "man" => "man",
            "history" => "history",
            "uptime" => "uptime",
            "pwsh" | "powershell" => "to_ps",
            "zsh" | "bash" | "sh" => "nop",
            "sleep" | "open" | "say" | "caffeinate" => "nop",
            _ => return None,
        },
    })
}

fn fmt_commas(mut n: u64) -> String {
    if n == 0 {
        return "0".to_string();
    }
    let mut parts: Vec<String> = Vec::new();
    while n > 0 {
        let r = n % 1000;
        n /= 1000;
        parts.push(if n > 0 { format!("{r:03}") } else { format!("{r}") });
    }
    parts.reverse();
    parts.join(",")
}

fn h12(h: u8) -> (u8, &'static str) {
    let ap = if h >= 12 { "PM" } else { "AM" };
    let x = h % 12;
    (if x == 0 { 12 } else { x }, ap)
}

fn sort_entries(v: &mut [Entry], dirs_first: bool) {
    v.sort_by(|a, b| {
        if dirs_first && a.is_dir != b.is_dir {
            return b.is_dir.cmp(&a.is_dir);
        }
        a.name.to_lowercase().cmp(&b.name.to_lowercase())
    });
}

impl Session {
    pub(crate) fn fs_msg(&self, op: &str, raw: &str, e: FsError) -> String {
        let abs = Vfs::resolve(&self.cwd, raw);
        match self.flavor {
            Flavor::Cmd => match e {
                FsError::NotFound if op == "cd" => "The system cannot find the path specified.".into(),
                FsError::NotFound => "The system cannot find the file specified.".into(),
                FsError::Exists => format!("A subdirectory or file {raw} already exists."),
                FsError::NotEmpty => "The directory is not empty.".into(),
                FsError::NotDir => "The directory name is invalid.".into(),
                FsError::IsDir | FsError::Invalid => "Access is denied.".into(),
            },
            Flavor::PowerShell => {
                let cmdlet = match op {
                    "ls" => "Get-ChildItem",
                    "cd" => "Set-Location",
                    "cat" => "Get-Content",
                    "mkdir" | "ni" => "New-Item",
                    "rm" | "rmdir" => "Remove-Item",
                    "cp" => "Copy-Item",
                    "mv" => "Move-Item",
                    "setcontent" => "Set-Content",
                    "addcontent" => "Add-Content",
                    _ => "Out-File",
                };
                let w = Self::win_path(&abs);
                let msg = match e {
                    FsError::NotFound => format!("Cannot find path '{w}' because it does not exist."),
                    FsError::Exists => format!("An item with the specified name {w} already exists."),
                    FsError::NotEmpty => format!("The item at {w} has children and the Recurse parameter was not specified."),
                    FsError::NotDir => format!("Cannot find path '{w}' because it is not a directory."),
                    FsError::IsDir => format!("Access to the path '{w}' is denied."),
                    FsError::Invalid => format!("Cannot remove '{w}'."),
                };
                format!("{cmdlet} : {msg}")
            }
            Flavor::Mac => {
                let why = match e {
                    FsError::NotFound => "No such file or directory",
                    FsError::Exists => "File exists",
                    FsError::NotEmpty => "Directory not empty",
                    FsError::NotDir => "Not a directory",
                    FsError::IsDir => "is a directory",
                    FsError::Invalid => "Operation not permitted",
                };
                format!("{op}: {raw}: {why}")
            }
        }
    }

    fn path_arg(&self, raw: &str) -> String {
        Vfs::resolve(&self.cwd, raw)
    }

    pub(crate) fn run(&mut self, name: &str, a: &Args, stdin: Option<&str>) -> Output {
        let Some(op) = canon(self.flavor, name) else {
            return self.not_found(name);
        };
        match op {
            "nop" => Output::default(),
            "clear" => Output { clear: true, ..Output::default() },
            "exit" => {
                if let Some(prev) = self.parents.pop() {
                    self.flavor = prev;
                    let mut o = ok(self.banner());
                    o.switched = true;
                    return o;
                }
                Output { exit: true, ..Output::default() }
            }
            "to_ps" => self.switch(Flavor::PowerShell),
            "to_cmd" => self.switch(Flavor::Cmd),
            "echo" => self.op_echo(a),
            "pwd" => {
                let p = self.show(&self.cwd);
                if self.flavor == Flavor::PowerShell {
                    ok(format!("\nPath\n----\n{p}\n\n"))
                } else {
                    ok(p)
                }
            }
            "cd" => self.op_cd(a),
            "ls" => self.op_ls(a),
            "cat" => self.op_cat(a, stdin),
            "mkdir" => self.op_mkdir(a),
            "ni" => self.op_ni(a),
            "touch" => {
                let mut o = Output::default();
                for p in &a.pos {
                    let abs = self.path_arg(p);
                    let r = if self.vfs.is_file(&abs) {
                        let c = self.vfs.read(&abs).unwrap_or("").to_string();
                        self.vfs.write(&abs, &c, false)
                    } else {
                        self.vfs.write(&abs, "", false)
                    };
                    if let Err(e) = r {
                        o.err.push_str(&format!("{}\n", self.fs_msg("touch", p, e)));
                    }
                }
                o
            }
            "rm" => self.op_rm(a),
            "rmdir" => self.op_rmdir(a),
            "cp" | "mv" => self.op_cp_mv(op, a),
            "setcontent" | "addcontent" => {
                let path = a.named("path").map(|s| s.to_string()).or_else(|| a.pos.first().cloned());
                let val = a.named("value").map(|s| s.to_string()).or_else(|| a.pos.get(1).cloned()).or_else(|| stdin.map(|s| s.to_string()));
                match (path, val) {
                    (Some(p), Some(v)) => {
                        let abs = self.path_arg(&p);
                        match self.vfs.write(&abs, &format!("{v}\n"), op == "addcontent") {
                            Ok(()) => Output::default(),
                            Err(e) => fail(self.fs_msg(op, &p, e)),
                        }
                    }
                    _ => fail("Set-Content : Missing -Path or -Value."),
                }
            }
            "testpath" => {
                let p = a.named("path").map(|s| s.to_string()).or_else(|| a.pos.first().cloned()).unwrap_or_default();
                ok(if self.vfs.exists(&self.path_arg(&p)) { "True" } else { "False" })
            }
            "date" => self.op_date(a),
            "time" => {
                let t = self.vfs.now;
                let (h, ap) = h12(t.hour);
                ok(format!("The current time is: {:>2}:{:02}:{:02}.00 {}", h, t.minute, t.second, ap).replace(" AM", "").replace(" PM", ""))
            }
            "whoami" => ok(match self.flavor {
                Flavor::Mac => "aeris".to_string(),
                _ => format!("{}\\aeris", self.hostname().to_lowercase()),
            }),
            "hostname" => ok(self.hostname()),
            "ver" => ok("\nAeris OS [Version 1.0.26100.1]\n"),
            "vol" => ok(" Volume in drive C is Aeris\n Volume Serial Number is AE12-2026\n"),
            "uname" => {
                if a.flag("a") || a.flag("all") {
                    let t = self.vfs.now;
                    ok(format!(
                        "Aeris {} 27.0.0 Aeris Kernel Version 27.0.0: {} {} {:>2} {:02}:{:02}:{:02} UTC {}; aeris:kernel-rs/RELEASE x86_64",
                        self.hostname(),
                        &DAYS[t.weekday() % 7][..3],
                        &MONTHS[(t.month.clamp(1, 12) - 1) as usize][..3],
                        t.day,
                        t.hour,
                        t.minute,
                        t.second,
                        t.year
                    ))
                } else {
                    ok("Aeris")
                }
            }
            "swvers" => ok("ProductName:\t\tAeris\nProductVersion:\t\t27.0\nBuildVersion:\t\t27A1001"),
            "uptime" => {
                let s = self.sys.uptime_s;
                ok(format!("up {} min, {} sec, 1 user", s / 60, s % 60))
            }
            "psver" => ok(PSVER.to_string()),
            "set" | "env" => self.op_env(a),
            "tree" => self.op_tree(a),
            "help" => ok(self.help_text()),
            "man" => match a.pos.first() {
                Some(p) => fail(format!("No manual entry for {p}")),
                None => fail("What manual page do you want?"),
            },
            "history" => {
                let mut s = String::new();
                for (i, h) in self.history.iter().enumerate() {
                    s.push_str(&format!("{:>5}  {}\n", i + 1, h));
                }
                ok(s)
            }
            "which" => {
                let mut s = String::new();
                let mut e = String::new();
                for p in &a.pos {
                    if canon(self.flavor, p).is_some() {
                        match self.flavor {
                            Flavor::Mac => s.push_str(&format!("{p}: aeris shell built-in command\n")),
                            Flavor::Cmd => s.push_str(&format!("C:\\System\\{p}.exe\n")),
                            Flavor::PowerShell => s.push_str(&format!("\nCommandType     Name\n-----------     ----\nCmdlet          {p}\n")),
                        }
                    } else {
                        e.push_str(&format!("{p} not found\n"));
                    }
                }
                Output { out: s, err: e, ..Output::default() }
            }
            "fetch" => ok(self.fetch()),
            "ps" => ok(self.processes()),
            "grep" => self.op_grep(a, stdin),
            "sort" => {
                let text = self.input_text(a, stdin);
                let mut lines: Vec<&str> = text.lines().collect();
                lines.sort_by(|x, y| x.to_lowercase().cmp(&y.to_lowercase()));
                if a.flag("r") || a.flag("descending") || a.flag("reverse") {
                    lines.reverse();
                }
                ok(lines.join("\n"))
            }
            "wc" | "measure" => {
                let text = self.input_text(a, stdin);
                let (l, w, b) = (text.lines().count(), text.split_whitespace().count(), text.len());
                if op == "measure" {
                    ok(format!("\nCount    : {l}\nAverage  :\nSum      :\nMaximum  :\nMinimum  :\nProperty :\n"))
                } else if a.flag("l") {
                    ok(format!("{l:>8}"))
                } else {
                    ok(format!("{l:>8}{w:>8}{b:>8}"))
                }
            }
            "head" | "tail" | "select" => self.op_head_tail(op, a, stdin),
            _ => self.not_found(name),
        }
    }

    fn switch(&mut self, to: Flavor) -> Output {
        if self.flavor == to {
            return Output::default();
        }
        self.parents.push(self.flavor);
        self.flavor = to;
        let mut o = ok(self.banner());
        o.switched = true;
        o
    }

    fn not_found(&self, name: &str) -> Output {
        fail(match self.flavor {
            Flavor::Cmd => format!("'{name}' is not recognized as an internal or external command,\noperable program or batch file."),
            Flavor::PowerShell => format!(
                "{name} : The term '{name}' is not recognized as the name of a cmdlet, function, script file, or operable program.\nCheck the spelling of the name, or if a path was included, verify that the path is correct and try again.\nAt line:1 char:1\n+ {name}\n+ {}\n    + CategoryInfo          : ObjectNotFound: ({name}:String) [], CommandNotFoundException\n    + FullyQualifiedErrorId : CommandNotFoundException",
                "~".repeat(name.chars().count().max(1))
            ),
            Flavor::Mac => format!("zsh: command not found: {name}"),
        })
    }

    fn input_text(&self, a: &Args, stdin: Option<&str>) -> String {
        if let Some(s) = stdin {
            return s.to_string();
        }
        let mut t = String::new();
        for p in &a.pos {
            if let Ok(c) = self.vfs.read(&self.path_arg(p)) {
                t.push_str(c);
            }
        }
        t
    }

    fn op_echo(&self, a: &Args) -> Output {
        if self.flavor == Flavor::Cmd && a.pos.is_empty() && a.flags.is_empty() {
            return ok(if self.cmd_echo { "ECHO is on." } else { "ECHO is off." });
        }
        let mut parts: Vec<String> = a.pos.clone();
        if self.flavor != Flavor::Cmd {
            for f in &a.flags {
                if self.flavor == Flavor::Mac && f == "n" {
                    continue;
                }
            }
        } else {
            // cmd treats `/x` tokens as text for echo
            let mut all: Vec<String> = a.flags.iter().map(|f| format!("/{f}")).collect();
            all.extend(parts);
            parts = all;
        }
        let s = parts.join(" ");
        if self.flavor == Flavor::Cmd && s.starts_with('.') {
            return ok(s[1..].trim_start().to_string());
        }
        ok(s)
    }

    fn op_cd(&mut self, a: &Args) -> Output {
        let target = a.pos.first().cloned();
        let t = match target {
            None => {
                return match self.flavor {
                    Flavor::Cmd => ok(Self::win_path(&self.cwd)),
                    _ => {
                        self.cwd = HOME.to_string();
                        Output::default()
                    }
                }
            }
            Some(t) => t,
        };
        // `cd /d X` handled: flags eats "/d".
        let abs = self.path_arg(&t);
        if self.vfs.is_dir(&abs) {
            self.cwd = abs;
            Output::default()
        } else if self.vfs.exists(&abs) {
            fail(self.fs_msg("cd", &t, FsError::NotDir))
        } else {
            fail(self.fs_msg("cd", &t, FsError::NotFound))
        }
    }

    fn op_ls(&self, a: &Args) -> Output {
        let mut targets: Vec<String> = a.pos.clone();
        if let Some(p) = a.named("path") {
            targets.insert(0, p.to_string());
        }
        if targets.is_empty() {
            targets.push(".".to_string());
        }
        let mut out = Output::default();
        let multi = targets.len() > 1;
        for (i, raw) in targets.iter().enumerate() {
            let abs = self.path_arg(raw);
            let Some(ent) = self.vfs.entry(&abs) else {
                let msg = match self.flavor {
                    Flavor::Cmd => "File Not Found".to_string(),
                    _ => self.fs_msg("ls", raw, FsError::NotFound),
                };
                out.err.push_str(&format!("{msg}\n"));
                continue;
            };
            let (dir, mut entries) = if ent.is_dir {
                (abs.clone(), self.vfs.list(&abs).unwrap_or_default())
            } else {
                (Vfs::parent(&abs).to_string(), alloc::vec![ent.clone()])
            };
            let all = a.flag("all") || a.flag("force") || a.flag("a");
            let _ = all;
            match self.flavor {
                Flavor::Mac => {
                    if !(a.flag("a") || a.flag("all")) {
                        entries.retain(|e| !e.name.starts_with('.'));
                    }
                    sort_entries(&mut entries, false);
                    if multi {
                        if i > 0 {
                            out.out.push('\n');
                        }
                        out.out.push_str(&format!("{raw}:\n"));
                    }
                    out.out.push_str(&self.mac_ls(&entries, a.flag("l") || a.flag("1")));
                }
                Flavor::PowerShell => {
                    sort_entries(&mut entries, true);
                    if a.flag("name") {
                        for e in &entries {
                            out.out.push_str(&format!("{}\n", e.name));
                        }
                    } else if entries.is_empty() {
                        // PowerShell prints nothing for an empty directory.
                    } else {
                        out.out.push_str(&self.ps_table(&dir, &entries));
                    }
                }
                Flavor::Cmd => {
                    sort_entries(&mut entries, true);
                    out.out.push_str(&self.cmd_dir(&dir, &entries, ent.is_dir, a.flag("b") || a.flag("w")));
                }
            }
        }
        out
    }

    fn mac_ls(&self, entries: &[Entry], long: bool) -> String {
        if long {
            let mut s = format!("total {}\n", entries.len() * 8);
            for e in entries {
                let t = e.stamp;
                s.push_str(&format!(
                    "{} {:>2} aeris  staff {:>6} {} {:>2} {:02}:{:02} {}\n",
                    if e.is_dir { "drwxr-xr-x" } else { "-rw-r--r--" },
                    if e.is_dir { 3 } else { 1 },
                    if e.is_dir { 96 } else { e.size },
                    &MONTHS[(t.month.clamp(1, 12) - 1) as usize][..3],
                    t.day,
                    t.hour,
                    t.minute,
                    e.name
                ));
            }
            return s;
        }
        if entries.is_empty() {
            return String::new();
        }
        let w = entries.iter().map(|e| e.name.chars().count()).max().unwrap_or(1) + 2;
        let per_row = (self.cols / w).max(1);
        let rows = entries.len().div_ceil(per_row);
        let mut s = String::new();
        for r in 0..rows {
            let mut line = String::new();
            for c in 0..per_row {
                if let Some(e) = entries.get(c * rows + r) {
                    line.push_str(&e.name);
                    let pad = w - e.name.chars().count();
                    if c * rows + r + rows < entries.len() {
                        line.push_str(&" ".repeat(pad));
                    }
                }
            }
            s.push_str(line.trim_end());
            s.push('\n');
        }
        s
    }

    fn ps_table(&self, dir: &str, entries: &[Entry]) -> String {
        let mut s = format!("\n    Directory: {}\n\n\n", Self::win_path(dir));
        s.push_str("Mode                 LastWriteTime         Length Name\n");
        s.push_str("----                 -------------         ------ ----\n");
        for e in entries {
            let t = e.stamp;
            let (h, ap) = h12(t.hour);
            let dt = format!("{}/{}/{}  {:>2}:{:02} {}", t.month, t.day, t.year, h, t.minute, ap);
            let len = if e.is_dir { String::new() } else { e.size.to_string() };
            s.push_str(&format!("{:<7}{:>27}{:>15} {}\n", if e.is_dir { "d-----" } else { "-a----" }, dt, len, e.name));
        }
        s.push('\n');
        s
    }

    fn cmd_dir(&self, dir: &str, entries: &[Entry], is_dir: bool, bare: bool) -> String {
        if bare {
            let mut s = String::new();
            for e in entries {
                s.push_str(&format!("{}\n", e.name));
            }
            return s;
        }
        let mut s = String::from(" Volume in drive C is Aeris\n Volume Serial Number is AE12-2026\n\n");
        s.push_str(&format!(" Directory of {}\n\n", Self::win_path(dir)));
        let mut rows: Vec<Entry> = Vec::new();
        if is_dir {
            for dots in [".", ".."] {
                if dots == ".." && dir == "/" {
                    continue;
                }
                rows.push(Entry { name: dots.to_string(), is_dir: true, size: 0, stamp: self.vfs.now });
            }
        }
        rows.extend_from_slice(entries);
        let (mut files, mut dirs, mut bytes) = (0u64, 0u64, 0u64);
        for e in &rows {
            let t = e.stamp;
            let (h, ap) = h12(t.hour);
            let date = format!("{:02}/{:02}/{}  {:02}:{:02} {}", t.month, t.day, t.year, h, t.minute, ap);
            if e.is_dir {
                dirs += 1;
                s.push_str(&format!("{date}    <DIR>          {}\n", e.name));
            } else {
                files += 1;
                bytes += e.size as u64;
                s.push_str(&format!("{date}    {:>14} {}\n", fmt_commas(e.size as u64), e.name));
            }
        }
        s.push_str(&format!("{:>16} File(s) {:>14} bytes\n", files, fmt_commas(bytes)));
        s.push_str(&format!("{:>16} Dir(s)  {:>14} bytes free\n", dirs, fmt_commas(118_432_100_352)));
        s
    }

    fn op_cat(&self, a: &Args, stdin: Option<&str>) -> Output {
        let mut files: Vec<String> = a.pos.clone();
        if let Some(p) = a.named("path") {
            files.insert(0, p.to_string());
        }
        if files.is_empty() {
            return match stdin {
                Some(s) => ok(s),
                None => match self.flavor {
                    Flavor::Cmd => fail("The syntax of the command is incorrect."),
                    Flavor::PowerShell => fail("Get-Content : Cannot bind argument to parameter 'Path' because it is null."),
                    Flavor::Mac => Output::default(),
                },
            };
        }
        let mut o = Output::default();
        for f in &files {
            let abs = self.path_arg(f);
            match self.vfs.read(&abs) {
                Ok(c) => o.out.push_str(c),
                Err(e) => o.err.push_str(&format!("{}\n", self.fs_msg("cat", f, e))),
            }
        }
        o
    }

    fn op_mkdir(&mut self, a: &Args) -> Output {
        let mut targets: Vec<String> = a.pos.clone();
        if let Some(p) = a.named("path") {
            targets.insert(0, p.to_string());
        }
        if targets.is_empty() {
            return fail(match self.flavor {
                Flavor::Mac => "usage: mkdir [-pv] directory ...".to_string(),
                _ => "The syntax of the command is incorrect.".to_string(),
            });
        }
        let parents = a.flag("p") || self.flavor != Flavor::Mac;
        let mut o = Output::default();
        for t in &targets {
            let abs = self.path_arg(t);
            match self.vfs.mkdir(&abs, parents) {
                Ok(()) => {
                    if self.flavor == Flavor::PowerShell {
                        if let Some(e) = self.vfs.entry(&abs) {
                            o.out.push_str(&self.ps_table(Vfs::parent(&abs), &[e]));
                        }
                    }
                }
                Err(e) => o.err.push_str(&format!("{}\n", self.fs_msg("mkdir", t, e))),
            }
        }
        o
    }

    fn op_ni(&mut self, a: &Args) -> Output {
        let mut path = a.named("path").map(|s| s.to_string()).or_else(|| a.pos.first().cloned());
        if let (Some(p), Some(n)) = (path.clone(), a.named("name")) {
            path = Some(format!("{}/{}", p.trim_end_matches(['/', '\\']), n));
        } else if path.is_none() {
            path = a.named("name").map(|s| s.to_string());
        }
        let Some(p) = path else {
            return fail("New-Item : Missing -Path.");
        };
        let abs = self.path_arg(&p);
        let is_dir = a.named("itemtype").map_or(false, |t| t.to_lowercase().starts_with("dir") || t.to_lowercase() == "directory");
        if self.vfs.exists(&abs) {
            return fail(self.fs_msg("ni", &p, FsError::Exists));
        }
        let r = if is_dir {
            self.vfs.mkdir(&abs, true)
        } else {
            let v = a.named("value").unwrap_or("").to_string();
            self.vfs.write(&abs, &v, false)
        };
        match r {
            Ok(()) => ok(self.vfs.entry(&abs).map(|e| self.ps_table(Vfs::parent(&abs), &[e])).unwrap_or_default()),
            Err(e) => fail(self.fs_msg("ni", &p, e)),
        }
    }

    fn op_rm(&mut self, a: &Args) -> Output {
        let mut targets: Vec<String> = a.pos.clone();
        if let Some(p) = a.named("path") {
            targets.insert(0, p.to_string());
        }
        if targets.is_empty() {
            return fail(match self.flavor {
                Flavor::Mac => "usage: rm [-f | -i] [-dPRrvW] file ...".to_string(),
                _ => "The syntax of the command is incorrect.".to_string(),
            });
        }
        let recursive = a.flag("recurse") || a.flag("r") || a.flag("s");
        let force = a.flag("force") || a.flag("f") || a.flag("q");
        let mut o = Output::default();
        for t in &targets {
            let abs = self.path_arg(t);
            if self.flavor == Flavor::Cmd && self.vfs.is_dir(&abs) {
                // `del dir` removes the files within, not the directory.
                if let Ok(list) = self.vfs.list(&abs) {
                    for e in list.into_iter().filter(|e| !e.is_dir) {
                        let _ = self.vfs.remove(&format!("{}/{}", abs.trim_end_matches('/'), e.name), false);
                    }
                }
                continue;
            }
            if self.flavor == Flavor::Mac && self.vfs.is_dir(&abs) && !recursive {
                o.err.push_str(&format!("rm: {t}: is a directory\n"));
                continue;
            }
            match self.vfs.remove(&abs, recursive) {
                Ok(()) => {}
                Err(FsError::NotFound) if force => {}
                Err(e) => o.err.push_str(&format!("{}\n", self.fs_msg("rm", t, e))),
            }
        }
        o
    }

    fn op_rmdir(&mut self, a: &Args) -> Output {
        let mut o = Output::default();
        if a.pos.is_empty() {
            return fail("The syntax of the command is incorrect.");
        }
        let recursive = a.flag("s");
        for t in &a.pos {
            let abs = self.path_arg(t);
            if self.vfs.exists(&abs) && !self.vfs.is_dir(&abs) {
                o.err.push_str(&format!("{}\n", self.fs_msg("rmdir", t, FsError::NotDir)));
                continue;
            }
            if let Err(e) = self.vfs.remove(&abs, recursive) {
                o.err.push_str(&format!("{}\n", self.fs_msg("rmdir", t, e)));
            }
        }
        o
    }

    fn op_cp_mv(&mut self, op: &str, a: &Args) -> Output {
        let (src, dst) = match (a.named("path").map(|s| s.to_string()).or_else(|| a.pos.first().cloned()), a.named("destination").map(|s| s.to_string()).or_else(|| {
            if a.named("path").is_some() { a.pos.first().cloned() } else { a.pos.get(1).cloned() }
        })) {
            (Some(s), Some(d)) => (s, d),
            _ => {
                return fail(match self.flavor {
                    Flavor::Mac => format!("usage: {op} source target"),
                    _ => "The syntax of the command is incorrect.".to_string(),
                })
            }
        };
        let s_abs = self.path_arg(&src);
        let mut d_abs = self.path_arg(&dst);
        if self.vfs.is_dir(&d_abs) {
            d_abs = format!("{}/{}", d_abs.trim_end_matches('/'), Vfs::basename(&s_abs));
        }
        let r = if op == "cp" { self.vfs.copy(&s_abs, &d_abs) } else { self.vfs.rename(&s_abs, &d_abs) };
        match r {
            Ok(()) => {
                if self.flavor == Flavor::Cmd {
                    ok(if op == "cp" { "        1 file(s) copied." } else { "        1 file(s) moved." })
                } else {
                    Output::default()
                }
            }
            Err(e) => fail(self.fs_msg(op, &src, e)),
        }
    }

    fn op_date(&self, a: &Args) -> Output {
        let t: Stamp = self.vfs.now;
        let wd = DAYS[t.weekday() % 7];
        let mon = MONTHS[(t.month.clamp(1, 12) - 1) as usize];
        let (h, ap) = h12(t.hour);
        match self.flavor {
            Flavor::Cmd => {
                if a.flag("t") {
                    ok(format!("{} {:02}/{:02}/{}", &wd[..3], t.month, t.day, t.year))
                } else {
                    ok(format!("The current date is: {} {:02}/{:02}/{}", &wd[..3], t.month, t.day, t.year))
                }
            }
            Flavor::PowerShell => ok(format!("{wd}, {mon} {}, {} {}:{:02}:{:02} {}", t.day, t.year, h, t.minute, t.second, ap)),
            Flavor::Mac => ok(format!(
                "{} {} {:>2} {:02}:{:02}:{:02} UTC {}",
                &wd[..3],
                &mon[..3],
                t.day,
                t.hour,
                t.minute,
                t.second,
                t.year
            )),
        }
    }

    fn op_env(&mut self, a: &Args) -> Output {
        // `set A=B` in cmd.
        if self.flavor == Flavor::Cmd {
            if let Some(first) = a.pos.first() {
                if let Some((k, v)) = first.split_once('=') {
                    self.vars.insert(k.to_string(), v.to_string());
                    return Output::default();
                }
                return match self.get_var(first) {
                    Some(v) => ok(format!("{first}={v}")),
                    None => fail(format!("Environment variable {first} not defined")),
                };
            }
        }
        let mut s = String::new();
        for (k, v) in &self.vars {
            s.push_str(&format!("{k}={v}\n"));
        }
        ok(s)
    }

    fn op_tree(&self, a: &Args) -> Output {
        let raw = a.pos.first().cloned().unwrap_or_else(|| ".".to_string());
        let abs = self.path_arg(&raw);
        if !self.vfs.is_dir(&abs) {
            return fail(self.fs_msg("tree", &raw, FsError::NotFound));
        }
        let mut s = String::new();
        match self.flavor {
            Flavor::Mac => s.push_str(&format!("{}\n", if raw == "." { "." } else { &raw })),
            _ => s.push_str(&format!("Folder PATH listing\nVolume serial number is AE12-2026\n{}\n", Self::win_path(&abs))),
        }
        self.tree_rec(&abs, "", &mut s);
        ok(s)
    }

    fn tree_rec(&self, dir: &str, prefix: &str, out: &mut String) {
        let Ok(mut list) = self.vfs.list(dir) else { return };
        sort_entries(&mut list, true);
        let n = list.len();
        for (i, e) in list.iter().enumerate() {
            let last = i + 1 == n;
            let branch = if self.flavor == Flavor::Mac {
                if last { "`-- " } else { "|-- " }
            } else if last {
                "\\---"
            } else {
                "+---"
            };
            out.push_str(&format!("{prefix}{branch}{}\n", e.name));
            if e.is_dir {
                let child = format!("{}/{}", dir.trim_end_matches('/'), e.name);
                let pad = if self.flavor == Flavor::Mac {
                    if last { "    " } else { "|   " }
                } else if last {
                    "    "
                } else {
                    "|   "
                };
                self.tree_rec(&child, &format!("{prefix}{pad}"), out);
            }
        }
    }

    fn op_grep(&self, a: &Args, stdin: Option<&str>) -> Output {
        let pattern = a.named("pattern").map(|s| s.to_string()).or_else(|| a.pos.first().cloned());
        let Some(pat) = pattern else {
            return fail(match self.flavor {
                Flavor::Cmd => "FINDSTR: Bad command line".to_string(),
                _ => "usage: grep pattern [file ...]".to_string(),
            });
        };
        let skip = if a.named("pattern").is_some() { 0 } else { 1 };
        let mut files: Vec<String> = a.pos.iter().skip(skip).cloned().collect();
        if let Some(p) = a.named("path") {
            files.push(p.to_string());
        }
        let insens = a.flag_exact("i") || self.flavor == Flavor::PowerShell;
        let pat_l = pat.to_lowercase();
        let matches = |line: &str| if insens { line.to_lowercase().contains(&pat_l) } else { line.contains(&pat) };
        let mut o = Output::default();
        if files.is_empty() {
            for l in stdin.unwrap_or("").lines() {
                if matches(l) {
                    o.out.push_str(&format!("{l}\n"));
                }
            }
        } else {
            let multi = files.len() > 1 || self.flavor == Flavor::PowerShell;
            for f in &files {
                match self.vfs.read(&self.path_arg(f)) {
                    Ok(c) => {
                        for (n, l) in c.lines().enumerate() {
                            if matches(l) {
                                if self.flavor == Flavor::PowerShell {
                                    o.out.push_str(&format!("{f}:{}:{l}\n", n + 1));
                                } else if multi {
                                    o.out.push_str(&format!("{f}:{l}\n"));
                                } else {
                                    o.out.push_str(&format!("{l}\n"));
                                }
                            }
                        }
                    }
                    Err(e) => o.err.push_str(&format!("{}\n", self.fs_msg("cat", f, e))),
                }
            }
        }
        o
    }

    fn op_head_tail(&self, op: &str, a: &Args, stdin: Option<&str>) -> Output {
        let mut n: usize = 10;
        let mut pos = a.pos.clone();
        let want_tail = op == "tail" || a.named("last").is_some();
        if let Some(v) = a.named("first").or(a.named("last")) {
            n = v.parse().unwrap_or(10);
        } else if let Some(f) = a.flags.iter().find(|f| f.chars().all(|c| c.is_ascii_digit())) {
            n = f.parse().unwrap_or(10);
        } else if a.flag_exact("n") && !pos.is_empty() {
            n = pos.remove(0).parse().unwrap_or(10);
        }
        let text = match stdin {
            Some(s) => s.to_string(),
            None => pos.iter().filter_map(|p| self.vfs.read(&self.path_arg(p)).ok().map(|s| s.to_string())).collect(),
        };
        let lines: Vec<&str> = text.lines().collect();
        let sel: Vec<&str> = if want_tail { lines[lines.len().saturating_sub(n)..].to_vec() } else { lines.into_iter().take(n).collect() };
        ok(sel.join("\n"))
    }

    fn help_text(&self) -> String {
        let names = command_names(self.flavor).join("  ");
        match self.flavor {
            Flavor::Cmd => format!("For more information on a specific command, type HELP command-name\n\n{names}\n"),
            Flavor::PowerShell => format!("\nTOPIC\n    Windows PowerShell Help System (Aeris edition)\n\nSUPPORTED COMMANDS\n    {names}\n\nTip: aliases like ls, cat, rm, cp also work.\n"),
            Flavor::Mac => format!("Aeris Terminal (zsh) built-ins:\n  {names}\n"),
        }
    }

    fn processes(&self) -> String {
        const P: [(&str, u32, u32); 8] = [
            ("System", 4, 148),
            ("aeris-kernel", 8, 2312),
            ("compositor", 112, 41_208),
            ("shell", 164, 18_512),
            ("terminal", 208, 9_644),
            ("settings", 240, 7_120),
            ("files", 276, 6_480),
            ("netd", 312, 3_028),
        ];
        let mut s = String::new();
        match self.flavor {
            Flavor::Cmd => {
                s.push_str("Image Name                     PID Session Name        Mem Usage\n========================= ======== ================ ============\n");
                for (n, pid, k) in P {
                    s.push_str(&format!("{:<25} {:>8} {:<16} {:>9} K\n", n, pid, "Console", fmt_commas(k as u64)));
                }
            }
            Flavor::PowerShell => {
                s.push_str("\nHandles  NPM(K)    PM(K)      WS(K)     CPU(s)     Id  SI ProcessName\n-------  ------    -----      -----     ------     --  -- -----------\n");
                for (n, pid, k) in P {
                    s.push_str(&format!("{:>7} {:>7} {:>8} {:>10} {:>10.2} {:>6} {:>3} {}\n", 120 + pid % 90, 8 + pid % 11, k / 2, k, pid as f32 / 40.0, pid, 1, n));
                }
            }
            Flavor::Mac => {
                s.push_str("  PID TTY           TIME CMD\n");
                for (n, pid, _) in P {
                    s.push_str(&format!("{pid:>5} ttys000    0:00.{:02} {n}\n", pid % 90));
                }
            }
        }
        s
    }

    fn fetch(&self) -> String {
        const LOGO: [&str; 11] = [
            "       .--.       ",
            "     .'    '.     ",
            "    /  .--.  \\    ",
            "   |  /    \\  |   ",
            "   |  | () |  |   ",
            "   |  \\____/  |   ",
            "    \\  '.__.'  /   ",
            "     '.      .'    ",
            "       '----'      ",
            "                  ",
            "                  ",
        ];
        let s = &self.sys;
        let up = s.uptime_s;
        let info: [String; 10] = [
            "aeris@".to_string() + &self.hostname().to_lowercase(),
            "-----------------".to_string(),
            format!("OS: Aeris OS {} x86_64", if s.mac_desktop { "27.0 (Mac style)" } else { "1.0 (Windows style)" }),
            "Kernel: aeris-kernel 0.1.0 (Rust)".to_string(),
            format!("Uptime: {} mins, {} secs", up / 60, up % 60),
            format!("Shell: {}", match self.flavor { Flavor::Cmd => "cmd", Flavor::PowerShell => "pwsh", Flavor::Mac => "zsh" }),
            format!("Resolution: {}x{}", s.width, s.height),
            format!("DE: Aeris Shell ({})", if s.mac_desktop { "Aqua-like" } else { "Fluent-like" }),
            format!("Memory: {}MiB / {}MiB", s.mem_used_mb, s.mem_total_mb),
            "Language: 100% Rust".to_string(),
        ];
        let mut out = String::new();
        for i in 0..LOGO.len().max(info.len()) {
            out.push_str(LOGO.get(i).unwrap_or(&"                  "));
            if let Some(l) = info.get(i) {
                out.push_str(l);
            }
            out.push('\n');
        }
        out
    }
}

const PSVER: &str = "\nName                           Value\n----                           -----\nPSVersion                      7.5.0-aeris\nPSEdition                      Core\nPlatform                       Aeris\nOS                             Aeris OS 1.0\nPSCompatibleVersions           {1.0, 2.0, 3.0, 4.0...}\n";
