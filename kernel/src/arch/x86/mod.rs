//! 32-bit x86 (i686) support.

mod context;
mod idt;
mod intr_handler;

use core::{
    arch::asm,
    ptr::NonNull,
    sync::atomic::{Ordering, compiler_fence},
};
use kidneyos_shared::{
    global_descriptor_table::{USER_CODE_SELECTOR, USER_DATA_SELECTOR},
    task_state_segment::TASK_STATE_SEGMENT,
};

pub use context::{PrepareThreadContext, SwitchThreadsContext, context_switch};
pub use idt::load as load_idt;

/// Returns whether maskable interrupts are enabled.
pub fn interrupts_enabled() -> bool {
    let flags: u32;
    unsafe {
        asm!(
            "pushfd",
            "mov {}, [esp]",
            "popfd",
            out(reg) flags
        );
    }

    // IF (interrupt enable flag)
    flags & (1 << 9) != 0
}

#[inline(always)]
pub fn enable_interrupts() {
    compiler_fence(Ordering::SeqCst);
    unsafe {
        asm!("sti", options(nomem, nostack));
    }
}

#[inline(always)]
pub fn disable_interrupts() {
    unsafe {
        asm!("cli", options(nomem, nostack));
    }
    compiler_fence(Ordering::SeqCst);
}

/// Sets the stack the CPU switches to when an interrupt arrives in user mode.
///
/// # Safety
///
/// `top` must be the top of a kernel stack that stays valid for as long as it
/// is set, and that nothing else is using while in user mode.
// 32-bit x86 specific: esp0 is a 32-bit field. Revisit for the x86_64 port.
#[allow(clippy::cast_possible_truncation)]
pub unsafe fn set_kernel_stack(top: NonNull<u8>) {
    unsafe {
        TASK_STATE_SEGMENT.esp0 = top.as_ptr() as u32;
    }
}

/// Drops to user mode, continuing at `ip` with the stack pointer `sp`.
///
/// # Safety
///
/// The loaded page tables must map `ip` and `sp` for user mode, and the kernel
/// stack must have been set with [`set_kernel_stack`].
pub unsafe fn enter_user_mode(ip: NonNull<u8>, sp: NonNull<u8>) -> ! {
    // https://wiki.osdev.org/Getting_to_Ring_3#iret_method
    // https://web.archive.org/web/20160326062442/http://jamesmolloy.co.uk/tutorial_html/10.-User%20Mode.html
    unsafe {
        asm!(
            "
            mov ds, {data_sel:x}
            mov es, {data_sel:x}
            mov fs, {data_sel:x}
            mov gs, {data_sel:x} // SS and CS are handled by iret

            // Set up the stack frame iret expects.
            push {data_sel:e} // stack segment
            push {sp}
            pushfd // eflags
            push {code_sel} // code segment
            push {ip}
            iretd
            ",
            data_sel = in(reg) USER_DATA_SELECTOR,
            sp = in(reg) sp.as_ptr(),
            code_sel = const USER_CODE_SELECTOR,
            ip = in(reg) ip.as_ptr(),
            options(noreturn),
        )
    }
}
