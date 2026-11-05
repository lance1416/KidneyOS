//! Four-level (x86_64 long mode) page tables.
//!
//! https://wiki.osdev.org/Paging#64-Bit_Paging
//!
//! This module is built on every target so that it can be unit tested on the
//! 32-bit test host; only [`PageManager::load`] and [`PageManager::is_loaded`]
//! touch the CPU, and they only exist in the x86_64 kernel. For the same reason, addresses are `u64` rather than `usize`.
//!
//! The API mirrors the i686 `PageManager`. The differences:
//!
//! - Pages can be non-executable (the NX bit). `map`, `map_range` and friends,
//!   which only take `write` and `user` like on i686, map writable pages as
//!   non-executable and read-only pages as executable ("W^X"); the `*_flags`
//!   variants take explicit [`PageFlags`].
//! - Huge pages are 2MiB, and need both addresses to be 2MiB aligned.
//! - [`PageManager::can_access`] rejects user accesses at or above [`USER_TOP`]
//!   and non-canonical addresses before looking at the page tables.

use core::{
    alloc::{Allocator, Layout},
    ptr::NonNull,
};

/// Size of a page (and of a page table).
pub const PAGE_SIZE: u64 = 0x1000;
/// Size of a huge page, mapped by a page directory entry.
pub const HUGE_PAGE_SIZE: u64 = 0x20_0000;
/// Exclusive upper bound of user virtual addresses: the end of the lower,
/// canonical half of the address space.
pub const USER_TOP: u64 = 0x0000_8000_0000_0000;

const ENTRIES: usize = 512;
const TABLE_LAYOUT: Layout = Layout::new::<PageTable>();

/// Whether `addr` is canonical, i.e. bits 63..47 are all equal. Using a
/// non-canonical address faults with #GP (not #PF).
pub const fn is_canonical(addr: u64) -> bool {
    let top = addr >> 47;
    top == 0 || top == 0x1_FFFF
}

/// The PML4, PDPT, PD and PT indices of `virt`.
// Intentional: each index is masked to 9 bits.
#[allow(clippy::cast_possible_truncation)]
const fn indices(virt: u64) -> [usize; 4] {
    [
        ((virt >> 39) & 0x1FF) as usize,
        ((virt >> 30) & 0x1FF) as usize,
        ((virt >> 21) & 0x1FF) as usize,
        ((virt >> 12) & 0x1FF) as usize,
    ]
}

/// What a mapping allows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PageFlags {
    pub write: bool,
    pub user: bool,
    pub exec: bool,
}

impl PageFlags {
    /// `write` and `user` as given; executable iff not writable.
    pub const fn w_xor_x(write: bool, user: bool) -> Self {
        Self {
            write,
            user,
            exec: !write,
        }
    }
}

/// An entry of any of the four levels of page tables.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(transparent)]
pub struct PageTableEntry(u64);

impl PageTableEntry {
    const PRESENT: u64 = 1 << 0;
    const WRITABLE: u64 = 1 << 1;
    const USER: u64 = 1 << 2;
    /// In a PDPT or PD entry: maps a huge page instead of pointing to a table.
    const HUGE: u64 = 1 << 7;
    const NO_EXECUTE: u64 = 1 << 63;
    const ADDR_MASK: u64 = 0x000F_FFFF_FFFF_F000;

    const fn new(addr: u64, flags: PageFlags, huge: bool) -> Self {
        let mut bits = (addr & Self::ADDR_MASK) | Self::PRESENT;
        if flags.write {
            bits |= Self::WRITABLE;
        }
        if flags.user {
            bits |= Self::USER;
        }
        if !flags.exec {
            bits |= Self::NO_EXECUTE;
        }
        if huge {
            bits |= Self::HUGE;
        }
        Self(bits)
    }

    pub const fn bits(self) -> u64 {
        self.0
    }

    pub const fn present(self) -> bool {
        self.0 & Self::PRESENT != 0
    }

    pub const fn writable(self) -> bool {
        self.0 & Self::WRITABLE != 0
    }

    pub const fn user(self) -> bool {
        self.0 & Self::USER != 0
    }

    pub const fn huge(self) -> bool {
        self.0 & Self::HUGE != 0
    }

    pub const fn executable(self) -> bool {
        self.0 & Self::NO_EXECUTE == 0
    }

    /// The physical address of the next level's table or of the page.
    pub const fn addr(self) -> u64 {
        self.0 & Self::ADDR_MASK
    }
}

#[repr(C, align(4096))]
struct PageTable([PageTableEntry; ENTRIES]);

