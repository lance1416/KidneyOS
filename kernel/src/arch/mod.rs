//! Architecture-specific code.
//!
//! Context switching, entering user mode, the kernel stack used for interrupts
//! from user mode, interrupt control and the interrupt descriptor table all
//! live here. The rest of the kernel only reaches the hardware for these
//! through the items re-exported from this module.
//!
//! The x86_64 port is still being brought up: for now its module only boots
//! (see `x86_64::boot`) and prints a greeting, and doesn't provide the items
//! above yet.

#[cfg(target_arch = "x86")]
mod x86;

#[cfg(target_arch = "x86")]
pub use x86::*;

#[cfg(target_arch = "x86_64")]
mod x86_64;
