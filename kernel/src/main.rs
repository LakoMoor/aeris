//! Aeris kernel entry point: boot splash -> Setup -> desktop.

#![no_std]
#![no_main]

extern crate alloc;

mod apps;
mod desktop;
mod gfx;
mod icons;
mod input;
mod oobe;
mod programs;
mod storage;
mod sys;
mod theme;
mod wallpaper;
mod winproc;
mod widgets;

use aeris_core::vfs::Vfs;
use apps::{Action, AppKind};
use bootloader_api::config::BootloaderConfig;
use bootloader_api::info::{FrameBuffer, PixelFormat};
use bootloader_api::{entry_point, BootInfo};
use core::fmt::Write;
use desktop::Desktop;
use gfx::{Canvas, Rect};
use input::{Event, Input};
use linked_list_allocator::LockedHeap;

#[global_allocator]
static ALLOC: LockedHeap = LockedHeap::empty();

const HEAP_SIZE: usize = 96 * 1024 * 1024;
static mut HEAP: [u8; HEAP_SIZE] = [0; HEAP_SIZE];

pub static CONFIG: BootloaderConfig = {
    let mut c = BootloaderConfig::new_default();
    c.kernel_stack_size = 512 * 1024;
    c.mappings.physical_memory = Some(bootloader_api::config::Mapping::Dynamic);
    c
};
entry_point!(kmain, config = &CONFIG);

struct Serial;
impl Write for Serial {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            sys::outb(0x3F8, b);
        }
        Ok(())
    }
}

macro_rules! log {
    ($($a:tt)*) => {{ let _ = writeln!(Serial, $($a)*); }};
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let _ = writeln!(Serial, "KERNEL PANIC: {info}");
    loop {
        sys::pause();
    }
}

// ---- screen --------------------------------------------------------------

struct Screen {
    fb: &'static mut FrameBuffer,
    w: i32,
    h: i32,
}

const CURSOR: [&str; 19] = [
    "X           ",
    "XX          ",
    "X.X         ",
    "X..X        ",
    "X...X       ",
    "X....X      ",
    "X.....X     ",
    "X......X    ",
    "X.......X   ",
    "X........X  ",
    "X.....XXXXX ",
    "X..X..X     ",
    "X.X X..X    ",
    "XX  X..X    ",
    "X    X..X   ",
    "     X..X   ",
    "      X..X  ",
    "      X..X  ",
    "       XX   ",
];

impl Screen {
    fn present(&mut self, scene: &[u32], r: Rect) {
        let r = r.intersect(&Rect::new(0, 0, self.w, self.h));
        if r.w <= 0 || r.h <= 0 {
            return;
        }
        let info = self.fb.info();
        let bpp = info.bytes_per_pixel;
        let stride = info.stride;
        let fmt = info.pixel_format;
        let buf = self.fb.buffer_mut();
        for y in r.y..r.bottom() {
            let src = &scene[(y * self.w + r.x) as usize..(y * self.w + r.right()) as usize];
            let off = (y as usize * stride + r.x as usize) * bpp;
            if bpp == 4 && fmt == PixelFormat::Bgr {
                // SAFETY: u32 -> bytes view of a plain slice.
                let bytes = unsafe { core::slice::from_raw_parts(src.as_ptr() as *const u8, src.len() * 4) };
                buf[off..off + bytes.len()].copy_from_slice(bytes);
            } else {
                for (i, &p) in src.iter().enumerate() {
                    put(buf, off + i * bpp, bpp, fmt, p);
                }
            }
        }
    }

    fn draw_cursor(&mut self, x: i32, y: i32) {
        let info = self.fb.info();
        let (bpp, stride, fmt) = (info.bytes_per_pixel, info.stride, info.pixel_format);
        let buf = self.fb.buffer_mut();
        for (j, row) in CURSOR.iter().enumerate() {
            for (i, c) in row.bytes().enumerate() {
                let (px, py) = (x + i as i32, y + j as i32);
                if px < 0 || py < 0 || px >= self.w || py >= self.h || c == b' ' {
                    continue;
                }
                let col = if c == b'X' { 0x000000 } else { 0xFFFFFF };
                put(buf, (py as usize * stride + px as usize) * bpp, bpp, fmt, col);
            }
        }
    }
}

