mod buddy_allocator;
mod dummy_allocator;
mod frame_allocator;
mod subblock_allocator;
pub mod user;
pub mod util;
pub mod vma;

use crate::interrupts::{IntrLevel, mutex_irq::hold_interrupts};
use alloc::{boxed::Box, vec};
use core::sync::atomic::AtomicBool;
use core::{
    alloc::{AllocError, GlobalAlloc, Layout},
    cell::UnsafeCell,
    mem::size_of,
    ops::Range,
    ptr,
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};
use dummy_allocator::DummyAllocatorSolution;
use frame_allocator::{CoreMapEntry, FrameAllocatorSolution, placement_algorithms::NextFit};
use kidneyos_shared::mem::{PAGE_FRAME_SIZE, PHYS_MAP_OFFSET};
use subblock_allocator::SubblockAllocatorSolution;

static FIRST_ALLOCATION: AtomicBool = AtomicBool::new(true);
static TOTAL_NUM_ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static TOTAL_NUM_DEALLOCATIONS: AtomicUsize = AtomicUsize::new(0);

const MAX_SUPPORTED_ALIGN: usize = 4096;

trait FrameAllocator {
    /// Allocates "frames_requested" number of contiguous frames
    ///
    /// This function should return a pointer to the start of the memory region on success and
    /// AllocError on failure
    fn alloc(&mut self, frames_requested: usize) -> Result<NonNull<u8>, AllocError>;

    /// Deallocates the frame or frames pointed to by "ptr_to_dealloc" according to layout
    ///
    /// This function should return the number of frames deallocated on success
    ///
    /// This function is unsafe because "ptr_to_dealloc" the caller must ensure that
    /// ptr_to_dealloc must be owned by the allocator
    unsafe fn dealloc(&mut self, ptr_to_dealloc: NonNull<u8>) -> usize;
}

// halt is used for cases where we would panic in KernelAllocator, but can't
// because doing so causes undefined behaviour as per the GlobalAlloc safety
// contract.
macro_rules! halt {
    () => {{
        super::eprintln!();
        loop {}
    }};
    ($($arg:tt)*) => {{
        kidneyos_shared::eprintln!($($arg)*);
        loop {}
    }};
}

/// The frames the very first allocation (the core map, see
/// [`KernelAllocator::init`]) takes for `size` bytes with alignment `align`.
fn first_allocation_frames(size: usize, align: usize) -> usize {
    (size + align).div_ceil(PAGE_FRAME_SIZE)
}

/// How many of `total_frames` free frames can be handed out, given that the
/// core map, with an entry for each of them, comes out of `total_frames` too.
fn usable_frames(total_frames: usize) -> usize {
    let core_map_frames = |frames: usize| {
        first_allocation_frames(
            frames * size_of::<CoreMapEntry>(),
            align_of::<CoreMapEntry>(),
        )
    };
    // An estimate which ignores that the core map takes up whole frames, so
    // it is at most a few frames off.
    let mut frames = total_frames * PAGE_FRAME_SIZE / (size_of::<CoreMapEntry>() + PAGE_FRAME_SIZE);
    while frames > 0 && frames + core_map_frames(frames) > total_frames {
        frames -= 1;
    }
    frames
}

enum KernelAllocatorState {
    DeInitialized,
    SetupState {
        dummy_allocator: DummyAllocatorSolution,
    },
    Initialized {
        subblock_allocator: SubblockAllocatorSolution<FrameAllocatorSolution<NextFit>>,
    },
}

/// The kernel's allocator. There is one CPU, so what keeps threads and
/// interrupt handlers from using it at the same time is that everything that
/// uses `state` holds interrupts off.
///
/// Every method takes `&self`, like [`GlobalAlloc`]'s: the global allocator
/// can be called from inside them (`init` allocates the core map with `vec!`),
/// so a `&mut KernelAllocator`, or a `&mut` into `state` held across an
/// allocation, would alias the one `GlobalAlloc` makes. Each method borrows
/// `state` mutably only while it doesn't allocate.
pub struct KernelAllocator {
    state: UnsafeCell<KernelAllocatorState>,
}

// SAFETY: There is one CPU, and everything that uses `state` holds interrupts
// off, so it is never used from two places at once.
unsafe impl Sync for KernelAllocator {}

