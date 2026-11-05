//! Architecture-specific code.
//!
//! Context switching, entering user mode, the kernel stack used for interrupts
//! from user mode, interrupt control and the interrupt descriptor table all
//! live here. The rest of the kernel only reaches the hardware for these
//! through the items re-exported from this module.
//!
//! The x86_64 port doesn't support user mode yet: its `enter_user_mode` is
//! unimplemented, and it has its own entry point (`x86_64::kmain`), which
//! shares the kernel's address space with new threads (`share_kernel_half`).

#[cfg(target_arch = "x86")]
mod x86;

#[cfg(target_arch = "x86")]
pub use x86::*;

#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "x86_64")]
pub use x86_64::*;
