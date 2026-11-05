// Wrapper for timer functions.

use crate::interrupts::timer::sleep;
use core::arch::asm;
use core::time::Duration;

const CPU_FREQUENCY_GHZ: u64 = 2;

/// Sleep for `t` milliseconds.
fn msleep_block(t: u64) {
    usleep_block(t * 1000);
}

/// Sleep for `t` microseconds.
fn usleep_block(t: u64) {
    nsleep_block(t * 1000);
}

/// Sleep for `t` nanoseconds.
fn nsleep_block(t: u64) {
    let cycles_to_wait = t * CPU_FREQUENCY_GHZ;

    let mut start_lo: u32;
    let mut start_hi: u32;
    let mut current_lo: u32;
    let mut current_hi: u32;

    // Get the current value of the time-stamp counter
    unsafe {
        asm!(
        "rdtsc",
        out("eax") start_lo,
        out("edx") start_hi
        );
    }

    loop {
        // Get the current value of the time-stamp counter
        unsafe {
            asm!(
            "rdtsc",
            out("eax") current_lo,
            out("edx") current_hi
            );
        }

        let start = (u64::from(start_hi) << 32) | u64::from(start_lo);
        let current = (u64::from(current_hi) << 32) | u64::from(current_lo);

        if current - start >= cycles_to_wait {
            return; // Exit the loop once the desired number of cycles has passed
        }
    }
}

/// Sleep for `t` milliseconds.
pub fn msleep(t: u64, block: bool) {
    if block {
        msleep_block(t);
    } else {
        sleep(Duration::from_millis(t));
    }
}

/// Sleep for `t` microseconds.
pub fn usleep(t: u64, block: bool) {
    if block {
        usleep_block(t);
    } else {
        sleep(Duration::from_micros(t));
    }
}

/// Sleep for `t` nanoseconds.
pub fn nsleep(t: u64, block: bool) {
    if block {
        nsleep_block(t);
    } else {
        sleep(Duration::from_nanos(t));
    }
}
