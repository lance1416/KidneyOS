#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]

//! Checks that the kernel saves and restores the SSE registers of processes:
//! does the same long floating-point computation twice and exits with 0 if
//! both give exactly the same result (and 1 otherwise).
//!
//! On x86_64, f64 arithmetic uses the SSE (xmm) registers, which hold the
//! computation's state for most of it. Run two of these at the same time
//! (`KIDNEYOS_INIT=sse,sse`): each is preempted many times during a
//! computation, and starts from its own pid, so if the other process's
//! registers leaked into it, its results would differ.

use core::hint::black_box;

/// Enough iterations that a computation spans many timer ticks.
const ITERATIONS: u32 = 3_000_000;

kidneyos_syscalls::entry_point!(main);

extern "C" fn main() -> ! {
    let seed = f64::from(kidneyos_syscalls::getpid());
    let first = compute(seed);
    let second = compute(seed);
    kidneyos_syscalls::exit(if first.to_bits() == second.to_bits() {
        0
    } else {
        1
    });

    // exit() doesn't return.
    loop {
        core::hint::spin_loop();
    }
}

#[inline(never)]
fn compute(seed: f64) -> f64 {
    let mut a = black_box(seed);
    let mut b = 1.0 / a;
    let mut sum = 0.0;
    for i in 0..ITERATIONS {
        a = a * 0.999_999_7 + b;
        b = b * 1.000_000_3 - f64::from(i) * 1e-12;
        sum += a * b;
    }
    sum
}

#[cfg(not(test))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {}
}
