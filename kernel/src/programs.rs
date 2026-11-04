//! Programs: installing them into the file system and running them from the shells.
//!
//! Executables live in `C:\Applications` (`/Applications`). At every boot the built-in
//! programs and everything the builder packed into the ramdisk (`programs/*.exe`) are
//! (re)installed there; the shells find them by name from the current directory or from
//! `/Applications`.

use crate::winproc;
use aeris_core::shell::Flavor;
use aeris_core::vfs::Vfs;
use aeris_core::Output;
use alloc::string::{String, ToString};

const BUILTIN: &[(&str, &[u8])] = &[("hello.exe", include_bytes!("../../crates/aeris-pe/tests/hello.exe"))];

/// Install built-ins plus the ramdisk archive ([u16 name_len][name][u32 len][bytes]...).
pub fn install(vfs: &mut Vfs, ramdisk: &[u8]) -> usize {
    let _ = vfs.mkdir("/Applications", true);
    let mut n = 0;
    for (name, bytes) in BUILTIN {
        if vfs.write_bytes(&alloc::format!("/Applications/{name}"), bytes).is_ok() {
            n += 1;
        }
    }
    let mut at = 0;
    while at + 2 <= ramdisk.len() {
        let nl = u16::from_le_bytes([ramdisk[at], ramdisk[at + 1]]) as usize;
        at += 2;
        let Some(name) = ramdisk.get(at..at + nl).and_then(|b| core::str::from_utf8(b).ok()) else { break };
        at += nl;
        let Some(len) = ramdisk.get(at..at + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize) else { break };
        at += 4;
        let Some(data) = ramdisk.get(at..at + len) else { break };
        at += len;
        if vfs.write_bytes(&alloc::format!("/Applications/{name}"), data).is_ok() {
            n += 1;
        }
    }
    n
}

fn find(vfs: &Vfs, cwd: &str, word: &str) -> Option<String> {
    let w = word.trim_matches('"');
    let mut cands = alloc::vec::Vec::new();
    if w.contains(['\\', '/']) || w.contains(':') {
        let p = Vfs::resolve(cwd, w);
        cands.push(p.clone());
        cands.push(alloc::format!("{p}.exe"));
    } else {
        for dir in [cwd, "/Applications"] {
            let d = dir.trim_end_matches('/');
            cands.push(alloc::format!("{d}/{w}"));
            cands.push(alloc::format!("{d}/{w}.exe"));
        }
    }
    cands.into_iter().find(|p| vfs.is_file(p) && vfs.read_bytes(p).map_or(false, |b| b.starts_with(b"MZ")))
}

/// If `line` starts with a native program, run it. Returns the shell output and exit code.
pub fn try_run(line: &str, flavor: Flavor, vfs: &Vfs, cwd: &str) -> Option<(Output, u32)> {
    let first = line.split_whitespace().next()?;
    let path = find(vfs, cwd, first)?;
    if flavor == Flavor::Mac {
        return Some((Output { err: alloc::format!("zsh: exec format error: {first}\n"), ..Output::default() }, 126));
    }
    let bytes = vfs.read_bytes(&path).ok()?;
    let mut o = Output::default();
    match winproc::run(bytes, line) {
        Ok(x) => {
            o.out = x.output.replace("\r\n", "\n");
            if x.timed_out {
                o.err = "The program was terminated: CPU time limit exceeded (15 s).\n".to_string();
            } else if x.faulted {
                o.err = "The program crashed (access violation, 0xC0000005).\n".to_string();
            }
            Some((o, x.code))
        }
        Err(e) => {
            o.err = alloc::format!("{first}: {e}\n");
            Some((o, 1))
        }
    }
}
