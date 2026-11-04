//! Persistent storage: an ATA PIO driver (primary bus, slave drive) and a tiny
//! "AERISFS1" container holding the settings plus the serialised file system.
//!
//! Layout: sector 0 = header (magic, length, checksum), sectors 1.. = payload.

use crate::sys::{inb, inw, outb, outw};
use crate::theme::Settings;
use alloc::string::String;
use alloc::vec;
use alloc::vec::Vec;

const DATA: u16 = 0x1F0;
const SECCNT: u16 = 0x1F2;
const LBA0: u16 = 0x1F3;
const LBA1: u16 = 0x1F4;
const LBA2: u16 = 0x1F5;
const DRIVE: u16 = 0x1F6;
const CMD: u16 = 0x1F7;
const MAGIC: &[u8; 8] = b"AERISFS1";
const MAX_PAYLOAD: usize = 8 * 1024 * 1024;

pub struct Disk {
    sectors: u32,
}

fn wait_ready() -> bool {
    for _ in 0..2_000_000 {
        let s = inb(CMD);
        if s & 0x80 == 0 {
            return s & 0x01 == 0;
        }
    }
    false
}

fn wait_drq() -> bool {
    for _ in 0..2_000_000 {
        let s = inb(CMD);
        if s & 0x01 != 0 || s & 0x20 != 0 {
            return false;
        }
        if s & 0x80 == 0 && s & 0x08 != 0 {
            return true;
        }
    }
    false
}

fn select(lba: u32) {
    outb(DRIVE, 0xF0 | ((lba >> 24) & 0x0F) as u8);
}

impl Disk {
    /// Probe the data disk (primary bus, slave). `None` when no such drive exists.
    pub fn probe() -> Option<Disk> {
        if inb(CMD) == 0xFF {
            return None; // floating bus: no controller
        }
        outb(DRIVE, 0xF0);
        for _ in 0..4 {
            inb(0x3F6);
        }
        outb(SECCNT, 0);
        outb(LBA0, 0);
        outb(LBA1, 0);
        outb(LBA2, 0);
        outb(CMD, 0xEC); // IDENTIFY
        if inb(CMD) == 0 {
            return None;
        }
        if !wait_ready() || !wait_drq() {
            return None;
        }
        let mut id = [0u16; 256];
        for w in id.iter_mut() {
            *w = inw(DATA);
        }
        let sectors = (id[60] as u32) | ((id[61] as u32) << 16);
        if sectors < 64 {
            return None;
        }
        Some(Disk { sectors })
    }

    pub fn sectors(&self) -> u32 {
        self.sectors
    }

    fn read_sector(&self, lba: u32, out: &mut [u8; 512]) -> bool {
        if !wait_ready() {
            return false;
        }
        select(lba);
        outb(SECCNT, 1);
        outb(LBA0, lba as u8);
        outb(LBA1, (lba >> 8) as u8);
        outb(LBA2, (lba >> 16) as u8);
        outb(CMD, 0x20);
        if !wait_drq() {
            return false;
        }
        for i in 0..256 {
            let w = inw(DATA);
            out[i * 2] = w as u8;
            out[i * 2 + 1] = (w >> 8) as u8;
        }
        true
    }

    fn write_sector(&self, lba: u32, data: &[u8; 512]) -> bool {
        if !wait_ready() {
            return false;
        }
        select(lba);
        outb(SECCNT, 1);
        outb(LBA0, lba as u8);
        outb(LBA1, (lba >> 8) as u8);
        outb(LBA2, (lba >> 16) as u8);
        outb(CMD, 0x30);
        if !wait_drq() {
            return false;
        }
        for i in 0..256 {
            outw(DATA, data[i * 2] as u16 | ((data[i * 2 + 1] as u16) << 8));
        }
        outb(CMD, 0xE7); // flush cache
        wait_ready()
    }

    /// Read the stored payload, if the container is valid.
    pub fn load(&self) -> Option<Vec<u8>> {
        let mut s = [0u8; 512];
        if !self.read_sector(0, &mut s) || &s[..8] != MAGIC {
            return None;
        }
        let len = u32::from_le_bytes(s[8..12].try_into().ok()?) as usize;
        let sum = u32::from_le_bytes(s[12..16].try_into().ok()?);
        if len == 0 || len > MAX_PAYLOAD || (len.div_ceil(512) as u32 + 1) > self.sectors {
            return None;
        }
        let mut data = vec![0u8; len.div_ceil(512) * 512];
        for (i, chunk) in data.chunks_mut(512).enumerate() {
            let mut sec = [0u8; 512];
            if !self.read_sector(1 + i as u32, &mut sec) {
                return None;
            }
            chunk.copy_from_slice(&sec);
        }
        data.truncate(len);
        (fnv(&data) == sum).then_some(data)
    }

    /// Write the payload, then the header (so a torn write leaves the old header invalid or intact).
    pub fn store(&self, payload: &[u8]) -> bool {
        if payload.len() > MAX_PAYLOAD || (payload.len().div_ceil(512) as u32 + 1) > self.sectors {
            return false;
        }
        for (i, chunk) in payload.chunks(512).enumerate() {
            let mut sec = [0u8; 512];
            sec[..chunk.len()].copy_from_slice(chunk);
            if !self.write_sector(1 + i as u32, &sec) {
                return false;
            }
        }
        let mut h = [0u8; 512];
        h[..8].copy_from_slice(MAGIC);
        h[8..12].copy_from_slice(&(payload.len() as u32).to_le_bytes());
        h[12..16].copy_from_slice(&fnv(payload).to_le_bytes());
        self.write_sector(0, &h)
    }
}

fn fnv(d: &[u8]) -> u32 {
    let mut h: u32 = 0x811C_9DC5;
    for b in d {
        h ^= *b as u32;
        h = h.wrapping_mul(0x0100_0193);
    }
    h
}

/// payload = [exp, dark, accent, name_len, name...] + Vfs bytes
pub fn pack(st: &Settings, vfs: &[u8]) -> Vec<u8> {
    let mut v = Vec::with_capacity(vfs.len() + 32);
    v.push(matches!(st.exp, crate::theme::Exp::Mac) as u8);
    v.push(st.dark as u8);
    v.push(st.accent as u8);
    let name = st.user.as_bytes();
    v.push(name.len().min(255) as u8);
    v.extend_from_slice(&name[..name.len().min(255)]);
    v.extend_from_slice(vfs);
    v
}

pub fn unpack(p: &[u8]) -> Option<(Settings, &[u8])> {
    let nl = *p.get(3)? as usize;
    let name = core::str::from_utf8(p.get(4..4 + nl)?).ok()?;
    let st = Settings {
        exp: if *p.first()? == 1 { crate::theme::Exp::Mac } else { crate::theme::Exp::Win },
        dark: *p.get(1)? == 1,
        accent: (*p.get(2)? as usize) % crate::theme::ACCENTS.len(),
        user: String::from(name),
    };
    Some((st, p.get(4 + nl..)?))
}
