//! PS/2 keyboard and mouse, polled.

use crate::sys::{inb, outb};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Char(char),
    Enter,
    Backspace,
    Tab,
    Esc,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    Delete,
    PageUp,
    PageDown,
    /// The Windows / Command key.
    Meta,
}

#[derive(Clone, Copy, Debug)]
pub struct KeyEv {
    pub key: Key,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

#[derive(Clone, Copy, Debug)]
pub enum Event {
    Key(KeyEv),
    /// Relative mouse movement and the wheel (positive = up).
    Mouse { dx: i32, dy: i32, wheel: i32, buttons: u8 },
}

const LOWER: &[u8; 58] = b"\0\x1b1234567890-=\x08\tqwertyuiop[]\n\0asdfghjkl;'`\0\\zxcvbnm,./\0*\0 ";
const UPPER: &[u8; 58] = b"\0\x1b!@#$%^&*()_+\x08\tQWERTYUIOP{}\n\0ASDFGHJKL:\"~\0|ZXCVBNM<>?\0*\0 ";

pub struct Input {
    shift: bool,
    ctrl: bool,
    alt: bool,
    caps: bool,
    e0: bool,
    mbuf: [u8; 4],
    midx: usize,
    mlen: usize,
    pub has_mouse: bool,
}

fn wait_write() {
    for _ in 0..100_000 {
        if inb(0x64) & 2 == 0 {
            return;
        }
    }
}
fn wait_read() -> bool {
    for _ in 0..100_000 {
        if inb(0x64) & 1 != 0 {
            return true;
        }
    }
    false
}
fn mouse_cmd(b: u8) -> u8 {
    wait_write();
    outb(0x64, 0xD4);
    wait_write();
    outb(0x60, b);
    if wait_read() {
        inb(0x60)
    } else {
        0
    }
}

impl Input {
    pub fn new() -> Self {
        // Drain stale bytes.
        while inb(0x64) & 1 != 0 {
            inb(0x60);
        }
        wait_write();
        outb(0x64, 0xA8); // enable the auxiliary (mouse) port
        wait_write();
        outb(0x64, 0x20);
        let mut cfg = if wait_read() { inb(0x60) } else { 0x47 };
        cfg &= !0b0011; // no IRQs, we poll
        cfg &= !0b10_0000; // enable aux clock
        wait_write();
        outb(0x64, 0x60);
        wait_write();
        outb(0x60, cfg);
        mouse_cmd(0xF6); // defaults
        // Try to enable the scroll wheel (IntelliMouse).
        for rate in [200u8, 100, 80] {
            mouse_cmd(0xF3);
            mouse_cmd(rate);
        }
        mouse_cmd(0xF2);
        let id = if wait_read() { inb(0x60) } else { 0 };
        let mlen = if id == 3 { 4 } else { 3 };
        mouse_cmd(0xF4); // enable reporting
        Self { shift: false, ctrl: false, alt: false, caps: false, e0: false, mbuf: [0; 4], midx: 0, mlen, has_mouse: true }
    }

    pub fn poll(&mut self) -> Option<Event> {
        for _ in 0..64 {
            let st = inb(0x64);
            if st & 1 == 0 {
                return None;
            }
            let b = inb(0x60);
            if st & 0x20 != 0 {
                if let Some(e) = self.mouse_byte(b) {
                    return Some(e);
                }
            } else if let Some(e) = self.key_byte(b) {
                return Some(e);
            }
        }
        None
    }

    fn mouse_byte(&mut self, b: u8) -> Option<Event> {
        if self.midx == 0 && b & 8 == 0 {
            return None; // out of sync
        }
        self.mbuf[self.midx] = b;
        self.midx += 1;
        if self.midx < self.mlen {
            return None;
        }
        self.midx = 0;
        let f = self.mbuf[0] as i32;
        let dx = self.mbuf[1] as i32 - ((f << 4) & 0x100);
        let dy = self.mbuf[2] as i32 - ((f << 3) & 0x100);
        let wheel = if self.mlen == 4 { ((self.mbuf[3] & 0x0F) as i8).wrapping_shl(4) as i8 >> 4 } else { 0 };
        Some(Event::Mouse { dx, dy: -dy, wheel: -(wheel as i32), buttons: (f & 7) as u8 })
    }

    fn ev(&self, key: Key) -> Event {
        Event::Key(KeyEv { key, ctrl: self.ctrl, alt: self.alt, shift: self.shift })
    }

    fn key_byte(&mut self, b: u8) -> Option<Event> {
        if b == 0xE0 {
            self.e0 = true;
            return None;
        }
        let release = b & 0x80 != 0;
        let code = b & 0x7F;
        let ext = core::mem::take(&mut self.e0);
        match (ext, code) {
            (false, 0x2A) | (false, 0x36) => {
                self.shift = !release;
                return None;
            }
            (false, 0x1D) | (true, 0x1D) => {
                self.ctrl = !release;
                return None;
            }
            (false, 0x38) | (true, 0x38) => {
                self.alt = !release;
                return None;
            }
            (false, 0x3A) => {
                if !release {
                    self.caps = !self.caps;
                }
                return None;
            }
            _ => {}
        }
        if release {
            return None;
        }
        if ext {
            let k = match code {
                0x48 => Key::Up,
                0x50 => Key::Down,
                0x4B => Key::Left,
                0x4D => Key::Right,
                0x47 => Key::Home,
                0x4F => Key::End,
                0x53 => Key::Delete,
                0x49 => Key::PageUp,
                0x51 => Key::PageDown,
                0x1C => Key::Enter,
                0x35 => Key::Char('/'),
                0x5B | 0x5C => Key::Meta,
                _ => return None,
            };
            return Some(self.ev(k));
        }
        let idx = code as usize;
        if idx >= LOWER.len() {
            return None;
        }
        let mut c = if self.shift { UPPER[idx] } else { LOWER[idx] };
        if self.caps && c.is_ascii_alphabetic() {
            c = if self.shift { c.to_ascii_lowercase() } else { c.to_ascii_uppercase() };
        }
        let k = match c {
            0 => return None,
            0x1b => Key::Esc,
            0x08 => Key::Backspace,
            b'\t' => Key::Tab,
            b'\n' => Key::Enter,
            c => Key::Char(c as char),
        };
        Some(self.ev(k))
    }
}
