//! Switching between the kernel stacks of threads.

use crate::threading::{
    thread_control_block::ThreadControlBlock,
    thread_functions::{ThreadFunction, run_thread},
};
use core::mem::offset_of;

macro_rules! save_registers {
    () => {
        // Saves the current thread's registers into it's context (on the stack).
        r#"
        push ebp
        mov ebp, esp    # Part of the calling convention, saving where this stack starts.
                        # We allocate no local variables.
        push ebx
        push esi
        push edi
        "#
    };
}

macro_rules! load_arguments {
    () => {
        // Loads two arguments from the stack into %eax and %edx.
        // * %eax = [%ebp + 0x8] is our first argument (switch_from).
        // * %edx = [%ebp + 0x12] is our second argument (switch_to).
        // These arguments are pointers to the TCB / Stack pointers.
        //
        // Note: We do not change the value of %eax as this will be our return value.
        r#"
            mov eax, [ebp + 0x8]
            mov edx, [ebp + 0xc]
        "#
    };
}

macro_rules! switch_stacks {
    () => {
        // Switches the current stack pointer.
        // Both %eax and %edx are *TCB = **stack and thus must be dereferenced once to get the stack pointer.
        r#"
            mov [eax + {offset}], esp
            mov esp, [edx + {offset}]
        "#
    };
}

macro_rules! restore_registers {
    () => {
        // Pops from the current stack to restore the thread's context.
        r#"
            pop edi
            pop esi
            pop ebx
            pop ebp
        "#
    };
}

/// Performs a context switch between two threads.
///
/// Must save the callee's registers and restore the next's registers.
///
/// The caller saved registers are: %eax, %ecx, and %edx.
/// So we may use them freely.
/// All others must be saved as part of the context switch.
///
/// Parameters are pushed to the stack the opposite order they are defined.
/// The last is pushed to the stack first (higher address), and the first is pushed last (lower address).
/// The caller is responsible to remove these from the stack.
///
/// Our return value will need to be placed into the %eax register.
#[unsafe(naked)]
pub unsafe extern "C" fn context_switch(
    _switch_from: *mut ThreadControlBlock,
    _switch_to: *mut ThreadControlBlock,
) -> *mut ThreadControlBlock {
    // Our function arguments are placed on the stack Right to Left.
    core::arch::naked_asm!(
        save_registers!(),
        load_arguments!(), // Required manually since this is a naked function.
        switch_stacks!(),
        restore_registers!(),
        r#"
            ret
        "#, // Required manually since this is a naked function.
        offset = const offset_of!(ThreadControlBlock, kernel_stack_pointer),
    )
}

#[allow(unused)]
#[repr(C, packed)]
pub struct PrepareThreadContext {
    entry_function: *const u8,
}

impl PrepareThreadContext {
    pub fn new(entry_function: *const u8) -> Self {
        Self { entry_function }
    }
}

/// This function is used to clean up a thread's arguments and call into `run_thread`.
#[unsafe(naked)]
unsafe extern "C" fn prepare_thread() -> i32 {
    // Since this function is only to be called from the `context_switch` function, we expect
    // That %eax and %edx contain the arguments passed to it.
    // Further, the entry function pointer is at a known position on the stack.
    // We move this into a register and call the run thread function.
    core::arch::naked_asm!(
        r#"
            # push [esp] # Already in place on stack.
            push edx
            push eax
            call {}
            hlt     # Never return to here.
        "#,
        sym run_thread,
    )
}

/// The context for a use within context_switch.
#[repr(C, packed)]
pub struct SwitchThreadsContext {
    edi: usize,          // Destination index.
    esi: usize,          // Source index.
    ebx: usize,          // Base (for memory access).
    ebp: usize,          // Stack base pointer.
    eip: ThreadFunction, // Instruction pointer (determines where to jump after the context switch).
}

impl SwitchThreadsContext {
    pub fn new() -> Self {
        Self {
            edi: 0,
            esi: 0,
            ebx: 0,
            ebp: 0,
            eip: prepare_thread,
        }
    }
}
