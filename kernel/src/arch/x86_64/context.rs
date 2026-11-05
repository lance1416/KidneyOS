//! Switching between the kernel stacks of threads.
//!
//! With the SysV calling convention, `context_switch` only has to save the
//! callee-saved registers (rbx, rbp and r12-r15); the caller saves the rest.
//! The kernel is built without SSE, so there is no other register state yet.

use crate::threading::{thread_control_block::ThreadControlBlock, thread_functions::run_thread};
use core::mem::offset_of;

/// Saves the callee-saved registers on the current stack, stores the stack
/// pointer in `switch_from`, switches to the stack of `switch_to` and restores
/// its registers. Returns `switch_from`, in the context of `switch_to`.
///
/// A thread that hasn't run yet starts in `prepare_thread`, with the stack
/// [`ThreadControlBlock::new_with_setup`] builds: a [`SwitchThreadsContext`]
/// above a [`PrepareThreadContext`].
///
/// # Safety
///
/// Both must be valid threads, `switch_to`'s stack pointer must point to a
/// `SwitchThreadsContext` (one saved by this function, or a new one), and
/// interrupts must be disabled.
#[unsafe(naked)]
pub unsafe extern "C" fn context_switch(
    _switch_from: *mut ThreadControlBlock,
    _switch_to: *mut ThreadControlBlock,
) -> *mut ThreadControlBlock {
    core::arch::naked_asm!(
        // The return address is already on the stack; the rest of
        // SwitchThreadsContext goes below it.
        "push rbp",
        "push rbx",
        "push r12",
        "push r13",
        "push r14",
        "push r15",
        "mov [rdi + {offset}], rsp",
        "mov rsp, [rsi + {offset}]",
        "pop r15",
        "pop r14",
        "pop r13",
        "pop r12",
        "pop rbx",
        "pop rbp",
        // rdi and rsi are still `switch_from` and `switch_to`, which
        // `prepare_thread` passes on to `run_thread`.
        "mov rax, rdi",
        "ret",
        offset = const offset_of!(ThreadControlBlock, kernel_stack_pointer),
    )
}

/// The stack of a thread that is switched out (or hasn't run yet), from its
/// stack pointer up, as `context_switch` pushes and pops it.
#[repr(C)]
pub struct SwitchThreadsContext {
    r15: usize,
    r14: usize,
    r13: usize,
    r12: usize,
    rbx: usize,
    rbp: usize,
    /// Where `context_switch` returns to.
    rip: unsafe extern "C" fn() -> !,
}

impl SwitchThreadsContext {
    /// The context of a thread that hasn't run yet: zeroed registers (rbp = 0
    /// ends the frame pointer chain) and `prepare_thread` to return to.
    pub fn new() -> Self {
        Self {
            r15: 0,
            r14: 0,
            r13: 0,
            r12: 0,
            rbx: 0,
            rbp: 0,
            rip: prepare_thread,
        }
    }
}

/// What a new thread's stack has above its [`SwitchThreadsContext`]: the
/// entry function, where a return address would be for `prepare_thread`. It
/// isn't read (`run_thread` takes the entry point from the thread control
/// block), but with it a new stack has 64 bytes on it, so that the stack
/// pointer stays 16-byte aligned.
#[repr(C)]
pub struct PrepareThreadContext {
    _entry_function: *const u8,
}

impl PrepareThreadContext {
    pub fn new(entry_function: *const u8) -> Self {
        Self {
            _entry_function: entry_function,
        }
    }
}

const _: () = assert!(
    (size_of::<SwitchThreadsContext>() + size_of::<PrepareThreadContext>()) % 16 == 0,
    "a new thread's stack pointer must stay 16-byte aligned"
);

/// Where new threads start, returned to by `context_switch` with
/// `switch_from` in rdi and `switch_to` in rsi. Calls `run_thread` with them.
#[unsafe(naked)]
unsafe extern "C" fn prepare_thread() -> ! {
    core::arch::naked_asm!(
        // The SysV ABI wants the stack 16-byte aligned at a call. It is
        // already (see PrepareThreadContext), but don't depend on it.
        "and rsp, -16",
        "call {run_thread}",
        "ud2", // run_thread never returns.
        run_thread = sym run_thread,
    )
}
