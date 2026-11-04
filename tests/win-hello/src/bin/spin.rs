#![no_std]
#![no_main]
#[no_mangle]
pub extern "system" fn start() -> ! {
    let mut x = 0u64;
    loop {
        x = core::hint::black_box(x.wrapping_add(1));
    }
}
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
