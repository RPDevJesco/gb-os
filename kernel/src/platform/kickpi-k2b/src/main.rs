#![no_std]
#![no_main]

#[no_mangle]
pub extern "C" fn boot_main() -> ! {
    loop { unsafe { core::arch::asm!("wfe"); } }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop { unsafe { core::arch::asm!("wfe"); } }
}
