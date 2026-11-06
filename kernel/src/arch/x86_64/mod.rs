//! x86_64 support.
//!
//! `boot.s` takes the CPU from the multiboot2 entry into long mode in the
//! higher half, and [`kmain`] sets up the descriptor tables ([`gdt`], [`idt`])
//! and page tables ([`paging`]) before starting the architecture-independent
//! kernel. The items below are what the rest of the kernel uses.
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

pub use context::{
    FpuState, PrepareThreadContext, SwitchThreadsContext, context_switch, switch_fpu,
};
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

/// With interrupts disabled, enables them until one has been handled. `sti`
/// only takes effect after the next instruction, so none can slip in before
/// the `hlt`.
pub fn wait_for_interrupt() {
    compiler_fence(Ordering::SeqCst);
    // SAFETY: Handling interrupts is safe here, per the caller.
    unsafe { asm!("sti", "hlt", "cli", options(nostack)) };
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
/// The loaded page tables must map `ip` and `sp` for user mode, and the kernel
/// stack must have been set with [`set_kernel_stack`].
pub unsafe fn enter_user_mode(ip: core::ptr::NonNull<u8>, sp: core::ptr::NonNull<u8>) -> ! {
    // Interrupts enabled in user mode (IF), and the reserved bit 1.
    const USER_RFLAGS: u64 = 0x202;
    // SAFETY: Per the caller, user mode can run at ip and sp.
    unsafe {
        asm!(
            // Data segments are mostly ignored in long mode, but iretq resets
            // the ones with a privileged descriptor anyway.
            "mov ds, {data:e}",
            "mov es, {data:e}",
            // The interrupt frame iretq returns with.
            "push {data}", // ss
            "push {sp}",
            "push {rflags}",
            "push {code}", // cs
            "push {ip}",
            // Don't leak kernel values to user mode.
            "xor eax, eax",
            "xor ebx, ebx",
            "xor ecx, ecx",
            "xor edx, edx",
            "xor esi, esi",
            "xor edi, edi",
            "xor ebp, ebp",
            "xor r8d, r8d",
            "xor r9d, r9d",
            "xor r10d, r10d",
            "xor r11d, r11d",
            "xor r12d, r12d",
            "xor r13d, r13d",
            "xor r14d, r14d",
            "xor r15d, r15d",
            "iretq",
            data = in(reg) u64::from(gdt::USER_DATA_SELECTOR),
            code = const gdt::USER_CODE_SELECTOR,
            rflags = const USER_RFLAGS,
            sp = in(reg) sp.as_ptr(),
            ip = in(reg) ip.as_ptr(),
            options(noreturn),
        )
    }
}
