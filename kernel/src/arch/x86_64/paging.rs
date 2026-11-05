//! Physical memory and the kernel's page tables.
//!
//! The kernel's address space has:
//!
//! - the direct map: every physical address `p` that the multiboot2 memory
//!   map mentions (plus the first 1MiB, for VGA memory) at
//!   `p + PHYS_MAP_OFFSET`, writable and not executable;
//! - the kernel image at `KERNEL_BASE + p`, with .text executable, .rodata
//!   read-only and the rest writable.
//!
//! Nothing else: unlike the boot stub's page tables, there is no identity map.

use core::{
    alloc::{AllocError, Allocator, Layout},
    ptr::NonNull,
    sync::atomic::{AtomicU64, Ordering},
};
use kidneyos_shared::{
    mem::{
        KERNEL_BASE, PAGE_FRAME_SIZE, PHYS_MAP_OFFSET,
        frames::RegionFrameAllocator,
        regions::{PhysRegion, RegionList},
    },
    multiboot2::{MEMORY_AVAILABLE, MemoryMapTag},
    paging::{PAGE_SIZE, PageFlags, PageManager},
    println,
    sizes::{GB, MB},
};
use spin::Mutex;

/// The most regions the frame allocator and the direct map keep track of.
const MAX_REGIONS: usize = 32;

/// The boot stub maps physical `0..1GiB` at `PHYS_MAP_OFFSET`; frames above
/// that can only be used once the kernel's page tables are loaded.
const BOOT_DIRECT_MAP_END: u64 = GB as u64;

static FRAMES: Mutex<RegionFrameAllocator<MAX_REGIONS>> = Mutex::new(RegionFrameAllocator::new());
/// Frames handed out must end below this, so that they are in the direct map.
static FRAME_LIMIT: AtomicU64 = AtomicU64::new(BOOT_DIRECT_MAP_END);

/// The kernel's page tables, once loaded.
static KERNEL_PAGES: Mutex<Option<KernelPages>> = Mutex::new(None);

struct KernelPages(PageManager<FrameAllocator>);

// SAFETY: The page tables are only accessed through the mutex.
unsafe impl Send for KernelPages {}

/// Returns the address that physical address `phys` is accessible at through
/// the direct map.
pub fn phys_to_virt(phys: u64) -> usize {
    usize::try_from(phys).expect("physical addresses fit in usize") + PHYS_MAP_OFFSET
}

/// Allocates page frames, addressed through the direct map. Only allocations
/// of (at most) one page are supported, and frames are never freed yet.
#[derive(Clone, Copy, Debug)]
pub struct FrameAllocator;

// SAFETY: Every frame is handed out at most once.
unsafe impl Allocator for FrameAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let page = PAGE_FRAME_SIZE;
        if layout.size() > page || layout.align() > page {
            return Err(AllocError);
        }
        let frame = FRAMES
            .lock()
            .allocate_below(FRAME_LIMIT.load(Ordering::Relaxed))
            .ok_or(AllocError)?;
        let pointer = NonNull::new(phys_to_virt(frame) as *mut u8).ok_or(AllocError)?;
        Ok(NonNull::slice_from_raw_parts(pointer, page))
    }

    unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {
        // TODO(x86_64): Free frames once there is a real frame allocator
        // (until then, only page tables, which are never freed, use this).
    }
}

/// Physical memory as found in the memory map.
pub struct PhysicalMemory {
    /// What the direct map covers.
    direct_map: RegionList<MAX_REGIONS>,
}

/// Makes the available memory in `memory_map` usable by [`FrameAllocator`],
/// except for `reserved` (the kernel image and the boot information), and
/// prints it.
pub fn init_frames(memory_map: &MemoryMapTag, reserved: &[PhysRegion]) -> PhysicalMemory {
    let mut direct_map = RegionList::new();
    // The first 1MiB has VGA memory, which isn't in the memory map.
    direct_map
        .add(PhysRegion {
            start: 0,
            end: MB as u64,
        })
        .expect("empty");

    let mut frames = FRAMES.lock();
    for entry in memory_map.entries() {
        let region = PhysRegion {
            start: entry.base_addr - entry.base_addr % PAGE_SIZE,
            end: entry
                .base_addr
                .saturating_add(entry.length)
                .next_multiple_of(PAGE_SIZE),
        };
        direct_map.add(region).expect("too many memory regions");

        if entry.r#type == MEMORY_AVAILABLE {
            println!(
                "  available: {:#014X}..{:#014X}",
                entry.base_addr,
                entry.base_addr.saturating_add(entry.length)
            );
            // Leave the first 1MiB to the BIOS and friends.
            if let Some(region) = region.clamp(MB as u64, u64::MAX) {
                frames.add_region(region).expect("too many memory regions");
            }
        }
    }
    for &region in reserved {
        frames.reserve(region).expect("too many memory regions");
    }

    println!(
        "Available memory: {} MiB in {} regions, {} MiB free for frames",
        memory_map
            .entries()
            .filter(|entry| entry.r#type == MEMORY_AVAILABLE)
            .map(|entry| entry.length)
            .sum::<u64>()
            / MB as u64,
        frames.free_regions().len(),
        frames.free_bytes() / MB as u64,
    );

    PhysicalMemory { direct_map }
}

