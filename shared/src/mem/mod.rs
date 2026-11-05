pub mod pool_allocator;
pub mod regions;

use crate::sizes::{KB, MB};

// Page size is 4KB. This is a property of x86 processors.
pub const PAGE_FRAME_SIZE: usize = 4 * KB;
pub const HUGE_PAGE_SIZE: usize = 4 * MB;

macro_rules! linker_offsets {
    ($($name:ident),*) => {
        $(
        #[inline]
        pub fn $name() -> usize {
            unsafe extern "C" {
                static $name: u8;
            }

            // The linker script will give this the correct address.
            &raw const $name as usize
        }
        )*
    }
}

pub mod phys {
    linker_offsets!(trampoline_start, trampoline_data_start, trampoline_end);

    macro_rules! to_phys {
        ($($name:ident),*) => {
            $(
            #[inline]
            pub fn $name() -> usize {
                super::virt::$name() - super::KERNEL_BASE
            }
            )*
        }
    }

    to_phys!(kernel_start, kernel_data_start, kernel_end);

    #[inline]
    pub fn main_stack_top() -> usize {
        kernel_end() + super::MAIN_STACK_SIZE
    }

    #[inline]
    pub fn trampoline_heap_top() -> usize {
        main_stack_top() + super::TRAMPOLINE_HEAP_SIZE
    }
}

pub mod virt {
    linker_offsets!(kernel_start, kernel_data_start, kernel_end);

    /// The main stack directly follows the kernel image, so it is mapped
    /// relative to it.
    #[inline]
    pub fn main_stack_top() -> usize {
        super::phys::main_stack_top() + super::KERNEL_BASE
    }

    /// Memory from here on is accessed through the direct map of physical
    /// memory.
    #[inline]
    pub fn trampoline_heap_top() -> usize {
        super::phys::trampoline_heap_top() + super::PHYS_MAP_OFFSET
    }
}

// Layout of the virtual address space. On i686 all three coincide: user space
// is everything below 2GiB, and the kernel image as well as the direct map of
// physical memory both start at 2GiB (so physical address `p` of the kernel
// image is mapped at `p + 0x80000000`, like every other physical address).

/// Virtual address that physical address 0 of the kernel image is linked at:
/// the image (loaded at its physical address by the bootloader) lives at
/// `phys + KERNEL_BASE`. Must match `KERNEL_BASE` in `build-support/i686.ld`.
pub const KERNEL_BASE: usize = 0x8000_0000;

/// Virtual address of the start of the direct map of physical memory: the
/// physical address `p` is accessible at `p + PHYS_MAP_OFFSET`. Memory handed
/// out by the frame and kernel allocators is addressed through it.
pub const PHYS_MAP_OFFSET: usize = 0x8000_0000;

/// Exclusive upper bound of user virtual addresses. Any virtual address at or
/// above it is a kernel address.
pub const USER_TOP: usize = 0x8000_0000;

// TODO: Figure out how to detect kernel stack overflows.
pub const MAIN_STACK_SIZE: usize = 2 * MB;
pub const TRAMPOLINE_HEAP_SIZE: usize = 8 * MB;

// TODO: We currently leave 8MB for the bootstrap allocator. This should be
// re-evaluated later.
pub const BOOTSTRAP_ALLOCATOR_SIZE: usize = 8 * MB;
