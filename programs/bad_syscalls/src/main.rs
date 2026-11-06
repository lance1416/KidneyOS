#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

//! For `make KIDNEYOS_TEST=badsyscall smoke-test`: makes system calls with
//! arguments that used to panic the kernel or do the wrong thing, and checks
//! that they fail cleanly (or work). Exits with 0 on success, and otherwise
//! with the number of the check that failed.

use core::ffi::c_void;
use core::ptr::null_mut;
use kidneyos_syscalls::defs::{Dirent, EFAULT, EINVAL, ENOSYS, O_CREATE, PROT_READ};
use kidneyos_syscalls::{
    Timespec, close, dup, dup2, exit, fork, getdents, getrandom, mmap, nanosleep, open, pipe, read,
    write,
};

/// The end of the user stack, above which nothing is mapped.
const STACK_TOP: usize = 0x7FFF_FFFF_F000;
const USER_TOP: usize = 0x8000_0000_0000;
const PAGE_SIZE: usize = 4096;

#[repr(align(8))]
struct DirentBuffer([u8; 256]);

fn check(number: i32, passed: bool) {
    if !passed {
        exit(number);
    }
}

fn is_error(result: i32, error: isize) -> bool {
    isize::try_from(result).is_ok_and(|result| result == -error)
}

fn check_getrandom() {
    let mut buffer = [0i8; 16];
    let start = buffer.as_mut_ptr();
    // The first byte is fine, but the range runs off the end of the stack.
    let too_long = STACK_TOP - start as usize + 16;
    check(1, is_error(getrandom(start, too_long, 0), EFAULT));
    check(2, getrandom(start, buffer.len(), 0) == 16);
}

fn check_unimplemented() {
    // A Pid is unsigned, so the error comes back truncated.
    check(3, fork() == (-ENOSYS).cast_unsigned() as u16);
    let duration = Timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    check(
        4,
        is_error(nanosleep(&raw const duration, null_mut()), ENOSYS),
    );
}

fn check_mmap() {
    let fd = open(c"/bad_syscalls".as_ptr(), O_CREATE);
    check(5, fd >= 0);
    for (number, addr) in [
        // addr + length overflows.
        (6, usize::MAX & !(PAGE_SIZE - 1)),
        // The end is past the user half.
        (7, USER_TOP - PAGE_SIZE),
        // In the kernel's half.
        (8, 0xFFFF_8000_0000_0000),
    ] {
        let result = mmap(addr as *mut c_void, 2 * PAGE_SIZE, PROT_READ, 0, fd, 0);
        check(number, result as isize == -EINVAL);
    }
    check(9, close(fd) == 0);
}

fn check_getdents() {
    let fd = open(c"/".as_ptr(), 0);
    check(10, fd >= 0);
    let mut buffer = DirentBuffer([0; 256]);
    let size = buffer.0.len() - 1;
    let misaligned = buffer.0.as_mut_ptr().wrapping_add(1).cast::<Dirent>();
    check(11, is_error(getdents(fd, misaligned, size), EINVAL));
    check(12, close(fd) == 0);
}

fn check_dup2() {
    let mut fds = [0; 2];
    check(13, pipe(fds.as_mut_ptr()) == 0);
    let [reader, writer] = fds;

    // Onto itself: nothing happens, in particular it stays open.
    check(14, dup2(reader, reader) >= 0);
    check(15, write(writer, b"x".as_ptr(), 1) == 1);
    let mut byte = 0;
    check(16, read(reader, &raw mut byte, 1) == 1);

    // Onto another open descriptor: that one is closed, not the source.
    let other = dup(writer);
    check(17, other >= 0);
    check(18, dup2(reader, other) >= 0);
    check(19, write(writer, b"y".as_ptr(), 1) == 1);
    check(20, read(reader, &raw mut byte, 1) == 1 && byte == b'y');
    check(21, write(writer, b"z".as_ptr(), 1) == 1);
    check(22, read(other, &raw mut byte, 1) == 1 && byte == b'z');
}

kidneyos_syscalls::entry_point!(main);

extern "C" fn main() -> ! {
    check_getrandom();
    check_unimplemented();
    check_mmap();
    check_getdents();
    check_dup2();

    exit(0);

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