/// Builds and loads the kernel's page tables (see the module documentation),
/// which replace the boot stub's.
///
/// # Safety
///
/// Must only be called once, after [`init_frames`], and after anything still
/// in use (the stack, the descriptor tables) has been moved off the identity
/// map.
pub unsafe fn init_kernel_page_tables(memory: &PhysicalMemory) {
    unsafe extern "C" {
        static kernel_start: u8;
        static kernel_rodata_start: u8;
        static kernel_data_start: u8;
        static kernel_end: u8;
    }
    // Only the addresses of the linker symbols are used.
    let (text, rodata, data, end) = {
        (
            &raw const kernel_start as u64,
            &raw const kernel_rodata_start as u64,
            &raw const kernel_data_start as u64,
            &raw const kernel_end as u64,
        )
    };
    let kernel_base = KERNEL_BASE as u64;
    let phys_map_offset = PHYS_MAP_OFFSET as u64;

    let mut pages = PageManager::new_in(FrameAllocator, PHYS_MAP_OFFSET);
    // SAFETY: Not loaded yet.
    unsafe {
        let data_flags = PageFlags {
            write: true,
            user: false,
            exec: false,
        };
        for region in memory.direct_map.as_slice() {
            pages.map_range_flags(
                region.start,
                region.start + phys_map_offset,
                region.len(),
                data_flags,
            );
        }

        for (start, end, flags) in [
            (
                text,
                rodata,
                PageFlags {
                    write: false,
                    user: false,
                    exec: true,
                },
            ),
            (rodata, data, PageFlags::default()),
            (data, end, data_flags),
        ] {
            pages.map_range_flags(start - kernel_base, start, end - start, flags);
        }
    }

    let direct_map_size = memory.direct_map.total_len();
    let direct_map_end = memory.direct_map.as_slice().last().map_or(0, |r| r.end);
    let identity_mapped = pages.is_mapped(0x10_0000_u64);

    // SAFETY: Everything in use is in the kernel image or the direct map, at
    // the same addresses as before.
    unsafe { pages.load() };
    FRAME_LIMIT.store(u64::MAX, Ordering::Relaxed);

    println!(
        "Paging: direct map of {} MiB (up to {direct_map_end:#X}) at {PHYS_MAP_OFFSET:#X}, \
         kernel at {KERNEL_BASE:#X}, identity map {}",
        direct_map_size / MB as u64,
        if identity_mapped {
            "still there"
        } else {
            "removed"
        },
    );

    *KERNEL_PAGES.lock() = Some(KernelPages(pages));
}

/// Maps a fresh, zeroed frame at the page containing `virt` in the kernel's
/// page tables, writable and not executable. Returns false if it's already
/// mapped or there is no free memory.
///
/// # Safety
///
/// Mapping memory at `virt` must not change what any existing pointer refers
/// to.
pub unsafe fn map_fresh_page(virt: u64) -> bool {
    let page = virt - virt % PAGE_SIZE;
    let mut kernel_pages = KERNEL_PAGES.lock();
    let Some(KernelPages(pages)) = kernel_pages.as_mut() else {
        return false;
    };
    if pages.is_mapped(page) {
        return false;
    }
    let Some(frame) = FRAMES.lock().allocate() else {
        return false;
    };
    // SAFETY: The frame was free, and the page wasn't mapped.
    unsafe {
        (phys_to_virt(frame) as *mut u8).write_bytes(0, PAGE_FRAME_SIZE);
        pages.map_flags(
            frame,
            page,
            PageFlags {
                write: true,
                user: false,
                exec: false,
            },
        );
        core::arch::asm!("invlpg [{}]", in(reg) page, options(nostack, preserves_flags));
    }
    true
}
