#![no_std]

pub mod bit_array;

// Re-exported so that `bitfield!` can be expanded in crates which do not depend on pastey.
#[doc(hidden)]
pub use pastey as __pastey;

pub mod macros;
pub mod mem;
pub mod multiboot2;
pub mod paging;
pub mod serial;
pub mod sizes;
pub mod video_memory;
