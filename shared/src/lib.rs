#![feature(slice_ptr_get)]
#![no_std]

pub mod bit_array;

// Re-exported so that `bitfield!` can be expanded in crates which do not depend on pastey.
#[doc(hidden)]
pub use pastey as __pastey;

pub mod global_descriptor_table;
pub mod macros;
pub mod mem;
pub mod paging;
pub mod segment;
pub mod serial;
pub mod sizes;
pub mod task_state_segment;
pub mod video_memory;
