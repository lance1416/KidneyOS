#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

//! Checks that the ELF loader maps and zeroes a segment's whole memory size,
//! not just its file size: `BSS` lives in `.bss`, which has no file contents
//! and spans several pages. Exits with 0 on success.

use core::ptr::{addr_of_mut, read_volatile, write_volatile};

const LEN: usize = 3 * 4096;

static mut BSS: [u8; LEN] = [0; LEN];

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let bss = addr_of_mut!(BSS).cast::<u8>();
    let mut code = 0;

    for i in [0, LEN / 2, LEN - 1] {
        // SAFETY: i < LEN, and nothing else accesses BSS.
        unsafe {
            if read_volatile(bss.add(i)) != 0 {
                code |= 0x1;
            }
            write_volatile(bss.add(i), 0xAB);
            if read_volatile(bss.add(i)) != 0xAB {
                code |= 0x2;
            }
        }
    }

    kidneyos_syscalls::exit(code);

    loop {}
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
