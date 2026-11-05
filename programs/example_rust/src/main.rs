#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

kidneyos_syscalls::entry_point!(main);

extern "C" fn main() -> ! {
    kidneyos_syscalls::exit(1);

    // exit() doesn't return.
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
