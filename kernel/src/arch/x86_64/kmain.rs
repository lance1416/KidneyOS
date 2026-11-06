//! The Rust entry point of the x86_64 kernel.

use super::{
    gdt,
    idt::{self, TrapFrame},
    paging::{self, phys_to_virt},
};
use crate::{
    INIT_PROGRAMS,
    interrupts::{intr_disable, intr_enable, pic},
    kernel_allocator,
    paging::{PageManager, PageManagerDefault},
    start_kernel,
};
use core::{
    hint::spin_loop,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};
use kidneyos_shared::{
    mem::regions::PhysRegion,
    multiboot2::{EXPECTED_MAGIC, Info, InfoTag},
    println,
    serial::serial_writer,
    video_memory::video_memory_writer,
};

/// Set at build time with `make KIDNEYOS_TEST=...` to run one of
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

    // SAFETY: Only changes how x87/SSE instructions behave, which the kernel
    // doesn't use.
    unsafe { enable_sse() };

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

    match TEST {
        None | Some("") => {}
        Some("pagefault") => page_fault_test(),
        // These only choose the init programs, see the Makefile.
        Some("usergp" | "protfault" | "badsyscall") => {}
        Some(test) => panic!("unknown KIDNEYOS_TEST {test:?}"),
    }

    // The kernel heap gets one region; the frame allocator above keeps the
    // rest.
    let heap = paging::take_largest_region();
    let to_usize = |addr| usize::try_from(addr).expect("physical addresses fit in usize");
    // SAFETY: Single core, interrupts disabled; the region is free memory in
    // the direct map that nothing else will hand out.
    unsafe { kernel_allocator().init(to_usize(heap.start)..to_usize(heap.end)) };

    // The address space of the thread kmain becomes: the kernel's half only.
    let page_manager = PageManager::default();
    // SAFETY: Same kernel half as the page tables loaded now, and nothing in
    // use is in the user half.
    unsafe { page_manager.load() };

    // SAFETY: Interrupts are still disabled (until the threads start), and
    // the IDT handles every IRQ.
    unsafe {
        println!("Setting up PIT");
        pic::pic_remap(pic::PIC1_OFFSET, pic::PIC2_OFFSET);
        pic::init_pit();
        println!("PIT set up!");
    }

    start_kernel(
        page_manager,
        INIT_PROGRAMS,
        &[preemption_test, preemption_test],
    )
}

/// Lets user programs use SSE (every x86_64 CPU has it): CR0.EM off (no x87
/// emulation), CR0.MP on (wait/fwait honour CR0.TS), CR0.NE on (x87 errors
/// raise #MF rather than the legacy external interrupt), and CR4.OSFXSR
/// (fxsave/fxrstor and SSE instructions) and CR4.OSXMMEXCPT (unmasked SIMD
/// floating-point exceptions raise #XM rather than #UD) on. Threads' register
/// state is switched with `fxsave`/`fxrstor` (see `context::FpuState`).
unsafe fn enable_sse() {
    const CR0_MP: u64 = 1 << 1;
    const CR0_EM: u64 = 1 << 2;
    const CR0_TS: u64 = 1 << 3;
    const CR0_NE: u64 = 1 << 5;
    const CR4_OSFXSR: u64 = 1 << 9;
    const CR4_OSXMMEXCPT: u64 = 1 << 10;
    // SAFETY: Per the caller.
    unsafe {
        core::arch::asm!(
            "mov {tmp}, cr0",
            "and {tmp}, {cr0_clear}",
            "or {tmp}, {cr0_set}",
            "mov cr0, {tmp}",
            "mov {tmp}, cr4",
            "or {tmp}, {cr4_set}",
            "mov cr4, {tmp}",
            tmp = out(reg) _,
            cr0_clear = in(reg) !(CR0_EM | CR0_TS),
            cr0_set = const CR0_MP | CR0_NE,
            cr4_set = const CR4_OSFXSR | CR4_OSXMMEXCPT,
            options(nostack),
        );
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

static PREEMPTION_TEST_THREADS: AtomicUsize = AtomicUsize::new(0);
static PREEMPTION_TEST_STARTED: [AtomicBool; 2] = [AtomicBool::new(false), AtomicBool::new(false)];

/// Run by two kernel threads at startup, to show that the timer preempts
/// threads: each spins without yielding until the other one has started. The
/// first one to run only gets past that if it is preempted.
// Kernel threads don't make system calls, so this checks the timer's
// preemption on its own.
extern "C" fn preemption_test() -> i32 {
    let me = PREEMPTION_TEST_THREADS.fetch_add(1, Ordering::Relaxed) % 2;
    let other = 1 - me;
    PREEMPTION_TEST_STARTED[me].store(true, Ordering::Relaxed);

    let start = idt::ticks();
    while !PREEMPTION_TEST_STARTED[other].load(Ordering::Relaxed) {
        spin_loop();
    }
    let waited = idt::ticks() - start;

    // Don't interleave with what other threads print.
    intr_disable();
    if waited > 0 {
        println!(
            "Preemption test: thread {me} was preempted while spinning, \
             and saw thread {other} start {waited} ticks later"
        );
    } else {
        println!("Preemption test: thread {me} started after thread {other}");
    }
    intr_enable();
    0
}