/// The result of a successful page table walk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Translation {
    /// Physical address the virtual address maps to.
    pub phys: u64,
    /// What every level allows, like the MMU combines them.
    pub flags: PageFlags,
    /// Whether it is mapped by a huge page.
    pub huge: bool,
}

/// Wraps the four levels of page tables of one address space.
///
/// Page tables are allocated with `alloc`; a table the allocator returns at
/// address `a` must be at physical address `a - phys_to_alloc_addr_offset`,
/// and every physical table address `p` must be accessible at
/// `p + phys_to_alloc_addr_offset`.
#[derive(Debug)]
pub struct PageManager<A: Allocator> {
    root: NonNull<PageTable>,
    alloc: A,
    phys_to_alloc_addr_offset: usize,
}

impl<A: Allocator> PageManager<A> {
    pub fn new_in(alloc: A, phys_to_alloc_addr_offset: usize) -> Self {
        let mut res = Self {
            root: NonNull::dangling(),
            alloc,
            phys_to_alloc_addr_offset,
        };
        res.root = res.allocate_table();
        res
    }

    pub fn from_mapping_ranges_in<R: IntoIterator<Item = MappingRange>>(
        mapping_ranges: R,
        alloc: A,
        phys_to_alloc_addr_offset: usize,
    ) -> Self {
        let mut res = Self::new_in(alloc, phys_to_alloc_addr_offset);
        for range in mapping_ranges {
            // SAFETY: res has not been loaded yet.
            unsafe {
                res.map_range_flags(range.phys_start, range.virt_start, range.len, range.flags());
            }
        }
        res
    }

    /// Physical address of the PML4, as loaded into cr3.
    pub fn root_phys(&self) -> u64 {
        self.phys_of(self.root)
    }

    /// Loads these page tables into cr3.
    ///
    /// # Safety
    ///
    /// Swapping from the previously loaded page tables to these must not cause
    /// any existing pointers (including the stack, the code being run and the
    /// descriptor tables) to refer to anything they shouldn't.
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    pub unsafe fn load(&self) {
        unsafe { core::arch::asm!("mov cr3, {}", in(reg) self.root_phys(), options(nostack)) };
    }

    /// Returns whether these page tables are loaded.
    #[cfg(all(target_arch = "x86_64", target_os = "none"))]
    pub fn is_loaded(&self) -> bool {
        let cr3: u64;
        unsafe { core::arch::asm!("mov {}, cr3", out(reg) cr3, options(nomem, nostack)) };
        cr3 & PageTableEntry::ADDR_MASK == self.root_phys()
    }

    /// Maps the page at `virt_addr` to the frame at `phys_addr`; both must be
    /// `PAGE_SIZE` aligned. Writable pages are not executable, read-only pages
    /// are (see [`PageFlags::w_xor_x`]).
    ///
    /// # Safety
    ///
    /// Adding this mapping must not cause any existing pointers to refer to
    /// anything they shouldn't. If these page tables are loaded, the caller is
    /// responsible for the TLB.
    pub unsafe fn map(&mut self, phys_addr: u64, virt_addr: u64, write: bool, user: bool) {
        unsafe { self.map_flags(phys_addr, virt_addr, PageFlags::w_xor_x(write, user)) };
    }

    /// Like [`map`](Self::map), with explicit flags.
    ///
    /// # Safety
    ///
    /// Same as `map`.
    pub unsafe fn map_flags(&mut self, phys_addr: u64, virt_addr: u64, flags: PageFlags) {
        assert_eq!(phys_addr % PAGE_SIZE, 0, "phys_addr was not page-aligned");
        assert_eq!(virt_addr % PAGE_SIZE, 0, "virt_addr was not page-aligned");
        let [l4, l3, l2, l1] = Self::checked_indices(virt_addr);

        let pdpt = self.next_table(self.root, l4, flags, virt_addr);
        let pd = self.next_table(pdpt, l3, flags, virt_addr);
        let pt = self.next_table(pd, l2, flags, virt_addr);

        // SAFETY: Page tables are only reachable through this PageManager.
        let entry = unsafe { &mut (*pt.as_ptr()).0[l1] };
        assert!(
            !entry.present(),
            "virtual address {virt_addr:#X} was already mapped"
        );
        *entry = PageTableEntry::new(phys_addr, flags, false);
    }

