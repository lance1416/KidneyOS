pub mod frames;
pub mod regions;

use crate::sizes::{KB, MB};

// Page size is 4KB. This is a property of x86 processors.
pub const PAGE_FRAME_SIZE: usize = 4 * KB;
pub const HUGE_PAGE_SIZE: usize = 2 * MB;

// Layout of the virtual address space. The kernel's own symbols (the kernel
// image's sections) are declared where they're used, see
// build-support/x86_64.ld.

/// The kernel image is linked in the top 2GiB of the address space (as
/// required by the kernel code model), and the boot stub maps physical
/// `0..1GiB` there, so physical address `p < 1GiB` is also accessible at
/// `p + KERNEL_BASE`. Must match `KERNEL_BASE` in `build-support/x86_64.ld`.
pub const KERNEL_BASE: usize = 0xFFFF_FFFF_8000_0000;

/// Virtual address of the start of the direct map of physical memory: the
/// physical address `p` is accessible at `p + PHYS_MAP_OFFSET`. Memory handed
/// out by the frame and kernel allocators is addressed through it.
///
/// The direct map starts at the beginning of the higher half (PML4 entry 256)
/// and covers all physical memory in the multiboot2 memory map. Until the
/// kernel has its own page tables, the boot stub maps physical `0..1GiB` here.
pub const PHYS_MAP_OFFSET: usize = 0xFFFF_8000_0000_0000;

/// Exclusive upper bound of user virtual addresses: user space is the lower
/// canonical half. Checks of user pointers must compare against this first:
/// addresses between it and the higher half are not canonical, and the page
/// tables say nothing about them.
pub const USER_TOP: usize = 0x0000_8000_0000_0000;
const _: () = assert!(USER_TOP as u64 == crate::paging::USER_TOP);
