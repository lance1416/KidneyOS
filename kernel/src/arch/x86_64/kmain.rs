//! The Rust entry point of the x86_64 kernel.

use core::{arch::asm, panic::PanicInfo};

/// Called by `_start` in boot.s, in long mode, with the multiboot2 magic and
/// the physical address of the multiboot2 boot information.
#[unsafe(no_mangle)]
extern "C" fn kmain(_magic: u32, _multiboot2_info_phys: usize) -> ! {
    // TODO(x86_64): Set up the GDT, TSS, IDT, page tables and the rest of the
    // kernel here.
    halt_forever()
}

/// Halts the CPU for good.
fn halt_forever() -> ! {
    loop {
        // SAFETY: Only stops the CPU; with interrupts disabled, only an NMI
        // wakes it up again.
        unsafe { asm!("cli", "hlt", options(nomem, nostack)) }
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    halt_forever()
}
