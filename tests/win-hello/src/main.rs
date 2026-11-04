#![no_std]
#![no_main]

use core::ffi::c_void;

#[link(name = "kernel32")]
extern "system" {
    fn GetStdHandle(n: u32) -> *mut c_void;
    fn WriteFile(h: *mut c_void, buf: *const u8, len: u32, written: *mut u32, ov: *mut c_void) -> i32;
    fn ExitProcess(code: u32) -> !;
}

static MSG: &[u8] = b"Hello from a native x64 Windows program on Aeris!\r\n";

#[no_mangle]
pub extern "system" fn start() -> ! {
    // SAFETY: plain Win32 calls with valid arguments.
    unsafe {
        let out = GetStdHandle(0xFFFF_FFF5); // STD_OUTPUT_HANDLE
        let mut n = 0u32;
        WriteFile(out, MSG.as_ptr(), MSG.len() as u32, &mut n, core::ptr::null_mut());
        ExitProcess(7)
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
