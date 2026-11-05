#![feature(slice_ptr_get)]
#![no_std]

pub mod bit_array;

// Re-exported so that `bitfield!` can be expanded in crates which do not depend on pastey.
#[doc(hidden)]
pub use pastey as __pastey;

// The descriptor tables and the page tables have a different layout in long
// mode; the x86_64 kernel doesn't use these yet.
#[cfg(target_arch = "x86")]
pub mod global_descriptor_table;
pub mod macros;
pub mod mem;
pub mod multiboot2;
#[cfg(target_arch = "x86")]
pub mod paging;
#[cfg(target_arch = "x86")]
pub mod segment;
pub mod serial;
pub mod sizes;
#[cfg(target_arch = "x86")]
pub mod task_state_segment;
pub mod video_memory;
