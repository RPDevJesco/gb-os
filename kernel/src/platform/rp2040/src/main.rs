#![no_std]
#![no_main]

#[no_mangle]
pub extern "C" fn _reset_handler() -> ! {
    loop { core::hint::spin_loop(); }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop { core::hint::spin_loop(); }
}
