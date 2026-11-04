//! Aeris core: platform-independent OS logic (file system and shells).
//!
//! Everything here is `no_std` + `alloc` so the kernel can link it, and it is
//! unit-tested on the host with `cargo test -p aeris-core`.

#![cfg_attr(not(test), no_std)]
extern crate alloc;

pub mod cmds;
pub mod shell;
pub mod vfs;

pub use shell::{Flavor, Output, Session, SysInfo};
pub use vfs::Stamp;

#[cfg(test)]
mod tests;