    /// Like [`map`](Self::map), but maps a `HUGE_PAGE_SIZE` page; both
    /// addresses must be `HUGE_PAGE_SIZE` aligned.
    ///
    /// # Safety
    ///
    /// Same as `map`.
    pub unsafe fn huge_map(&mut self, phys_addr: u64, virt_addr: u64, write: bool, user: bool) {
        unsafe { self.huge_map_flags(phys_addr, virt_addr, PageFlags::w_xor_x(write, user)) };
    }

    /// Like [`huge_map`](Self::huge_map), with explicit flags.
    ///
    /// # Safety
    ///
    /// Same as `map`.
    pub unsafe fn huge_map_flags(&mut self, phys_addr: u64, virt_addr: u64, flags: PageFlags) {
        assert_eq!(
            phys_addr % HUGE_PAGE_SIZE,
            0,
            "phys_addr was not huge-page-aligned"
        );
        assert_eq!(
            virt_addr % HUGE_PAGE_SIZE,
            0,
            "virt_addr was not huge-page-aligned"
        );
        let [l4, l3, l2, _] = Self::checked_indices(virt_addr);

        let pdpt = self.next_table(self.root, l4, flags, virt_addr);
        let pd = self.next_table(pdpt, l3, flags, virt_addr);

        // SAFETY: Page tables are only reachable through this PageManager.
        let entry = unsafe { &mut (*pd.as_ptr()).0[l2] };
        assert!(
            !entry.present(),
            "virtual address {virt_addr:#X} was already mapped"
        );
        *entry = PageTableEntry::new(phys_addr, flags, true);
    }

    /// Maps `virt_start..virt_start + len` to `phys_start..phys_start + len`,
    /// using huge pages where both addresses allow it. Both addresses and `len`
    /// must be `PAGE_SIZE` aligned.
    ///
    /// # Safety
    ///
    /// Same as `map`.
    pub unsafe fn map_range(
        &mut self,
        phys_start: u64,
        virt_start: u64,
        len: u64,
        write: bool,
        user: bool,
    ) {
        unsafe {
            self.map_range_flags(phys_start, virt_start, len, PageFlags::w_xor_x(write, user));
        }
    }

    /// Like [`map_range`](Self::map_range), with explicit flags.
    ///
    /// # Safety
    ///
    /// Same as `map`.
    pub unsafe fn map_range_flags(
        &mut self,
        phys_start: u64,
        virt_start: u64,
        len: u64,
        flags: PageFlags,
    ) {
        assert_eq!(phys_start % PAGE_SIZE, 0, "phys_start was not page-aligned");
        assert_eq!(virt_start % PAGE_SIZE, 0, "virt_start was not page-aligned");
        assert_eq!(len % PAGE_SIZE, 0, "len was not a multiple of PAGE_SIZE");

        let mut offset = 0;
        while offset < len {
            let (phys, virt) = (phys_start + offset, virt_start + offset);
            unsafe {
                if phys % HUGE_PAGE_SIZE == 0
                    && virt % HUGE_PAGE_SIZE == 0
                    && len - offset >= HUGE_PAGE_SIZE
                {
                    self.huge_map_flags(phys, virt, flags);
                    offset += HUGE_PAGE_SIZE;
                } else {
                    self.map_flags(phys, virt, flags);
                    offset += PAGE_SIZE;
                }
            }
        }
    }

    /// Like `map_range` with `phys_start` and `virt_start` both `start`.
    ///
    /// # Safety
    ///
    /// Same as `map`.
    pub unsafe fn id_map_range(&mut self, start: u64, len: u64, write: bool, user: bool) {
        unsafe { self.map_range(start, start, len, write, user) };
    }

    /// Walks the page tables for `virt`, like the MMU does.
    pub fn translate(&self, virt: u64) -> Option<Translation> {
        if !is_canonical(virt) {
            return None;
        }
        let indices = indices(virt);
        let mut table = self.root;
        let mut flags = PageFlags {
            write: true,
            user: true,
            exec: true,
        };
        for (level, &index) in indices.iter().enumerate() {
            // SAFETY: Page tables are only reachable through this PageManager.
            let entry = unsafe { (*table.as_ptr()).0[index] };
            if !entry.present() {
                return None;
            }
            flags.write &= entry.writable();
            flags.user &= entry.user();
            flags.exec &= entry.executable();

            let page_size = match level {
                3 => Some(PAGE_SIZE),
                2 if entry.huge() => Some(HUGE_PAGE_SIZE),
                _ => None,
            };
            if let Some(page_size) = page_size {
                return Some(Translation {
                    phys: entry.addr() + virt % page_size,
                    flags,
                    huge: page_size == HUGE_PAGE_SIZE,
                });
            }
            table = self.table_at(entry.addr());
        }
        unreachable!("the last level always returns")
    }

