//! The Rust entry point of the x86_64 kernel.

use core::{arch::asm, panic::PanicInfo};
use kidneyos_shared::{
    eprintln,
    mem::KERNEL_BASE,
    multiboot2::{EXPECTED_MAGIC, Info, InfoTag, MEMORY_AVAILABLE},
    println,
    serial::serial_writer,
    sizes::{GB, MB},
    video_memory::video_memory_writer,
};

/// Physical memory below this is mapped at [`KERNEL_BASE`] by the boot stub.
const BOOT_MAPPED_PHYS_END: usize = GB;

/// Returns the address that physical address `phys` is mapped at by the boot
/// stub's page tables, if the `len` bytes from it are mapped.
fn boot_phys_to_virt(phys: usize, len: usize) -> Option<usize> {
    let end = phys.checked_add(len)?;
    (end <= BOOT_MAPPED_PHYS_END).then_some(phys + KERNEL_BASE)
}

/// Called by `_start` in boot.s, in long mode, with the multiboot2 magic and
/// the physical address of the multiboot2 boot information.
#[unsafe(no_mangle)]
extern "C" fn kmain(magic: u32, multiboot2_info_phys: usize) -> ! {
    // SAFETY: Single core, interrupts disabled, nothing else is using the
    // writers yet.
    unsafe {
        serial_writer().init();
        video_memory_writer().clear_screen();
    }

    println!("KidneyOS x86_64: hello");

    assert_eq!(
        magic, EXPECTED_MAGIC,
        "invalid multiboot2 magic, expected {EXPECTED_MAGIC:#X}, got {magic:#X}"
    );

    // SAFETY: The bootloader passed the physical address of a valid boot
    // information structure, which starts with `Info` (its total size) and
    // stays untouched since nothing allocates memory yet. It is accessed
    // through the boot stub's mapping, which covers it as checked.
    let info = unsafe {
        let header = boot_phys_to_virt(multiboot2_info_phys, size_of::<Info>())
            .expect("multiboot2 information isn't mapped");
        let total_size = (*(header as *const Info)).total_size as usize;
        let info = boot_phys_to_virt(multiboot2_info_phys, total_size)
            .expect("multiboot2 information isn't mapped");
        &*(info as *const Info)
    };

    let memory_map = info
        .iter()
        .find_map(|tag| match tag {
            InfoTag::MemoryMap(tag) => Some(tag),
            _ => None,
        })
        .expect("no memory map in the multiboot2 information");

    let mut available: u64 = 0;
    for entry in memory_map
        .entries()
        .filter(|entry| entry.r#type == MEMORY_AVAILABLE)
    {
        let end = entry.base_addr.saturating_add(entry.length);
        println!("  available: {:#014X}..{end:#014X}", entry.base_addr);
        available = available.saturating_add(entry.length);
    }
    println!(
        "Available memory: {} MiB ({available:#X} bytes)",
        available / MB as u64
    );

    // TODO(x86_64): Set up the GDT, TSS, IDT, page tables and the rest of the
    // kernel here.
    halt_forever()
}

/// Halts the CPU for good.
fn halt_forever() -> ! {
    loop {
        // SAFETY: Only stops the CPU; with interrupts disabled, only an NMI
        // wakes it up again.
        unsafe { asm!("cli", "hlt", options(nomem, nostack)) }
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    eprintln!("KidneyOS x86_64 panic: {info}");
    halt_forever()
}