impl KernelAllocator {
    pub const fn new() -> KernelAllocator {
        Self {
            state: UnsafeCell::new(KernelAllocatorState::SetupState {
                dummy_allocator: DummyAllocatorSolution::new_in(0, 0),
            }),
        }
    }

    /// Initialize the kernel allocator
    ///
    /// "memory" is the range of physical memory available for frames, i.e.
    /// the largest usable region from the bootloader's memory map. Memory used
    /// by the kernel image, its stack and the bootstrap heap is excluded here.
    ///
    /// # Safety
    ///
    /// This function can only be called when the allocator is uninitialized.
    pub unsafe fn init(&self, memory: Range<usize>) {
        let _guard = hold_interrupts(IntrLevel::IntrOff);

        // The exclusive max address is the end of the region in the direct map.
        let frames_ceil_address = PHYS_MAP_OFFSET + memory.end;

        // The region is the largest one of the boot frame allocator (see
        // RegionFrameAllocator::take_largest), which excludes everything in
        // use already.
        let frames_base_address = PHYS_MAP_OFFSET + memory.start;
        assert!(
            frames_base_address < frames_ceil_address,
            "no usable memory after the kernel"
        );

        {
            // SAFETY: Interrupts are off, and this borrow ends before the
            // allocation below.
            let KernelAllocatorState::SetupState { dummy_allocator } =
                (unsafe { &mut *self.state.get() })
            else {
                // We can panic here because the kernel hasn't been initialized yet
                panic!("[PANIC]: init called while kernel allocator was already initialized");
            };

            // Check to see if dummy_allocator initialized properly (both start and end should be zero)
            let start = dummy_allocator.get_start_address();
            let end = dummy_allocator.get_end_address();
            assert_eq!(start, 0);
            assert_eq!(end, 0);

            // Set the proper start and end addresses
            dummy_allocator.set_start_address(frames_base_address);
            dummy_allocator.set_end_address(frames_ceil_address);
        }

        let num_frames_in_system =
            usable_frames((frames_ceil_address - frames_base_address) / PAGE_FRAME_SIZE);

        // This should ALWAYS be the first global allocation to take place - should use dummy allocator
        // (through GlobalAlloc::alloc, which moves its start address past the
        // core map).
        let core_map: Box<[CoreMapEntry]> =
            vec![CoreMapEntry::default(); num_frames_in_system].into_boxed_slice();

        // SAFETY: Interrupts are off, and nothing else borrows `state` now.
        let KernelAllocatorState::SetupState { dummy_allocator } = (unsafe { &*self.state.get() })
        else {
            panic!("[PANIC]: the core map wasn't allocated with the dummy allocator");
        };
        let frames_start_address = dummy_allocator.get_start_address();

        // Check that the dummy allocator actually updated its internal state
        // I.e. the start address should have moved to accommodate Coremap Entries
        assert_ne!(frames_base_address, frames_start_address);

        let frame_allocator = FrameAllocatorSolution::<NextFit>::new(
            NonNull::slice_from_raw_parts(
                NonNull::new(frames_start_address as *mut u8)
                    .expect("Could not create NonNull pointer"),
                PAGE_FRAME_SIZE * num_frames_in_system,
            ),
            core_map,
        );

        // SAFETY: Interrupts are off, and nothing else borrows `state` now.
        unsafe {
            *self.state.get() = KernelAllocatorState::Initialized {
                subblock_allocator: SubblockAllocatorSolution::new(frame_allocator),
            };
        }
    }

    pub fn frame_alloc(&self, frames: usize) -> Result<NonNull<u8>, AllocError> {
        let _guard = hold_interrupts(IntrLevel::IntrOff);
        // SAFETY: Interrupts are off, and the frame allocator doesn't use the
        // global allocator.
        let KernelAllocatorState::Initialized { subblock_allocator } =
            (unsafe { &mut *self.state.get() })
        else {
            return Err(AllocError);
        };

        subblock_allocator.get_frame_allocator().alloc(frames)
    }

    pub fn frame_dealloc(&self, ptr: NonNull<u8>) {
        let _guard = hold_interrupts(IntrLevel::IntrOff);
        // SAFETY: As in `frame_alloc`.
        let KernelAllocatorState::Initialized { subblock_allocator } =
            (unsafe { &mut *self.state.get() })
        else {
            halt!("[KERNEL ALLOCATOR]: Dealloc called on DeInitialized or SetupState kernel");
        };

        unsafe { subblock_allocator.get_frame_allocator().dealloc(ptr) };
    }

