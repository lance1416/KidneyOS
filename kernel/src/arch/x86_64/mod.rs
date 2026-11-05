//! x86_64 support.
//!
//! `boot.s` takes the CPU from the multiboot2 entry into long mode in the
//! higher half, and [`kmain`] then sets up the kernel's own descriptor tables
//! ([`gdt`], [`idt`]), page tables ([`paging`]) and the timer, and idles. The
//! rest of the kernel (threads, file systems, user programs) is still i686
//! only.

mod boot;
mod gdt;
mod idt;
mod kmain;

use core::{
    arch::asm,
    sync::atomic::{Ordering, compiler_fence},
};

/// Returns whether maskable interrupts are enabled.
pub fn interrupts_enabled() -> bool {
    let flags: u64;
    unsafe { asm!("pushfq", "pop {}", out(reg) flags, options(nomem, preserves_flags)) };
    // IF (interrupt enable flag)
    flags & (1 << 9) != 0
}

#[inline(always)]
pub fn enable_interrupts() {
    compiler_fence(Ordering::SeqCst);
    unsafe { asm!("sti", options(nomem, nostack)) };
}

#[inline(always)]
pub fn disable_interrupts() {
    unsafe { asm!("cli", options(nomem, nostack)) };
    compiler_fence(Ordering::SeqCst);
}

/// Halts the CPU for good.
#[allow(dead_code)]
pub fn halt_forever() -> ! {
    loop {
        // SAFETY: Only stops the CPU; with interrupts disabled, only an NMI
        // wakes it up again.
        unsafe { asm!("cli", "hlt", options(nomem, nostack)) }
    }
}
