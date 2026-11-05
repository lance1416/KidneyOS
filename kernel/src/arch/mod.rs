//! Architecture-specific code.
//!
//! Context switching, entering user mode, the kernel stack used for interrupts
//! from user mode, interrupt control and the interrupt descriptor table all
//! live here. The rest of the kernel only reaches the hardware for these
//! through the items re-exported from this module.

#[cfg(target_arch = "x86")]
mod x86;

#[cfg(target_arch = "x86")]
pub use x86::*;
