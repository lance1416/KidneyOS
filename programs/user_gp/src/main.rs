#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

//! For `make KIDNEYOS_TEST=usergp smoke-test`: raises a general protection
//! fault in user mode, which must kill this process and nothing else. Started
//! once per way of doing so; processes started together get consecutive pids,
//! so the pid picks a different one for each. Exits with 1 if it survives.

use core::arch::asm;
use core::ptr::read_volatile;
use kidneyos_syscalls::{exit, getpid};

/// The first address above the lower canonical half.
const NON_CANONICAL: usize = 0x0000_8000_0000_0000;

kidneyos_syscalls::entry_point!(main);

extern "C" fn main() -> ! {
    // SAFETY: Each of these faults, and that is the point.
    unsafe {
        match getpid() % 4 {
            0 => asm!("cli"),
            1 => asm!("hlt"),
            // The breakpoint gate can't be used from ring 3, so this is a
            // general protection fault too.
            2 => asm!("int3"),
            _ => {
                read_volatile(NON_CANONICAL as *const u8);
            }
        }
    }

    exit(1);

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
