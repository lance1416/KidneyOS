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

/// Returned when a [`RegionList`] would need more than its capacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegionListFull;

/// A set of physical memory, as at most `N` disjoint, non-adjacent regions
/// sorted by address. Adding merges overlapping and adjacent regions; removing
/// may split one.
#[derive(Clone, Debug)]
pub struct RegionList<const N: usize> {
    regions: [PhysRegion; N],
    len: usize,
}

impl<const N: usize> Default for RegionList<N> {
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> RegionList<N> {
    const EMPTY: PhysRegion = PhysRegion { start: 0, end: 0 };

    pub const fn new() -> Self {
        Self {
            regions: [Self::EMPTY; N],
            len: 0,
        }
    }

    pub fn as_slice(&self) -> &[PhysRegion] {
        &self.regions[..self.len]
    }

    /// Total size of the regions in bytes.
    pub fn total_len(&self) -> u64 {
        self.as_slice().iter().map(PhysRegion::len).sum()
    }

    /// Adds `region` to the set. On error, the set is unchanged.
    pub fn add(&mut self, mut region: PhysRegion) -> Result<(), RegionListFull> {
        if region.is_empty() {
            return Ok(());
        }

        let mut out = Self::new();
        let mut placed = false;
        for &existing in self.as_slice() {
            if existing.end < region.start {
                out.push(existing)?;
            } else if region.end < existing.start {
                if !placed {
                    out.push(region)?;
                    placed = true;
                }
                out.push(existing)?;
            } else {
                // Overlapping or adjacent: merge into `region`, which can't
                // have been placed yet since the regions are sorted.
                region = PhysRegion {
                    start: region.start.min(existing.start),
                    end: region.end.max(existing.end),
                };
            }
        }
        if !placed {
            out.push(region)?;
        }

        *self = out;
        Ok(())
    }

    /// Removes `region` from the set. On error, the set is unchanged.
    pub fn remove(&mut self, region: PhysRegion) -> Result<(), RegionListFull> {
        if region.is_empty() {
            return Ok(());
        }

        let mut out = Self::new();
        for &existing in self.as_slice() {
            if existing.end <= region.start || region.end <= existing.start {
                out.push(existing)?;
                continue;
            }
            let below = PhysRegion {
                start: existing.start,
                end: region.start,
            };
            let above = PhysRegion {
                start: region.end,
                end: existing.end,
            };
            for part in [below, above] {
                if part.start < part.end {
                    out.push(part)?;
                }
            }
        }

        *self = out;
        Ok(())
    }

    fn push(&mut self, region: PhysRegion) -> Result<(), RegionListFull> {
        let slot = self.regions.get_mut(self.len).ok_or(RegionListFull)?;
        *slot = region;
        self.len += 1;
        Ok(())
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

    #[test]
    fn region_list_merges_overlapping_and_adjacent() {
        let mut list = RegionList::<4>::new();
        list.add(region(3 * MB, 4 * MB)).expect("fits");
        list.add(region(MB, 2 * MB)).expect("fits");
        assert_eq!(
            list.as_slice(),
            [region(MB, 2 * MB), region(3 * MB, 4 * MB)]
        );

        // Adjacent to the first one.
        list.add(region(2 * MB, 2 * MB + 4096)).expect("fits");
        assert_eq!(
            list.as_slice(),
            [region(MB, 2 * MB + 4096), region(3 * MB, 4 * MB)]
        );

        // Bridges both.
        list.add(region(MB + 1, 3 * MB + 1)).expect("fits");
        assert_eq!(list.as_slice(), [region(MB, 4 * MB)]);

        // Empty regions are ignored.
        list.add(region(5 * MB, 5 * MB)).expect("fits");
        assert_eq!(list.total_len(), 3 * MB);
    }

    #[test]
    fn region_list_remove_splits() {
        let mut list = RegionList::<4>::new();
        list.add(region(MB, 8 * MB)).expect("fits");
        list.add(region(GB, 2 * GB)).expect("fits");

        list.remove(region(2 * MB, 3 * MB)).expect("fits");
        assert_eq!(
            list.as_slice(),
            [
                region(MB, 2 * MB),
                region(3 * MB, 8 * MB),
                region(GB, 2 * GB)
            ]
        );

        // Covers the end of one region and the start of the next.
        list.remove(region(7 * MB, GB + MB)).expect("fits");
        assert_eq!(
            list.as_slice(),
            [
                region(MB, 2 * MB),
                region(3 * MB, 7 * MB),
                region(GB + MB, 2 * GB)
            ]
        );

        // Removing everything.
        list.remove(region(0, u64::MAX)).expect("fits");
        assert!(list.as_slice().is_empty());
    }

    #[test]
    fn region_list_capacity() {
        let mut list = RegionList::<2>::new();
        list.add(region(0, MB)).expect("fits");
        list.add(region(2 * MB, 3 * MB)).expect("fits");
        assert_eq!(list.add(region(4 * MB, 5 * MB)), Err(RegionListFull));
        // A region that merges still fits.
        list.add(region(MB, 2 * MB)).expect("fits");
        assert_eq!(list.as_slice(), [region(0, 3 * MB)]);

        let mut list = RegionList::<1>::new();
        list.add(region(0, 3 * MB)).expect("fits");
        assert_eq!(list.remove(region(MB, 2 * MB)), Err(RegionListFull));
        assert_eq!(list.as_slice(), [region(0, 3 * MB)], "unchanged on error");
    }
}