    /// Returns whether `pointer` is valid for reads if `write = false`, and
    /// writes if `write = true`. If `user = true`, the access must also be
    /// allowed from user mode, which is never the case at or above
    /// [`USER_TOP`] (whatever the page tables say).
    pub fn can_access(&self, pointer: u64, write: bool, user: bool) -> bool {
        if user && pointer >= USER_TOP {
            return false;
        }
        self.translate(pointer)
            .is_some_and(|t| (!write || t.flags.write) && (!user || t.flags.user))
    }

    /// Returns whether all of `pointer..pointer + count` is accessible, like
    /// [`can_access`](Self::can_access).
    pub fn can_access_range(&self, pointer: u64, count: u64, write: bool, user: bool) -> bool {
        let Some(end) = pointer.checked_add(count) else {
            return false;
        };
        if user && end > USER_TOP {
            return false;
        }
        let start = pointer - pointer % PAGE_SIZE;
        let mut page = start;
        while page < end {
            if !self.can_access(page, write, user) {
                return false;
            }
            page += PAGE_SIZE;
        }
        true
    }

    /// Returns true if `pointer` is mapped.
    pub fn is_mapped(&self, pointer: u64) -> bool {
        self.can_access(pointer, false, false)
    }

    /// Returns true if `count` bytes from `pointer` are mapped.
    pub fn is_range_mapped(&self, pointer: u64, count: u64) -> bool {
        self.can_access_range(pointer, count, false, false)
    }

    /// Returns true if `pointer` is writeable.
    pub fn is_writeable(&self, pointer: u64) -> bool {
        self.can_access(pointer, true, false)
    }

    /// Returns true if `count` bytes from `pointer` are writeable.
    pub fn is_range_writeable(&self, pointer: u64, count: u64) -> bool {
        self.can_access_range(pointer, count, true, false)
    }

    fn checked_indices(virt: u64) -> [usize; 4] {
        assert!(
            is_canonical(virt),
            "virtual address {virt:#X} is not canonical"
        );
        indices(virt)
    }

    fn allocate_table(&mut self) -> NonNull<PageTable> {
        let Ok(table) = self.alloc.allocate(TABLE_LAYOUT) else {
            panic!("allocation failed");
        };
        let table = table.cast::<PageTable>();
        // SAFETY: Freshly allocated with the layout of a PageTable.
        unsafe { table.as_ptr().write_bytes(0, 1) };
        table
    }

    fn phys_of(&self, table: NonNull<PageTable>) -> u64 {
        (table.as_ptr() as usize - self.phys_to_alloc_addr_offset) as u64
    }

    fn table_at(&self, phys: u64) -> NonNull<PageTable> {
        let phys = usize::try_from(phys).expect("page tables are allocated in addressable memory");
        NonNull::new((phys + self.phys_to_alloc_addr_offset) as *mut PageTable)
            .expect("page table at a null address")
    }

    /// Returns the table that `table[index]` points to, allocating it if
    /// needed, and allows `flags` in that entry.
    fn next_table(
        &mut self,
        table: NonNull<PageTable>,
        index: usize,
        flags: PageFlags,
        virt: u64,
    ) -> NonNull<PageTable> {
        // SAFETY: Page tables are only reachable through this PageManager.
        let entry = unsafe { (*table.as_ptr()).0[index] };
        let next = if entry.present() {
            assert!(
                !entry.huge(),
                "virtual address {virt:#X} was already mapped by a huge page"
            );
            entry.addr()
        } else {
            let next = self.allocate_table();
            self.phys_of(next)
        };
        // Like on i686, an access must be allowed at every level, so the upper
        // levels allow anything a mapping below them needs. Execution is only
        // restricted in the last level.
        let allowed = PageFlags {
            write: flags.write || (entry.present() && entry.writable()),
            user: flags.user || (entry.present() && entry.user()),
            exec: true,
        };
        // SAFETY: As above.
        unsafe { (*table.as_ptr()).0[index] = PageTableEntry::new(next, allowed, false) };
        self.table_at(next)
    }

