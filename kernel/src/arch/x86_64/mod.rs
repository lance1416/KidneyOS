//! x86_64 support.
//!
//! `boot.s` takes the CPU from the multiboot2 entry into long mode in the
//! higher half, and [`kmain`] sets up the descriptor tables ([`gdt`], [`idt`])
//! and page tables ([`paging`]) before starting the architecture-independent
//! kernel. The items below are what the rest of the kernel uses, like those of
//! the i686 module. Entering user mode isn't supported yet.
//!
//! Only the parts that make sense in a host process are built for the host
//! tests (`target_os` other than "none").

#[cfg(target_os = "none")]
mod boot;
mod context;
mod gdt;
#[cfg(target_os = "none")]
mod idt;
#[cfg(target_os = "none")]
mod kmain;
#[cfg(target_os = "none")]
mod paging;

pub use context::{PrepareThreadContext, SwitchThreadsContext, context_switch};
pub use gdt::set_kernel_stack;
#[cfg(target_os = "none")]
pub use paging::share_kernel_half;

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
pub fn halt_forever() -> ! {
    loop {
        // SAFETY: Only stops the CPU; with interrupts disabled, only an NMI
        // wakes it up again.
        unsafe { asm!("cli", "hlt", options(nomem, nostack)) }
    }
}

/// Drops to user mode, continuing at `ip` with the stack pointer `sp`.
///
/// # Safety
///
/// See the i686 version.
pub unsafe fn enter_user_mode(_ip: core::ptr::NonNull<u8>, _sp: core::ptr::NonNull<u8>) -> ! {
    // TODO(x86_64): iretq to ring 3 with the user selectors from gdt.rs (M4).
    // Nothing creates user threads on x86_64 yet, so this isn't reached.
    unimplemented!("user mode on x86_64")
}
