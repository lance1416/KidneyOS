use nom::bytes::complete::{tag, take};
use nom::combinator::{map_opt, verify};
use nom::error::Error;
use nom::number::Endianness;
use nom::number::complete::{u8, u16, u32, u64};
use nom::{IResult, Parser};

use alloc::vec::Vec;

// Endianness of the fields within the Elf file.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ElfEndianness {
    Little,
    Big,
}

impl ElfEndianness {
    fn to_nom(self) -> Endianness {
        match self {
            ElfEndianness::Little => Endianness::Little,
            ElfEndianness::Big => Endianness::Big,
        }
    }
}

// Intended purpose for the ELF file.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ElfUsage {
    // Static Library
    Relocatable,
    // Executable
    Executable,
    // Dynamically Linked Library
    Shared,
    // Core Dump (from a crash)
    Core,
}

// Common Architectures from https://wiki.osdev.org/ELF
// Our host architecture is x86.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ElfArchitecture {
    // No architecture specified.
    Generic,

    Sparc,
    Mips,
    PowerPC,
    RiscV,
    Ia64,
    X86,
    X8664,
    Arm,
    Arm64,
}

/// Whether the file is a 32- or 64-bit ELF (EI_CLASS), which decides the
/// size of addresses and offsets, and the layout of the program headers.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ElfClass {
    Elf32,
    Elf64,
}

impl ElfClass {
    /// Parses an address or offset field: 4 bytes in 32-bit ELFs, 8 bytes in
    /// 64-bit ones.
    fn word(self, endian: Endianness) -> impl Fn(&[u8]) -> IResult<&[u8], u64> {
        move |bytes| match self {
            ElfClass::Elf32 => u32(endian).map(u64::from).parse(bytes),
            ElfClass::Elf64 => u64(endian).parse(bytes),
        }
    }
}

// 32- and 64-bit ELFs; addresses and offsets are u64 for both.
// Not every parsed field is used (yet).
#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub struct ElfHeader {
    pub class: ElfClass,
    pub endianness: ElfEndianness,
    pub header_version: u8,
    pub abi: u8,
    pub usage: ElfUsage,
    pub architecture: ElfArchitecture,
    pub elf_version: u32,
    pub program_entry: u64,
    pub program_headers_offset: u64,
    pub section_headers_offset: u64,
    pub flags: u32,
    pub elf_header_size: u16,
    pub program_header_entry_size: u16,
    pub program_header_count: u16,
    pub section_header_entry_size: u16,
    pub section_header_count: u16,
    pub section_header_index: u16,
}

impl ElfHeader {
    pub fn parse(bytes: &[u8]) -> IResult<&[u8], ElfHeader> {
        let (bytes, _) = tag(&b"\x7FELF"[..]).parse(bytes)?;

        let (bytes, class) = map_opt(u8, |value| match value {
            1 => Some(ElfClass::Elf32),
            2 => Some(ElfClass::Elf64),
            _ => None,
        })
        .parse(bytes)?;

        // Endianness seems to reflect the endianness of the fields, not only the architecture
        let (bytes, endianness) = map_opt(u8, |value| match value {
            1 => Some(ElfEndianness::Little),
            2 => Some(ElfEndianness::Big),
            _ => None,
        })
        .parse(bytes)?;

        let endian = endianness.to_nom();

        let (bytes, header_version) = u8(bytes)?;
        let (bytes, abi) = u8(bytes)?;

        let (bytes, _) = take(8usize).parse(bytes)?;

        let (bytes, usage) = map_opt(u16(endian), |value| match value {
            1 => Some(ElfUsage::Relocatable),
            2 => Some(ElfUsage::Executable),
            3 => Some(ElfUsage::Shared),
            4 => Some(ElfUsage::Core),
            _ => None,
        })
        .parse(bytes)?;

        let (bytes, architecture) = map_opt(u16(endian), |value| match value {
            0x00 => Some(ElfArchitecture::Generic),
            0x02 => Some(ElfArchitecture::Sparc),
            0x03 => Some(ElfArchitecture::X86),
            0x08 => Some(ElfArchitecture::Mips),
            0x14 => Some(ElfArchitecture::PowerPC),
            0x28 => Some(ElfArchitecture::Arm),
            0x32 => Some(ElfArchitecture::Ia64),
            0x3E => Some(ElfArchitecture::X8664),
            0xB7 => Some(ElfArchitecture::Arm64),
            0xF3 => Some(ElfArchitecture::RiscV),
            _ => None, // Could be Some(ElfArchitecture::Generic)
        })
        .parse(bytes)?;

        let (bytes, elf_version) = u32(endian).parse(bytes)?;
        let word = class.word(endian);
        let (bytes, program_entry) = word(bytes)?;
        let (bytes, program_headers_offset) = word(bytes)?;
        let (bytes, section_headers_offset) = word(bytes)?;

        let (bytes, flags) = u32(endian).parse(bytes)?;

        let (bytes, elf_header_size) = u16(endian).parse(bytes)?;
        let (bytes, program_header_entry_size) = u16(endian).parse(bytes)?;
        let (bytes, program_header_count) = u16(endian).parse(bytes)?;
        let (bytes, section_header_entry_size) = u16(endian).parse(bytes)?;
        let (bytes, section_header_count) = u16(endian).parse(bytes)?;
        let (bytes, section_header_index) = u16(endian).parse(bytes)?;

        Ok((
            bytes,
            ElfHeader {
                class,
                endianness,
                header_version,
                abi,
                usage,
                architecture,
                elf_version,
                program_entry,
                program_headers_offset,
                section_headers_offset,
                flags,
                elf_header_size,
                program_header_entry_size,
                program_header_count,
                section_header_entry_size,
                section_header_count,
                section_header_index,
            },
        ))
    }
}

