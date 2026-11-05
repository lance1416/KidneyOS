//! Allocation of physical page frames from the memory reported by the
//! bootloader.

use super::regions::{PhysRegion, RegionList, RegionListFull, largest_region};

const FRAME_SIZE: u64 = super::PAGE_FRAME_SIZE as u64;

/// Hands out physical page frames from a set of free regions, lowest address
/// first.
///
/// This is meant for the kernel's own early allocations (page tables, stacks),
/// which are never freed. Memory that should be managed by the frame allocator
/// students implement is handed over as one contiguous region with
/// [`take_largest`](Self::take_largest), since that interface only supports a
/// single region.
#[derive(Clone, Debug, Default)]
pub struct RegionFrameAllocator<const N: usize> {
    free: RegionList<N>,
}

impl<const N: usize> RegionFrameAllocator<N> {
    pub const fn new() -> Self {
        Self {
            free: RegionList::new(),
        }
    }

    /// Makes the whole frames within `region` available.
    pub fn add_region(&mut self, region: PhysRegion) -> Result<(), RegionListFull> {
        let start = region.start.next_multiple_of(FRAME_SIZE);
        let end = region.end - region.end % FRAME_SIZE;
        if start >= end {
            return Ok(());
        }
        self.free.add(PhysRegion { start, end })
    }

    /// Makes every frame that overlaps `region` unavailable, e.g. because the
    /// kernel image or the bootloader's data is there.
    pub fn reserve(&mut self, region: PhysRegion) -> Result<(), RegionListFull> {
        let start = region.start - region.start % FRAME_SIZE;
        let end = region.end.saturating_add(FRAME_SIZE - 1);
        let end = end - end % FRAME_SIZE;
        self.free.remove(PhysRegion { start, end })
    }

    /// Allocates the lowest free frame that ends at or below `limit`, and
    /// returns its physical address.
    pub fn allocate_below(&mut self, limit: u64) -> Option<u64> {
        let frame = self
            .free
            .as_slice()
            .iter()
            .map(|region| region.start)
            .find(|&start| start.saturating_add(FRAME_SIZE) <= limit)?;
        self.free
            .remove(PhysRegion {
                start: frame,
                end: frame + FRAME_SIZE,
            })
            .ok()?;
        Some(frame)
    }

    /// Allocates the lowest free frame, and returns its physical address.
    pub fn allocate(&mut self) -> Option<u64> {
        self.allocate_below(u64::MAX)
    }

    /// Removes the largest free region (the first one, if several are equally
    /// large) and returns it.
    pub fn take_largest(&mut self) -> Option<PhysRegion> {
        let largest = largest_region(self.free.as_slice().iter().copied(), 0, u64::MAX)?;
        self.free.remove(largest).ok()?;
        Some(largest)
    }

    /// The free regions, sorted by address.
    pub fn free_regions(&self) -> &[PhysRegion] {
        self.free.as_slice()
    }

    /// The number of free bytes.
    pub fn free_bytes(&self) -> u64 {
        self.free.total_len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1 << 20;
    const GB: u64 = 1 << 30;

    const fn region(start: u64, end: u64) -> PhysRegion {
        PhysRegion { start, end }
    }

    /// Roughly what QEMU reports with -m 4G, with the kernel at 1MiB.
    fn qemu_like() -> RegionFrameAllocator<8> {
        let mut frames = RegionFrameAllocator::new();
        frames.add_region(region(MB, 0xBFFE_0000)).expect("fits");
        frames.add_region(region(4 * GB, 5 * GB)).expect("fits");
        frames.reserve(region(MB, 0x0012_3456)).expect("fits");
        frames
    }

    #[test]
    fn regions_are_page_aligned_inwards_and_reservations_outwards() {
        let mut frames = RegionFrameAllocator::<4>::new();
        frames.add_region(region(0x1001, 0x5FFF)).expect("fits");
        assert_eq!(frames.free_regions(), [region(0x2000, 0x5000)]);
        frames.reserve(region(0x2FFF, 0x3001)).expect("fits");
        assert_eq!(
            frames.free_regions(),
            [region(0x4000, 0x5000)],
            "both frames touched by the reservation are gone"
        );

        // Regions smaller than a frame are ignored.
        frames.add_region(region(0x6001, 0x6FFF)).expect("fits");
        assert_eq!(frames.free_bytes(), 0x1000);
    }

    #[test]
    fn allocates_lowest_frames_first_across_regions() {
        let mut frames = RegionFrameAllocator::<4>::new();
        frames.add_region(region(0x1000, 0x3000)).expect("fits");
        frames
            .add_region(region(0x10_0000, 0x10_1000))
            .expect("fits");
        assert_eq!(frames.allocate(), Some(0x1000));
        assert_eq!(frames.allocate(), Some(0x2000));
        assert_eq!(frames.allocate(), Some(0x10_0000));
        assert_eq!(frames.allocate(), None);
    }

    #[test]
    fn allocate_below_skips_frames_above_the_limit() {
        let mut frames = qemu_like();
        let first = frames.allocate_below(GB).expect("free frame");
        assert_eq!(
            first, 0x0012_4000,
            "first frame after the reserved kernel image"
        );

        let mut frames = RegionFrameAllocator::<4>::new();
        frames.add_region(region(4 * GB, 5 * GB)).expect("fits");
        assert_eq!(frames.allocate_below(GB), None);
        assert_eq!(frames.allocate_below(4 * GB + 0x1000), Some(4 * GB));
    }

    #[test]
    fn take_largest_removes_it() {
        let mut frames = qemu_like();
        let before = frames.free_bytes();
        let largest = frames.take_largest().expect("free memory");
        assert_eq!(largest, region(0x0012_4000, 0xBFFE_0000));
        assert_eq!(frames.free_bytes(), before - largest.len());
        assert_eq!(frames.free_regions(), [region(4 * GB, 5 * GB)]);
    }
}
