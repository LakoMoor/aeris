#![no_std]
#![no_main]
#[no_mangle]
pub extern "system" fn start() -> ! {
    // SAFETY: deliberately invalid: writes to address 0 to trigger an access violation.
    unsafe { core::ptr::write_volatile(core::hint::black_box(8usize) as *mut u32, 1) };
    loop {}
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
