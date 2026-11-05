//! Architecture-independent handling of CPU exceptions. The architecture's
//! interrupt entry stubs call into these.

use crate::interrupts::intr_enable;
use crate::system::running_process;

pub extern "C" fn unhandled() -> ! {
    panic!("unhandled interrupt");
}

/// Handles a page fault when accessing `vaddr` from the instruction at
/// `return_ip`, by installing a page from a VMA covering `vaddr`.
pub fn page_fault(vaddr: usize, error_code: u32, return_ip: usize) {
    // important: re-enable interrupts before acquiring lock to prevent deadlock
    intr_enable();
    let pcb = running_process();
    let pcb = pcb.lock();
    // try checking for a VMA matching this address
    // SAFETY: the faulting address isn't mapped, so installing a page for it
    // can't invalidate any existing references.
    if !unsafe { pcb.vmas.install_pte(vaddr) } {
        panic!(
            "page fault with error code {error_code:#b} occurred when trying to access {vaddr:#X} from instruction at {return_ip:#X}"
        );
    }
}

/// Called directly from the entry stub, with the CPU-pushed error code and
/// return address as arguments.
pub extern "C" fn general_protection_fault(error_code: u32, return_ip: usize) -> ! {
    panic!(
        "general protection fault with error code {error_code:#b} occurred from instruction at {return_ip:#X}"
    );
}
