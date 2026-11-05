//! Page tables.
//!
//! The i686 kernel uses two-level 32-bit page tables ([`i686`]), the x86_64
//! kernel four-level long mode ones ([`x86_64`]). Both provide a `PageManager`
//! with the same methods, re-exported here for the architecture being built.

#[cfg(target_arch = "x86")]
mod i686;
#[cfg(target_arch = "x86")]
pub use i686::*;

// Built on every target, so that it is unit tested on the (i686) test host
// too; only loading the page tables is x86_64 specific.
pub mod x86_64;
#[cfg(target_arch = "x86_64")]
pub use self::x86_64::*;