// Intended purpose for this program segment type.
// We only really care about the "Load" type, but the other types can carry useful information.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ElfProgramType {
    Ignore,
    Load,
    Dynamic,
    Interpret,
    Note,
    ProgramHeaderTable,
    OsSpecific(u32),
}

// Not every parsed field is used (yet).
#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub struct ElfProgramHeader<'a> {
    pub program_type: ElfProgramType,

    // Offset of the program data within the file.
    pub file_offset: u64,

    // Mount address, virtual address.
    pub virtual_address: u64,

    // Typically ignored, unless our operating system cares about physical addresses.
    // I find this is typically equal to the virtual_address field.
    pub physical_address: u64,

    // Content of the program section.
    pub data: &'a [u8], // avoid copying large sections of the ELF

    // Size that needs to be allocated, should be strictly greater than data.
    pub memory_size: u64,

    // Memory access flags.
    pub executable: bool,
    pub writable: bool,
    pub readable: bool,

    pub alignment: u64,
}

impl<'a> ElfProgramHeader<'a> {
    // Parses a Program Header for a segment.
    //  `bytes` is a slice pointing to the start of the ProgramHeader.
    //  `endian` refers to the Endianness of the fields within the header (from the elf header).
    //  `full_file` is a slice that contains the whole file. Used to grab a slice to segment data.
    pub fn parse(
        bytes: &'a [u8],
        class: ElfClass,
        endian: Endianness,
        full_file: &'a [u8],
    ) -> IResult<&'a [u8], Self> {
        let (bytes, program_type) = map_opt(u32(endian), |value| match value {
            0 => Some(ElfProgramType::Ignore),
            1 => Some(ElfProgramType::Load),
            2 => Some(ElfProgramType::Dynamic),
            3 => Some(ElfProgramType::Interpret),
            4 => Some(ElfProgramType::Note),
            // Shared library information (reserved) and thread-local storage
            // templates, which KidneyOS doesn't support.
            5 | 7 => Some(ElfProgramType::Ignore),
            6 => Some(ElfProgramType::ProgramHeaderTable),
            0x60000000.. => Some(ElfProgramType::OsSpecific(value)), // OS Specific Headers
            _ => None,
        })
        .parse(bytes)?;

        // In 64-bit ELFs, the flags are the second field (to keep the 8-byte
        // fields aligned); in 32-bit ones, the seventh.
        let (bytes, flags64) = match class {
            ElfClass::Elf32 => (bytes, 0),
            ElfClass::Elf64 => u32(endian).parse(bytes)?,
        };
        let word = class.word(endian);
        let (bytes, file_offset) = word(bytes)?;
        let (bytes, virtual_address) = word(bytes)?;
        let (bytes, physical_address) = word(bytes)?;
        let (bytes, file_size) = word(bytes)?;
        // Protection, to make sure we map enough memory to contain the file data.
        let (bytes, memory_size) = verify(&word, |size| file_size <= *size).parse(bytes)?;
        let (bytes, flags) = match class {
            ElfClass::Elf32 => u32(endian).parse(bytes)?,
            ElfClass::Elf64 => (bytes, flags64),
        };
        let (bytes, alignment) = word(bytes)?;

        let executable = flags & 1 != 0;
        let writable = flags & 2 != 0;
        let readable = flags & 4 != 0;

        let (data_start, _) = take(to_usize(file_offset, full_file)?).parse(full_file)?;
        let (_, data) = take(to_usize(file_size, full_file)?).parse(data_start)?;

        Ok((
            bytes,
            ElfProgramHeader {
                program_type,
                file_offset,
                virtual_address,
                physical_address,
                data,
                memory_size,
                executable,
                writable,
                readable,
                alignment,
            },
        ))
    }
}

/// Converts an offset or size from the file to `usize`, failing the parse if
/// it doesn't fit (it couldn't be within the file then anyway).
fn to_usize(value: u64, input: &[u8]) -> Result<usize, nom::Err<Error<&[u8]>>> {
    usize::try_from(value)
        .map_err(|_| nom::Err::Failure(Error::new(input, nom::error::ErrorKind::TooLarge)))
}

#[derive(Clone, Debug)]
pub struct Elf<'a> {
    // Contains elf metadata.
    pub header: ElfHeader,

    // And program segments.
    pub program_headers: Vec<ElfProgramHeader<'a>>,
    // Sections (used for linking) are ignored in this parse method.
}

impl<'a> Elf<'a> {
    pub fn parse(full_bytes: &'a [u8]) -> IResult<&'a [u8], Elf<'a>> {
        let (bytes, header) = ElfHeader::parse(full_bytes)?;

        let (mut program_header_bytes, _) =
            take(to_usize(header.program_headers_offset, full_bytes)?).parse(full_bytes)?;

        let mut program_headers = Vec::with_capacity(header.program_header_count as usize);

        for _ in 0..header.program_header_count {
            let (_, program_header) = ElfProgramHeader::parse(
                program_header_bytes,
                header.class,
                header.endianness.to_nom(),
                full_bytes,
            )?;

            program_headers.push(program_header);

            let (next, _) = take(header.program_header_entry_size).parse(program_header_bytes)?;

            program_header_bytes = next;
        }

        Ok((
            bytes,
            Elf {
                header,
                program_headers,
            },
        ))
    }

    pub fn parse_bytes(bytes: &'a [u8]) -> Result<Elf<'a>, nom::Err<Error<&'a [u8]>>> {
        Ok(Self::parse(bytes)?.1)
    }
}
