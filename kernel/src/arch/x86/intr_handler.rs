use core::arch::asm;

use crate::drivers::ata::ata_interrupt;
use crate::drivers::input::keyboard;
use crate::interrupts::{handlers, pic, timer};
use crate::threading::scheduling;
use crate::user_program::syscall;

/* This file contains all the interrupt handlers to be installed in the IDT when the kernel is initialized.
 * Each must be naked function with C linkage and the type fn() -> !
 */

#[unsafe(naked)]
pub unsafe extern "C" fn unhandled_handler() -> ! {
    core::arch::naked_asm!(
        "call {}",
        sym handlers::unhandled,
    )
}

#[unsafe(naked)]
pub unsafe extern "C" fn page_fault_handler() -> ! {
    extern "C" fn inner(error_code: u32, return_eip: usize) {
        let vaddr: usize;
        // SAFETY: cr2 holds the address of the most recent page fault.
        unsafe { asm!("mov {}, cr2", out(reg) vaddr) };
        handlers::page_fault(vaddr, error_code, return_eip);
    }

    core::arch::naked_asm!(
        "
        pusha
        # pusha pushes 8 registers, so to get past them we need to add 8 * 4 = 32 bytes to the stack pointer
        # first push return_eip, which is above error_code on the stack, so need to add 4 extra bytes
        push [esp+36]
        # now push error_code; due to previous push we need to add 4 extra bytes here as well
        push [esp+36]
        call {}
        # pop arguments
        add esp, 8
        popa
        # pop error code argument
        add esp, 4
        iretd
        ",
        sym inner,
    )
}

#[unsafe(naked)]
pub unsafe extern "C" fn general_protection_fault_handler() -> ! {
    core::arch::naked_asm!(
        "
        call {}
        ",
        sym handlers::general_protection_fault,
    )
}

#[unsafe(naked)]
pub unsafe extern "C" fn syscall_handler() -> ! {
    core::arch::naked_asm!(
        "
        // Syscall ABI: eax holds the syscall number on entry and the return
        // value on exit; ebx, ecx and edx hold the arguments. Every register
        // other than eax is preserved, so no kernel values leak back to the
        // program through the caller-saved registers of the C ABI.
        pushad
        // The C ABI requires the direction flag to be clear on function entry.
        cld

        // Push arguments to stack.
        push edx
        push ecx
        push ebx
        push eax

        call {}

        add esp, 16 // Drop arguments from stack.

        // Overwrite the saved eax (the last of the 8 registers pushed by
        // pushad, so at the highest address) with the return value.
        mov [esp + 28], eax
        popad

        iretd
        ",
        sym syscall::handler,
    )
}

#[unsafe(naked)]
pub unsafe extern "C" fn timer_interrupt_handler() -> ! {
    core::arch::naked_asm!(
        "
        pusha
        // Push IRQ0 value onto the stack.
        push 0x0
        call {} // Update system clock
        call {} // Send EOI signal to PICs
        call {} // Yield process

        add esp, 4 // Drop arguments from stack
        popa
        iretd
        ",
        sym timer::step_sys_clock,
        sym pic::send_eoi,
        sym scheduling::scheduler_yield_and_continue,
    )
}

#[unsafe(naked)]
pub unsafe extern "C" fn ide_prim_interrupt_handler() -> ! {
    core::arch::naked_asm!(
    "
    pusha
    // Push IRQ14 value onto the stack.
    push 0XE
    call {} // Send irq signal to ATA
    call {} // Send EOI signal to PICs
    call {} // Yield process

    add esp, 4 // Drop arguments from stack
    popa
    iretd
    ",
    sym ata_interrupt::on_ide_interrupt,
    sym pic::send_eoi,
    sym scheduling::scheduler_yield_and_continue,
    )
}

#[unsafe(naked)]
pub unsafe extern "C" fn ide_secd_interrupt_handler() -> ! {
    core::arch::naked_asm!(
    "
    pusha
    // Push IRQ15 value onto the stack.
    push 0XF
    call {} // Send irq signal to ATA
    call {} // Send EOI signal to PICs
    call {} // Yield process

    add esp, 4 // Drop arguments from stack
    popa
    iretd
    ",
    sym ata_interrupt::on_ide_interrupt,
    sym pic::send_eoi,
    sym scheduling::scheduler_yield_and_continue,
    )
}

#[unsafe(naked)]
pub unsafe extern "C" fn keyboard_handler() -> ! {
    core::arch::naked_asm!(
    "
    pusha
    // Push IRQ1 value onto the stack.
    push 0X1
    call {} // Handle keyboard interrupt
    call {} // Send EOI signal to PICs
    call {} // Yield process

    add esp, 4 // Drop arguments from stack
    popa
    iretd
    ",
    sym keyboard::atkbd::on_keyboard_interrupt,
    sym pic::send_eoi,
    sym scheduling::scheduler_yield_and_continue,
    )
}
