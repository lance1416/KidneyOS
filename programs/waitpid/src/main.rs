#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

/// The kernel stores a child's exit code in bits 8..16 of the waitpid status,
/// like POSIX `WEXITSTATUS`.
fn wexitstatus(status: i32) -> i32 {
    (status >> 8) & 0xff
}

#[unsafe(no_mangle)]
pub extern "C" fn _start() -> ! {
    let mut status: i32 = 0;
    // Currently hard coding the wait process for testing. Waiting on a child
    // of its own (the commented-out code below) needs fork, which the kernel
    // doesn't implement yet.
    kidneyos_syscalls::waitpid(2, &mut status, 1);

    kidneyos_syscalls::exit(wexitstatus(status));

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