    pub fn deinit(&self) {
        let _guard = hold_interrupts(IntrLevel::IntrOff);
        // SAFETY: Interrupts are off, and the subblock allocator's deinit
        // doesn't use the global allocator.
        let KernelAllocatorState::Initialized {
            subblock_allocator, ..
        } = (unsafe { &mut *self.state.get() })
        else {
            panic!("[KERNEL ALLOCATOR]: deinit called before initialization of kernel allocator");
        };

        let mut incorrect_num_allocs = false;

        if TOTAL_NUM_ALLOCATIONS.load(Ordering::Relaxed)
            != TOTAL_NUM_DEALLOCATIONS.load(Ordering::Relaxed)
        {
            incorrect_num_allocs = true;
        }

        subblock_allocator.deinit();

        if incorrect_num_allocs {
            halt!("[KERNEL ALLOCATOR]: Leaks detected");
        }

        // SAFETY: As above; the borrow of the subblock allocator has ended.
        unsafe { *self.state.get() = KernelAllocatorState::DeInitialized };
    }
}

impl Default for KernelAllocator {
    fn default() -> Self {
        Self::new()
    }
}

// SAFETY:
//
// - We don't panic.
// - We don't mess up layout calculations.
// - We never rely on allocations happening.
unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _guard = hold_interrupts(IntrLevel::IntrOff);
        unsafe {
            if FIRST_ALLOCATION.load(Ordering::Relaxed) {
                // If we are here, it should be the dummy allocator doing the allocation
                let KernelAllocatorState::SetupState { dummy_allocator } = &mut *self.state.get()
                else {
                    halt!("[KERNEL ALLOCATOR]: Kernel initialized before Coremap Entries created")
                };

                let size = layout.size();
                let align = layout.align();

                // The alignment of the layout should never be larger than the size of a page
                if align > MAX_SUPPORTED_ALIGN {
                    return ptr::null_mut();
                }

                let num_frames_requested = first_allocation_frames(size, align);

                let Ok(region) = dummy_allocator.alloc(num_frames_requested) else {
                    halt!(
                        "[KERNEL ALLOCATOR]: Unable to allocate memory according to provided layout in DummyAllocator"
                    );
                };

                // We should never use dummy allocator again
                FIRST_ALLOCATION.store(false, Ordering::Relaxed);

                region.as_ptr().cast::<u8>()
            } else {
                let KernelAllocatorState::Initialized {
                    subblock_allocator, ..
                } = &mut *self.state.get()
                else {
                    halt!("[KERNEL ALLOCATOR]: Allocation requested before kernel is Initialized");
                };

                // The alignment of the layout should never be larger than the size of a page
                if layout.align() > MAX_SUPPORTED_ALIGN {
                    return ptr::null_mut();
                }

                // Allocate using subblock allocator
                let ret_ptr = match subblock_allocator.allocate(layout) {
                    Ok(t) => t,
                    Err(_) => halt!(
                        "[KERNEL ALLOCATOR]: Unable to allocate memory according to provided layout in SubblockAllocator"
                    ),
                };

                TOTAL_NUM_ALLOCATIONS.fetch_add(1, Ordering::Relaxed);

                ret_ptr
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let _guard = hold_interrupts(IntrLevel::IntrOff);
        unsafe {
            let KernelAllocatorState::Initialized {
                subblock_allocator, ..
            } = &mut *self.state.get()
            else {
                halt!(
                    "[KERNEL ALLOCATOR]: dealloc called before initialization of kernel allocator"
                );
            };

            subblock_allocator.deallocate(ptr, layout);

            TOTAL_NUM_DEALLOCATIONS.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usable_frames_and_core_map_fit() {
        let core_map_frames = |frames: usize| {
            first_allocation_frames(
                frames * size_of::<CoreMapEntry>(),
                align_of::<CoreMapEntry>(),
            )
        };
        for total_frames in (1..20_000).chain([1 << 20, (1 << 20) + 1, 1 << 24]) {
            let frames = usable_frames(total_frames);
            assert!(frames + core_map_frames(frames) <= total_frames);
            assert!(frames + 1 + core_map_frames(frames + 1) > total_frames);
        }
    }
}
