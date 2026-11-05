//! Architecture-specific code.
//!
//! Context switching, entering user mode, the kernel stack used for interrupts
//! from user mode, interrupt control, the descriptor tables and the boot code
//! all live here. The rest of the kernel only reaches the hardware for these
//! through the items re-exported from this module.
//!
//! KidneyOS only runs on x86_64 ([`x86_64`]), whose entry point is
//! `x86_64::kmain`. The implementation is kept in its own module behind this
//! one, so that the interface the rest of the kernel uses stays in one place
//! (and another port could be added next to it).

#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "x86_64")]
pub use x86_64::*;
