#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

/// The kernel stores a child's exit code in bits 8..16 of the waitpid status,
/// like POSIX `WEXITSTATUS`.
fn wexitstatus(status: i32) -> i32 {
    (status >> 8) & 0xff
}

kidneyos_syscalls::entry_point!(main);

/// The exit code of `example_rust`, the program this one waits for.
const EXPECTED_EXIT_CODE: i32 = 1;

/// Waits for the process started right after this one, and exits with 0 if
/// `waitpid` returned its pid and its exit code. Run as
/// `KIDNEYOS_INIT=waitpid,example_rust` (the kernel starts the init programs
/// one after the other, so they get consecutive pids).
///
/// Waiting on a child of its own (the commented-out code below) needs fork,
/// which the kernel doesn't implement yet.
extern "C" fn main() -> ! {
    let child = kidneyos_syscalls::getpid() + 1;
    let mut status: i32 = 0;
    let waited = kidneyos_syscalls::waitpid(child, &mut status, 0);

    if waited != child {
        kidneyos_syscalls::exit(0x100);
    }
    if wexitstatus(status) != EXPECTED_EXIT_CODE {
        kidneyos_syscalls::exit(0x200 | wexitstatus(status));
    }
    kidneyos_syscalls::exit(0);

    // let p = kidneyos_syscalls::fork();

    // if p == 0 {
    //     kidneyos_syscalls::exit(1);
    // } else {
    //     let mut status: i32 = 0;
    //     kidneyos_syscalls::waitpid(p, &mut status, 0);
    //     if kidneyos_syscalls::wifexited(status) {
    //         kidneyos_syscalls::exit(2);
    //     }
    //     let exit_code = kidneyos_syscalls::wifexitstatus(status) as usize;
    //     // Should be 1
    //     kidneyos_syscalls::exit(exit_code);
    // }

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
