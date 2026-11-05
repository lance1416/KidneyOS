//! The Rust entry point of the x86_64 kernel.

use super::{
    disable_interrupts, enable_interrupts, gdt, halt_forever,
    idt::{self, TrapFrame},
    paging::{self, phys_to_virt},
};
use crate::interrupts::pic;
use core::{arch::asm, panic::PanicInfo};
use kidneyos_shared::{
    eprintln,
    mem::regions::PhysRegion,
    multiboot2::{EXPECTED_MAGIC, Info, InfoTag},
    println,
    serial::serial_writer,
    video_memory::video_memory_writer,
};

/// Timer interrupts per second.
const TIMER_HZ: u32 = 100;
/// How many ticks apart kmain reports the tick count.
const TICKS_PER_REPORT: u64 = 100;

/// Set at build time with `make ARCH=x86_64 KIDNEYOS_TEST=...` to run one of
/// the tests below while booting.
const TEST: Option<&str> = option_env!("KIDNEYOS_TEST");

/// The address `KIDNEYOS_TEST=pagefault` writes to: unmapped, so the write
/// faults, and the page fault handler maps a page there. It must be canonical
/// (bit 47 clear in the lower half), or the write raises #GP instead.
const PAGE_FAULT_TEST_ADDRESS: u64 = 0x0000_0BAD_BEEF_0128;

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

    // SAFETY: Called once, with interrupts disabled.
    unsafe {
        gdt::init();
        idt::init();
    }
    println!("GDT, TSS and IDT loaded");

    // SAFETY: The bootloader passed the physical address of a valid boot
    // information structure, which nothing has overwritten (nothing has
    // allocated memory yet). The boot stub maps it in the direct map, as the
    // assertion checks.
    let info = unsafe {
        let phys = multiboot2_info_phys as u64;
        assert!(phys < 1 << 30, "the multiboot2 information isn't mapped");
        &*(phys_to_virt(phys) as *const Info)
    };
    let memory_map = info
        .iter()
        .find_map(|tag| match tag {
            InfoTag::MemoryMap(tag) => Some(tag),
            _ => None,
        })
        .expect("no memory map in the multiboot2 information");

    // The frame allocator must not hand out the kernel image or the boot
    // information (which the memory map is part of).
    let reserved = [image_region(), {
        let start = multiboot2_info_phys as u64;
        PhysRegion {
            start,
            end: start + u64::from(info.total_size),
        }
    }];
    let memory = paging::init_frames(memory_map, &reserved);

    // SAFETY: Called once; the stack, the descriptor tables and everything
    // else in use are in the kernel image.
    unsafe { paging::init_kernel_page_tables(&memory) };

    // SAFETY: Interrupts are still disabled, and the IDT handles every IRQ.
    unsafe {
        pic::pic_remap(pic::PIC1_OFFSET, pic::PIC2_OFFSET);
        #[allow(clippy::cast_possible_truncation)]
        pic::init_pit_with_reload((pic::PIT_INPUT_HZ / TIMER_HZ) as u16);
    }
    enable_interrupts();
    println!("Interrupts enabled, timer at {TIMER_HZ}Hz");

    match TEST {
        None | Some("") => {}
        Some("pagefault") => page_fault_test(),
        Some(test) => panic!("unknown KIDNEYOS_TEST {test:?}"),
    }

    let mut reported = 0;
    loop {
        // SAFETY: Waits for the next interrupt; interrupts are enabled.
        unsafe { asm!("hlt", options(nomem, nostack, preserves_flags)) };
        let ticks = idt::ticks();
        if ticks / TICKS_PER_REPORT > reported {
            reported = ticks / TICKS_PER_REPORT;
            println!("tick {}", reported * TICKS_PER_REPORT);
        }
    }
}

/// The physical memory the kernel image (including the boot stub) was loaded
/// into.
fn image_region() -> PhysRegion {
    unsafe extern "C" {
        static image_start_phys: u8;
        static kernel_end_phys: u8;
    }
    // Only the addresses of the linker symbols are used.
    PhysRegion {
        start: &raw const image_start_phys as u64,
        end: &raw const kernel_end_phys as u64,
    }
}

/// `KIDNEYOS_TEST=pagefault`: writes to an unmapped address. The page fault
/// handler prints the fault (with cr2), maps a page there and returns, so the
/// write is retried and succeeds.
fn page_fault_test() {
    fn map_test_page(cr2: u64, _frame: &TrapFrame) -> bool {
        // SAFETY: Nothing uses this address yet.
        cr2 == PAGE_FAULT_TEST_ADDRESS && unsafe { paging::map_fresh_page(cr2) }
    }

    println!("KIDNEYOS_TEST=pagefault: writing to {PAGE_FAULT_TEST_ADDRESS:#018X}");
    idt::set_page_fault_fixup(map_test_page);

    let pointer = PAGE_FAULT_TEST_ADDRESS as *mut u64;
    // SAFETY: Faults, and the handler maps the page before the write is
    // retried. Volatile, so that the write and read really happen.
    let value = unsafe {
        pointer.write_volatile(0x1234_5678);
        pointer.read_volatile()
    };
    assert_eq!(value, 0x1234_5678);
    println!("KIDNEYOS_TEST=pagefault: passed, read back {value:#X} after the page fault");
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    disable_interrupts();
    eprintln!("KidneyOS x86_64 panic: {info}");
    halt_forever()
}
