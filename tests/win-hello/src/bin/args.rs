#![no_std]
#![no_main]
use core::ffi::c_void;
#[link(name = "kernel32")]
extern "system" {
    fn GetStdHandle(n: u32) -> *mut c_void;
    fn WriteFile(h: *mut c_void, buf: *const u8, len: u32, written: *mut u32, ov: *mut c_void) -> i32;
    fn GetCommandLineA() -> *const u8;
    fn ExitProcess(code: u32) -> !;
}
#[no_mangle]
pub extern "system" fn start() -> ! {
    // SAFETY: GetCommandLineA returns a NUL-terminated string.
    unsafe {
        let out = GetStdHandle(0xFFFF_FFF5);
        let p = GetCommandLineA();
        let mut n = 0u32;
        while *p.add(n as usize) != 0 {
            n += 1;
        }
        let mut w = 0u32;
        WriteFile(out, b"cmdline: ".as_ptr(), 9, &mut w, core::ptr::null_mut());
        WriteFile(out, p, n, &mut w, core::ptr::null_mut());
        WriteFile(out, b"\r\n".as_ptr(), 2, &mut w, core::ptr::null_mut());
        ExitProcess(0)
    }
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
