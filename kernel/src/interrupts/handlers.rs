//! Architecture-independent handling of CPU exceptions. The architecture's
//! interrupt entry stubs call into these.
use crate::interrupts::intr_enable;
use crate::system::running_process;
use crate::threading::process_functions::kill_process;

/// Bit 0 of a page fault's error code: the page is present, so the access
/// broke its protection (e.g. a write to a read-only page, or executing a
/// non-executable one), rather than finding nothing mapped.
const PAGE_FAULT_PRESENT: u32 = 1 << 0;
/// Bit 2 of a page fault's error code: the access was made in user mode.
const PAGE_FAULT_USER: u32 = 1 << 2;

/// Handles a page fault when accessing `vaddr` from the instruction at
/// `return_ip`, by installing a page from a VMA covering `vaddr`. A fault
/// that no VMA covers, or on a page that is present, kills the process if it
/// happened in user mode (and is a kernel bug otherwise).
pub fn page_fault(vaddr: usize, error_code: u32, return_ip: usize) {
    // important: re-enable interrupts before acquiring lock to prevent deadlock
    intr_enable();
    if error_code & PAGE_FAULT_PRESENT == 0 {
        let pcb = running_process();
        let pcb = pcb.lock();
        // try checking for a VMA matching this address
        // SAFETY: the faulting address isn't mapped, so installing a page for it
        // can't invalidate any existing references.
        if unsafe { pcb.vmas.install_pte(vaddr) } {
            return;
        }
    }
    if error_code & PAGE_FAULT_USER != 0 {
        kill_process(format_args!(
            "page fault with error code {error_code:#b} when accessing {vaddr:#X} from \
             instruction at {return_ip:#X}"
        ));
    }
    panic!(
        "page fault with error code {error_code:#b} occurred when trying to access {vaddr:#X} from instruction at {return_ip:#X}"
    );
}

/// Kills the running process for an exception it caused in user mode.
pub fn user_exception(name: &str, error_code: u64, return_ip: usize) -> ! {
    intr_enable();
    kill_process(format_args!(
        "{name} with error code {error_code:#X} at instruction {return_ip:#X}"
    ));
}
