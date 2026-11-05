//! Parsing of the multiboot2 boot information.
//!
//! https://www.gnu.org/software/grub/manual/multiboot2/multiboot.html#Boot-information-format

use core::{
    ffi::{CStr, c_char},
    mem::size_of,
    ptr::{from_ref, read_unaligned},
};

/// The value that multiboot2 compatible bootloaders load into eax before
/// transferring control to the OS.
pub const EXPECTED_MAGIC: u32 = 0x36D7_6289;

#[repr(C)]
pub struct Info {
    pub total_size: u32,
    pub reserved: u32,
}

const END_TYPE: u32 = 0;
const COMMANDLINE_TYPE: u32 = 1;
const BOOT_LOADER_NAME_TYPE: u32 = 2;
const MEMORY_MAP_TYPE: u32 = 6;

#[allow(dead_code)]
#[repr(u32)]
#[repr(C)]
pub enum InfoTag {
    Commandline(CommandlineTag) = COMMANDLINE_TYPE,
    BootLoaderName(BootLoaderNameTag) = BOOT_LOADER_NAME_TYPE,
    MemoryMap(MemoryMapTag) = MEMORY_MAP_TYPE,
}

// NOTE: We can't properly represent InfoTag's native structure as a Rust type
// because Rust doesn't support unsized enum variant fields, which prevents us
// from describing tags that end with variable-sized data such as strings. See:
// https://github.com/rust-lang/rfcs/issues/1151. This is why the impls below
// are necessary.

#[repr(C)]
pub struct CommandlineTag {
    _size: u32,
    commandline_start: c_char,
}

impl From<&CommandlineTag> for &CStr {
    fn from(val: &CommandlineTag) -> Self {
        // SAFETY: multiboot guarantees that commandline tags will contain a
        // valid C string.
        unsafe { CStr::from_ptr(from_ref(val).cast::<u32>().offset(1).cast::<c_char>()) }
    }
}

#[repr(C)]
pub struct BootLoaderNameTag {
    _size: u32,
    boot_loader_name_start: c_char,
}

impl From<&BootLoaderNameTag> for &CStr {
    fn from(val: &BootLoaderNameTag) -> Self {
        // SAFETY: multiboot guarantees that boot loader name tags will contain
        // a valid C string.
        unsafe { CStr::from_ptr(from_ref(val).cast::<u32>().offset(1).cast::<c_char>()) }
    }
}

/// The memory map provided by the bootloader. It is followed by
/// `(size - 16) / entry_size` entries of `entry_size` bytes each, whose layout
/// starts with [`MemoryMapEntry`].
#[repr(C)]
pub struct MemoryMapTag {
    size: u32,
    entry_size: u32,
    _entry_version: u32,
    // Only u32 fields: a field with a larger alignment would move the variant's
    // data in InfoTag away from directly behind the type field.
}

/// Type of memory map entries that describe RAM available for use.
pub const MEMORY_AVAILABLE: u32 = 1;

#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct MemoryMapEntry {
    pub base_addr: u64,
    pub length: u64,
    pub r#type: u32,
    _reserved: u32,
}

impl MemoryMapTag {
    /// Size of the tag (including its type and size fields) before the entries.
    const HEADER_SIZE: usize = 16;

    pub fn entries(&self) -> impl Iterator<Item = MemoryMapEntry> + '_ {
        let entry_size = self.entry_size as usize;
        let count = (self.size as usize)
            .saturating_sub(Self::HEADER_SIZE)
            .checked_div(entry_size)
            .unwrap_or(0);
        // `self` starts at the tag's size field, right after its type field.
        let first = from_ref(self)
            .cast::<u8>()
            .wrapping_add(Self::HEADER_SIZE - size_of::<u32>());
        (0..count).map(move |i| {
            // SAFETY: multiboot guarantees `count` entries of `entry_size`
            // bytes follow the header, each starting with the fields of
            // MemoryMapEntry. Entries are only guaranteed to be 4-byte
            // aligned, hence the unaligned read.
            unsafe { read_unaligned(first.add(i * entry_size).cast::<MemoryMapEntry>()) }
        })
    }
}

#[repr(C)]
struct Headers {
    r#type: u32,
    size: u32,
}

impl Info {
    // Small compile-time constant; `try_from` isn't const.
    #[allow(clippy::cast_possible_truncation)]
    pub const fn iter(&self) -> InfoIterator<'_> {
        InfoIterator {
            info: self,
            offset: size_of::<Self>() as u32,
        }
    }
}

pub struct InfoIterator<'a> {
    info: &'a Info,
    offset: u32,
}

impl<'a> InfoIterator<'a> {
    const unsafe fn curr_ptr(&self) -> *const u8 {
        unsafe { from_ref(self.info).cast::<u8>().add(self.offset as usize) }
    }

    const fn curr_headers(&self) -> &Headers {
        // SAFETY: The return value of curr_ptr depends on Info, which is
        // guaranteed by multiboot to have an alignment of 64, as well as on
        // offset, which is guaranteed by both multiboot and by our checks to be
        // a multiple of 8, meaning the result of curr_ptr is guaranteed to have
        // an alignment of at least 64, which is greater than what is required
        // by Headers.
        #[allow(clippy::cast_ptr_alignment)]
        unsafe {
            &*self.curr_ptr().cast::<Headers>()
        }
    }
}

impl<'a> Iterator for InfoIterator<'a> {
    type Item = &'a InfoTag;

    fn next(&mut self) -> Option<Self::Item> {
        let curr_headers = self.curr_headers();
        let curr = match curr_headers.r#type {
            END_TYPE => return None,
            COMMANDLINE_TYPE | BOOT_LOADER_NAME_TYPE | MEMORY_MAP_TYPE => {
                // SAFETY: Same as curr_headers.
                #[allow(clippy::cast_ptr_alignment)]
                unsafe {
                    &*self.curr_ptr().cast::<InfoTag>()
                }
            }
            _ => {
                // It is UB to cast this to a variant since its discriminant is
                // not in the type definition for InfoTag, so we skip it.
                self.offset += curr_headers.size;
                self.offset = self.offset.next_multiple_of(8);
                return self.next();
            }
        };
        self.offset += curr_headers.size;
        self.offset = self.offset.next_multiple_of(8);
        Some(curr)
    }
}
