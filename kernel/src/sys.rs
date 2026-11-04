//! Hardware access: port I/O, RTC, TSC clock and power control.

use aeris_core::Stamp;
use core::arch::asm;
use core::sync::atomic::{AtomicU64, Ordering};

#[inline]
pub fn outb(port: u16, v: u8) {
    // SAFETY: plain port write; callers only use well-known legacy ports.
    unsafe { asm!("out dx, al", in("dx") port, in("al") v, options(nomem, nostack, preserves_flags)) }
}
#[inline]
pub fn inb(port: u16) -> u8 {
    let v: u8;
    // SAFETY: plain port read of a well-known legacy port.
    unsafe { asm!("in al, dx", out("al") v, in("dx") port, options(nomem, nostack, preserves_flags)) }
    v
}
#[inline]
pub fn inw(port: u16) -> u16 {
    let v: u16;
    // SAFETY: plain port read (ATA data register).
    unsafe { asm!("in ax, dx", out("ax") v, in("dx") port, options(nomem, nostack, preserves_flags)) }
    v
}
#[inline]
pub fn outw(port: u16, v: u16) {
    // SAFETY: plain port write (ACPI/QEMU power ports).
    unsafe { asm!("out dx, ax", in("dx") port, in("ax") v, options(nomem, nostack, preserves_flags)) }
}

#[inline]
pub fn rdtsc() -> u64 {
    let lo: u32;
    let hi: u32;
    // SAFETY: rdtsc has no side effects.
    unsafe { asm!("rdtsc", out("eax") lo, out("edx") hi, options(nomem, nostack, preserves_flags)) }
    ((hi as u64) << 32) | lo as u64
}

pub fn pause() {
    core::hint::spin_loop();
}

static TSC_PER_MS: AtomicU64 = AtomicU64::new(2_000_000);
static BOOT_TSC: AtomicU64 = AtomicU64::new(0);

/// Calibrate the TSC against PIT channel 2 (about 10 ms).
pub fn init_clock() {
    outb(0x61, (inb(0x61) & 0xFD) | 1);
    outb(0x43, 0xB0);
    let count: u16 = 11932;
    outb(0x42, (count & 0xFF) as u8);
    outb(0x42, (count >> 8) as u8);
    let v = inb(0x61) & 0xFE;
    outb(0x61, v);
    outb(0x61, v | 1);
    let t0 = rdtsc();
    let mut guard = 0u32;
    while inb(0x61) & 0x20 == 0 && guard < 50_000_000 {
        guard += 1;
    }
    let t1 = rdtsc();
    let per_ms = (t1 - t0) / 10;
    if per_ms > 100_000 {
        TSC_PER_MS.store(per_ms, Ordering::Relaxed);
    }
    BOOT_TSC.store(rdtsc(), Ordering::Relaxed);
}

/// Milliseconds since boot.
pub fn ms() -> u64 {
    (rdtsc() - BOOT_TSC.load(Ordering::Relaxed)) / TSC_PER_MS.load(Ordering::Relaxed)
}

fn rtc(reg: u8) -> u8 {
    outb(0x70, reg);
    inb(0x71)
}

pub fn now() -> Stamp {
    let mut guard = 0;
    while rtc(0x0A) & 0x80 != 0 && guard < 100_000 {
        guard += 1;
    }
    let (mut s, mut mi, mut h, mut d, mut mo, mut y) = (rtc(0), rtc(2), rtc(4), rtc(7), rtc(8), rtc(9));
    let b = rtc(0x0B);
    if b & 4 == 0 {
        let bcd = |v: u8| (v & 0x0F) + (v >> 4) * 10;
        let pm = h & 0x80 != 0;
        s = bcd(s);
        mi = bcd(mi);
        h = bcd(h & 0x7F) | if pm { 0x80 } else { 0 };
        d = bcd(d);
        mo = bcd(mo);
        y = bcd(y);
    }
    if b & 2 == 0 && h & 0x80 != 0 {
        h = ((h & 0x7F) % 12) + 12;
    }
    let mut st = Stamp::new(2000 + y as u16, mo.clamp(1, 12), d.max(1), h % 24, mi);
    st.second = s;
    st
}

pub fn shutdown() -> ! {
    outw(0x604, 0x2000); // QEMU / ACPI PM1a
    outw(0xB004, 0x2000); // Bochs / old QEMU
    outw(0x4004, 0x3400); // VirtualBox
    loop {
        pause();
    }
}

pub fn reboot() -> ! {
    outb(0x64, 0xFE);
    loop {
        pause();
    }
}
