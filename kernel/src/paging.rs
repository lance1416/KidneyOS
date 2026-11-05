use alloc::alloc::Global;
use kidneyos_shared::{mem::PHYS_MAP_OFFSET, paging};

pub type PageManager<A = Global> = paging::PageManager<A>;

pub trait PageManagerDefault {
    /// An address space with only the kernel mapped.
    fn default() -> Self;
}

impl PageManagerDefault for PageManager<Global> {
    fn default() -> Self {
        // The kernel heap is in the direct map.
        #[allow(unused_mut)]
        let mut page_manager = PageManager::new_in(Global, PHYS_MAP_OFFSET);
        // The kernel's half is shared with the kernel's page tables, which
        // the host tests don't have.
        #[cfg(target_os = "none")]
        // SAFETY: The page tables are allocated from the kernel heap, which is
        // in the direct map.
        unsafe {
            crate::arch::share_kernel_half(&mut page_manager)
        };
        page_manager
    }
}
