#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

//! For `make KIDNEYOS_TEST=protfault smoke-test`: makes a protection fault on
//! a page that is present, in a VMA of this process, which must kill this
//! process and nothing else. Started once per way of doing so; processes
//! started together get consecutive pids, so the pid picks a different one for
//! each. Exits with a nonzero code if it survives.

use core::ffi::c_void;
use core::hint::black_box;
use core::mem::transmute;
use core::ptr::{read_volatile, write_volatile};
use kidneyos_syscalls::defs::{O_CREATE, PROT_READ};
use kidneyos_syscalls::{close, exit, ftruncate, getpid, mmap, open};

const MAPPING: *mut u8 = 0x1234_5000 as *mut u8;
const RET: u8 = 0xC3;

/// Jumps to the stack, which isn't executable.
fn execute_stack() {
    let code = black_box([RET]);
    // SAFETY: Faults before the first instruction, and that is the point.
    let function: extern "C" fn() = unsafe { transmute(code.as_ptr()) };
    function();
}

/// Writes to a read-only page of a file mapping, after a read has brought the
/// page in.
fn write_read_only_mapping() {
    let path = c"/prot_fault".as_ptr();
    let fd = open(path, O_CREATE);
    if fd < 0 || ftruncate(fd, 4096) < 0 || close(fd) < 0 {
        exit(2);
    }
    let fd = open(path, 0);
    let mapping = mmap(MAPPING.cast::<c_void>(), 4096, PROT_READ, 0, fd, 0);
    if mapping.cast::<u8>() != MAPPING {
        exit(3);
    }
    // SAFETY: The mapping is valid to read; writing faults, and that is the
    // point.
    unsafe {
        read_volatile(MAPPING);
        write_volatile(MAPPING, 1);
    }
}

kidneyos_syscalls::entry_point!(main);

extern "C" fn main() -> ! {
    match getpid() % 2 {
        0 => execute_stack(),
        _ => write_read_only_mapping(),
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