    /// Frees `table` and the tables below it; `level` is 4 for the PML4.
    fn free_table(&mut self, table: NonNull<PageTable>, level: u8) {
        if level > 1 {
            for index in 0..ENTRIES {
                // SAFETY: Page tables are only reachable through this PageManager.
                let entry = unsafe { (*table.as_ptr()).0[index] };
                if entry.present() && !entry.huge() {
                    self.free_table(self.table_at(entry.addr()), level - 1);
                }
            }
        }
        // SAFETY: Allocated by allocate_table with this allocator.
        unsafe { self.alloc.deallocate(table.cast(), TABLE_LAYOUT) };
    }
}

impl<A: Allocator> Drop for PageManager<A> {
    fn drop(&mut self) {
        #[cfg(all(target_arch = "x86_64", target_os = "none"))]
        assert!(!self.is_loaded(), "page manager dropped while still loaded");
        self.free_table(self.root, 4);
    }
}

/// A range for [`PageManager::from_mapping_ranges_in`].
#[derive(Clone, Copy, Debug)]
pub struct MappingRange {
    pub phys_start: u64,
    pub virt_start: u64,
    pub len: u64,
    pub write: bool,
    pub user: bool,
    pub exec: bool,
}

impl MappingRange {
    pub const fn flags(&self) -> PageFlags {
        PageFlags {
            write: self.write,
            user: self.user,
            exec: self.exec,
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::alloc::Global;

    const KERNEL_HALF: u64 = 0xFFFF_8000_0000_0000;
    const MB: u64 = 1 << 20;

    // Physical addresses are host addresses here (offset 0), so the tables
    // are ordinary heap memory; the addresses mapped are just numbers.
    fn page_manager() -> PageManager<Global> {
        PageManager::new_in(Global, 0)
    }

    fn rw(user: bool) -> PageFlags {
        PageFlags::w_xor_x(true, user)
    }

    #[test]
    fn canonical_addresses() {
        assert!(is_canonical(0));
        assert!(is_canonical(USER_TOP - 1));
        assert!(!is_canonical(USER_TOP));
        assert!(!is_canonical(0xFFFF_7FFF_FFFF_FFFF));
        assert!(is_canonical(KERNEL_HALF));
        assert!(is_canonical(u64::MAX));
    }

    #[test]
    fn map_and_translate_in_both_halves() {
        let mut pm = page_manager();
        unsafe {
            pm.map(0x5000, 0x40_0000, false, true);
            pm.map(0x6000, KERNEL_HALF + 0x1234_5000, true, false);
        }

        let user = pm.translate(0x40_0123).expect("mapped");
        assert_eq!(user.phys, 0x5123);
        assert_eq!(
            user.flags,
            PageFlags {
                write: false,
                user: true,
                exec: true
            }
        );
        assert!(!user.huge);

        let kernel = pm.translate(KERNEL_HALF + 0x1234_5FFF).expect("mapped");
        assert_eq!(kernel.phys, 0x6FFF);
        assert_eq!(
            kernel.flags,
            PageFlags {
                write: true,
                user: false,
                exec: false
            }
        );

        assert_eq!(pm.translate(0x40_1000), None);
        assert_eq!(pm.translate(KERNEL_HALF), None);
    }

    #[test]
    fn huge_pages() {
        let mut pm = page_manager();
        unsafe { pm.huge_map(0x4000_0000, KERNEL_HALF + 0x20_0000, true, false) };
        let t = pm.translate(KERNEL_HALF + 0x3F_FFFF).expect("mapped");
        assert_eq!(t.phys, 0x401F_FFFF);
        assert!(t.huge);
        assert_eq!(pm.translate(KERNEL_HALF + 0x40_0000), None);
    }

    #[test]
    fn map_range_uses_huge_pages_where_aligned() {
        let mut pm = page_manager();
        // 1MiB..5MiB: 4KiB pages up to 2MiB, one huge page, then 4KiB pages.
        unsafe { pm.map_range(MB, KERNEL_HALF + MB, 4 * MB, true, false) };
        assert!(!pm.translate(KERNEL_HALF + MB).expect("mapped").huge);
        assert!(pm.translate(KERNEL_HALF + 2 * MB).expect("mapped").huge);
        assert!(!pm.translate(KERNEL_HALF + 4 * MB).expect("mapped").huge);
        for addr in (MB..5 * MB).step_by(0x1_0000) {
            assert_eq!(pm.translate(KERNEL_HALF + addr).expect("mapped").phys, addr);
        }
        assert_eq!(pm.translate(KERNEL_HALF + 5 * MB), None);

        // Misaligned physical addresses can't use huge pages.
        unsafe { pm.map_range(0x1000, 2 * MB, 2 * MB, true, false) };
        assert!(!pm.translate(2 * MB).expect("mapped").huge);
        assert_eq!(
            pm.translate(4 * MB - 1).expect("mapped").phys,
            2 * MB + 0xFFF
        );
    }

    #[test]
    #[should_panic(expected = "already mapped")]
    fn double_map_panics() {
        let mut pm = page_manager();
        unsafe {
            pm.map(0x1000, 0x40_0000, true, false);
            pm.map(0x2000, 0x40_0000, true, false);
        }
    }

    #[test]
    #[should_panic(expected = "already mapped by a huge page")]
    fn mapping_inside_a_huge_page_panics() {
        let mut pm = page_manager();
        unsafe {
            pm.huge_map(0, 0x20_0000, true, false);
            pm.map(0x1000, 0x20_1000, true, false);
        }
    }

    #[test]
    #[should_panic(expected = "not canonical")]
    fn mapping_non_canonical_address_panics() {
        let mut pm = page_manager();
        unsafe { pm.map(0x1000, USER_TOP, true, true) };
    }

    #[test]
    fn exec_and_write_combine_across_levels() {
        let mut pm = page_manager();
        unsafe {
            // Shares all upper levels with the writable page below.
            pm.map_flags(
                0x1000,
                0x40_0000,
                PageFlags {
                    write: false,
                    user: true,
                    exec: true,
                },
            );
            pm.map_flags(0x2000, 0x40_1000, rw(true));
        }
        assert!(pm.can_access(0x40_0000, false, true));
        assert!(
            !pm.can_access(0x40_0000, true, true),
            "upgrading the upper levels doesn't leak"
        );
        assert!(pm.can_access(0x40_1000, true, true));
        assert!(pm.translate(0x40_0000).expect("mapped").flags.exec);
        assert!(!pm.translate(0x40_1000).expect("mapped").flags.exec);
    }

    #[test]
    fn user_access_requires_user_bit() {
        let mut pm = page_manager();
        unsafe {
            pm.map(0x1000, 0x40_0000, true, false);
            pm.map(0x2000, 0x40_1000, false, true);
        }
        assert!(pm.can_access(0x40_0000, true, false));
        assert!(!pm.can_access(0x40_0000, false, true));
        assert!(pm.can_access(0x40_1000, false, true));
        assert!(!pm.can_access(0x40_1000, true, true));
        assert!(!pm.can_access_range(0x40_0FF0, 0x20, false, true));
        assert!(pm.can_access_range(0x40_0FF0, 0x20, false, false));
    }

    #[test]
    fn user_access_rejected_at_or_above_user_top() {
        let mut pm = page_manager();
        // Even if (wrongly) mapped for user mode, kernel-half addresses are
        // never accessible from user mode.
        unsafe { pm.map(0x1000, KERNEL_HALF, true, true) };
        assert!(pm.can_access(KERNEL_HALF, true, false));
        assert!(!pm.can_access(KERNEL_HALF, false, true));

        // Non-canonical addresses are not accessible at all.
        assert!(!pm.can_access(USER_TOP, false, false));
        assert!(!pm.can_access(0x1234_5678_9ABC_DEF0, false, false));

        // Ranges ending past USER_TOP or wrapping around are rejected.
        unsafe { pm.map(0x2000, USER_TOP - PAGE_SIZE, true, true) };
        assert!(pm.can_access_range(USER_TOP - PAGE_SIZE, PAGE_SIZE, true, true));
        assert!(!pm.can_access_range(USER_TOP - PAGE_SIZE, PAGE_SIZE + 1, false, true));
        assert!(!pm.can_access_range(u64::MAX - 0xF, 0x20, false, false));
        assert!(pm.can_access_range(USER_TOP - 8, 0, false, true));
    }

    #[test]
    fn from_mapping_ranges() {
        let pm = PageManager::from_mapping_ranges_in(
            [
                MappingRange {
                    phys_start: 0x10_0000,
                    virt_start: KERNEL_HALF + 0x10_0000,
                    len: 0x2000,
                    write: false,
                    user: false,
                    exec: true,
                },
                MappingRange {
                    phys_start: 0,
                    virt_start: 0x4000_0000,
                    len: 4 * MB,
                    write: true,
                    user: false,
                    exec: false,
                },
            ],
            Global,
            0,
        );
        assert!(
            pm.translate(KERNEL_HALF + 0x10_1000)
                .expect("mapped")
                .flags
                .exec
        );
        assert!(pm.is_range_writeable(0x4000_0000, 4 * MB));
        assert!(!pm.is_mapped(0x4000_0000 + 4 * MB));
    }
}