fn put(buf: &mut [u8], off: usize, bpp: usize, fmt: PixelFormat, p: u32) {
    let (r, g, b) = ((p >> 16) as u8, (p >> 8) as u8, p as u8);
    match fmt {
        PixelFormat::Rgb => {
            buf[off] = r;
            buf[off + 1] = g;
            buf[off + 2] = b;
        }
        PixelFormat::Bgr => {
            buf[off] = b;
            buf[off + 1] = g;
            buf[off + 2] = r;
        }
        _ => {
            buf[off] = ((r as u16 + g as u16 + b as u16) / 3) as u8;
        }
    }
    let _ = bpp;
}

struct Pointer {
    x: i32,
    y: i32,
    left: bool,
}

fn kmain(bi: &'static mut BootInfo) -> ! {
    // SAFETY: the heap region is a private static handed to the allocator exactly once.
    unsafe {
        ALLOC.lock().init(core::ptr::addr_of_mut!(HEAP) as *mut u8, HEAP_SIZE);
    }
    sys::init_clock();
    let phys = bi.physical_memory_offset.into_option().unwrap_or(0);
    winproc::init(phys);
    log!("cpu tables ready, phys offset {:#x}", phys);
    log!("Aeris kernel starting");
    let ram_addr = bi.ramdisk_addr.into_option();
    let ram_len = bi.ramdisk_len;
    let fb = bi.framebuffer.as_mut().expect("no framebuffer");
    let info = fb.info();
    let (w, h) = (info.width as i32, info.height as i32);
    log!("framebuffer {}x{} {:?} bpp={}", w, h, info.pixel_format, info.bytes_per_pixel);
    let mut screen = Screen { fb, w, h };
    let mut cv = Canvas::new(w, h);
    let mut input = Input::new();
    let mut ptr = Pointer { x: w / 2, y: h / 2, left: false };
    let full = Rect::new(0, 0, w, h);

    // ---- splash
    loop {
        let ms = sys::ms();
        if ms > 2400 {
            break;
        }
        gfx_splash(&mut cv, ms);
        screen.present(&cv.buf, full);
        while input.poll().is_some() {}
        sys::pause();
    }
    log!("splash done");

    // ---- storage
    let disk = storage::Disk::probe();
    log!("data disk: {}", match &disk { Some(d) => d.sectors(), None => 0 });
    let loaded = disk.as_ref().and_then(|d| d.load()).and_then(|p| {
        let (st, vb) = storage::unpack(&p)?;
        let vfs = Vfs::from_bytes(vb, sys::now())?;
        Some((st, vfs))
    });
    log!("restored from disk: {}", loaded.is_some());

    // ---- setup (skipped when a saved system was found)
    let mut oobe = oobe::Oobe::new(w, h);
    oobe.mouse = (ptr.x, ptr.y);
    oobe.done = loaded.is_some();
    let mut dirty = true;
    while !oobe.done {
        let ms = sys::ms();
        let mut moved = false;
        while let Some(ev) = input.poll() {
            match ev {
                Event::Key(k) => {
                    oobe.key(k, ms);
                    dirty = true;
                }
                Event::Mouse { dx, dy, buttons, .. } => {
                    ptr.x = (ptr.x + dx).clamp(0, w - 1);
                    ptr.y = (ptr.y + dy).clamp(0, h - 1);
                    oobe.mouse = (ptr.x, ptr.y);
                    let left = buttons & 1 != 0;
                    if left && !ptr.left {
                        oobe.mouse_down(ptr.x, ptr.y, ms);
                    }
                    ptr.left = left;
                    dirty = true;
                    moved = true;
                }
            }
        }
        if dirty || oobe.animating() {
            oobe.draw(&mut cv, ms);
            screen.present(&cv.buf, full);
            screen.draw_cursor(ptr.x, ptr.y);
            dirty = false;
        } else if moved {
            screen.draw_cursor(ptr.x, ptr.y);
        }
        sys::pause();
    }
    log!("setup done");

    // ---- desktop
    let (st, mut vfs) = loaded.unwrap_or_else(|| (oobe.settings(), Vfs::new(sys::now())));
    vfs.now = sys::now();
    let ramdisk: &[u8] = match (ram_addr, ram_len) {
        // SAFETY: the bootloader maps the ramdisk at this address for `ram_len` bytes.
        (Some(a), l) if l > 0 => unsafe { core::slice::from_raw_parts(a as *const u8, l as usize) },
        _ => &[],
    };
    log!("installed {} programs", programs::install(&mut vfs, ramdisk));
    let mut desk = Desktop::new(w, h, st, vfs);
    let t = sys::now();
    let mut last_saved: alloc::vec::Vec<u8> = alloc::vec::Vec::new();
    let mut last_save_ms = 0u64;
    let mut activity = true;
    desk.launch(
        if desk.st.exp == theme::Exp::Win { AppKind::Terminal(aeris_core::Flavor::PowerShell) } else { AppKind::Terminal(aeris_core::Flavor::Mac) },
        t,
        sys::ms(),
    );
    let mut now = t;
    let mut last_tick = 0u64;
    let mut last_blink = 0u64;
    let mut last_key = desk.hover_key();
    let mut dirty = true;
    let (mut cx, mut cy) = (ptr.x, ptr.y);
    loop {
        let ms = sys::ms();
        let mut moved = false;
        while let Some(ev) = input.poll() {
            match ev {
                Event::Key(k) => {
                    desk.key(k, now, ms);
                    activity = true;
                    dirty = true;
                }
                Event::Mouse { dx, dy, wheel, buttons } => {
                    ptr.x = (ptr.x + dx).clamp(0, w - 1);
                    ptr.y = (ptr.y + dy).clamp(0, h - 1);
                    moved = true;
                    let left = buttons & 1 != 0;
                    if left && !ptr.left {
                        desk.mouse_down(ptr.x, ptr.y, now, ms);
                        activity = true;
                        dirty = true;
                    } else if !left && ptr.left {
                        desk.mouse_up();
                        dirty = true;
                    } else {
                        desk.mouse_move(ptr.x, ptr.y);
                        let k = desk.hover_key();
                        if k != last_key {
                            last_key = k;
                            dirty = true;
                        }
                    }
                    desk.mouse = (ptr.x, ptr.y);
                    ptr.left = left;
                    if wheel != 0 {
                        desk.scroll(wheel);
                        dirty = true;
                    }
                }
            }
        }
        if ms - last_tick >= 250 {
            last_tick = ms;
            let n = sys::now();
            if n.minute != now.minute {
                dirty = true;
            }
            now = n;
            desk.tick_sys(ms / 1000);
        }
        if desk.needs_blink() && ms - last_blink >= 530 {
            last_blink = ms;
            dirty = true;
        }
        if let Some(d) = &disk {
            if activity && ms - last_save_ms >= 2000 {
                activity = false;
                last_save_ms = ms;
                save(d, &desk, &mut last_saved);
            }
        }
        if let Some(a) = desk.power.take() {
            if let Some(d) = &disk {
                save(d, &desk, &mut last_saved);
            }
            match a {
                Action::Shutdown => sys::shutdown(),
                _ => sys::reboot(),
            }
        }
        if dirty {
            desk.draw(&mut cv, now, ms);
            screen.present(&cv.buf, full);
            screen.draw_cursor(ptr.x, ptr.y);
            dirty = false;
            last_key = desk.hover_key();
        } else if moved {
            // Erase the old cursor from the scene and draw it at the new spot.
            screen.present(&cv.buf, Rect::new(cx, cy, 13, 20));
            screen.draw_cursor(ptr.x, ptr.y);
        }
        cx = ptr.x;
        cy = ptr.y;
        sys::pause();
    }
}

fn gfx_splash(cv: &mut Canvas, ms: u64) {
    oobe::draw_splash(cv, ms);
}

/// Write settings + file system to disk if anything changed since the last save.
fn save(d: &storage::Disk, desk: &Desktop, last: &mut alloc::vec::Vec<u8>) {
    let blob = storage::pack(&desk.st, &desk.vfs.to_bytes());
    if blob != *last && d.store(&blob) {
        *last = blob;
        log!("saved to disk");
    }
}
