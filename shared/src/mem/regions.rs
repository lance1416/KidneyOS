//! Selection of usable physical memory reported by the bootloader.

/// A range of physical memory, `[start, end)`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PhysRegion {
    pub start: u64,
    pub end: u64,
}

impl PhysRegion {
    pub const fn len(&self) -> u64 {
        self.end.saturating_sub(self.start)
    }

    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The part of this region within `[min, max)`, if any.
    pub fn clamp(self, min: u64, max: u64) -> Option<Self> {
        let clamped = Self {
            start: self.start.max(min),
            end: self.end.min(max),
        };
        (!clamped.is_empty()).then_some(clamped)
    }
}

/// Returns the largest of `regions` after clamping each one to `[min, max)`.
///
/// If several regions are equally large, the first one wins.
pub fn largest_region(
    regions: impl IntoIterator<Item = PhysRegion>,
    min: u64,
    max: u64,
) -> Option<PhysRegion> {
    regions
        .into_iter()
        .filter_map(|region| region.clamp(min, max))
        .fold(None, |best: Option<PhysRegion>, region| match best {
            Some(best) if best.len() >= region.len() => Some(best),
            _ => Some(region),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const MB: u64 = 1 << 20;
    const GB: u64 = 1 << 30;

    const fn region(start: u64, end: u64) -> PhysRegion {
        PhysRegion { start, end }
    }

    #[test]
    fn picks_largest() {
        let regions = [region(0, 0x9F000), region(MB, 512 * MB), region(GB, 3 * GB)];
        assert_eq!(
            largest_region(regions, MB, 4 * GB),
            Some(region(GB, 3 * GB))
        );
    }

    #[test]
    fn clamps_before_comparing() {
        // QEMU with -m 4G: most memory is below the PCI hole, the rest above
        // 4GiB. Only the first 2GiB are addressable through the direct map.
        let regions = [
            region(0, 0x9FC00),
            region(MB, 0xBFFE_0000),
            region(4 * GB, 5 * GB),
        ];
        assert_eq!(
            largest_region(regions, MB, 2 * GB),
            Some(region(MB, 2 * GB))
        );

        // A region that is large but mostly out of range loses to one that fits.
        let regions = [region(GB + 512 * MB, 4 * GB), region(MB, GB)];
        assert_eq!(largest_region(regions, MB, 2 * GB), Some(region(MB, GB)));
    }

    #[test]
    fn ties_keep_first() {
        let regions = [region(MB, 2 * MB), region(3 * MB, 4 * MB)];
        assert_eq!(largest_region(regions, 0, GB), Some(region(MB, 2 * MB)));
    }

    #[test]
    fn nothing_usable() {
        assert_eq!(largest_region([], 0, GB), None);
        assert_eq!(largest_region([region(0, MB)], MB, GB), None);
        assert_eq!(largest_region([region(2 * GB, 3 * GB)], MB, GB), None);
    }
}
