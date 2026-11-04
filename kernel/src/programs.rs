//! Built-in program registry and the glue between shells and native processes.

use crate::winproc;
use aeris_core::shell::Flavor;
use aeris_core::Output;
use alloc::string::{String, ToString};

/// Native Windows x64 programs shipped with Aeris.
const BUILTIN: &[(&str, &[u8])] = &[("hello.exe", include_bytes!("../../crates/aeris-pe/tests/hello.exe"))];

/// If `line` names a native program, run it. Returns the shell output and the exit code.
pub fn try_run(line: &str, flavor: Flavor) -> Option<(Output, u32)> {
    let first = line.split_whitespace().next()?;
    let name = first.trim_matches('"').rsplit(['\\', '/']).next()?.to_lowercase();
    let name = if name.ends_with(".exe") { name } else { alloc::format!("{name}.exe") };
    let is_exe_typed = first.to_lowercase().ends_with(".exe");
    let prog = BUILTIN.iter().find(|(n, _)| *n == name);
    if flavor == Flavor::Mac {
        if is_exe_typed {
            return Some((Output { err: alloc::format!("zsh: exec format error: {first}\n"), ..Output::default() }, 126));
        }
        return None;
    }
    let (_, bytes) = prog?;
    let mut o = Output::default();
    match winproc::run(bytes) {
        Ok(x) => {
            o.out = x.output.replace("\r\n", "\n");
            if x.faulted {
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

#[allow(dead_code)]
pub fn names() -> impl Iterator<Item = String> {
    BUILTIN.iter().map(|(n, _)| n.to_string())
}
