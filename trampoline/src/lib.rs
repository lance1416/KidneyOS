#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(not(test), no_main)]

mod multiboot2_header;

use core::{arch::asm, ptr::NonNull};
use kidneyos_shared::{
    global_descriptor_table,
    mem::{
        KERNEL_BASE, PAGE_FRAME_SIZE, PHYS_MAP_OFFSET,
        phys::{
            kernel_end, main_stack_top, trampoline_data_start, trampoline_end, trampoline_heap_top,
            trampoline_start,
        },
        pool_allocator::PoolAllocator,
        regions::{PhysRegion, largest_region},
    },
    multiboot2::{EXPECTED_MAGIC, Info, InfoTag, MEMORY_AVAILABLE},
    paging::{self, PageManager, kernel_mapping_ranges},
    println,
    sizes::MB,
    video_memory::{VIDEO_MEMORY_COLS, video_memory_writer},
};

#[cfg(target_os = "none")]
#[panic_handler]
fn panic(args: &core::panic::PanicInfo) -> ! {
    kidneyos_shared::eprintln!("{}", args);
    loop {}
}

#[cfg_attr(not(test), unsafe(no_mangle))]
#[unsafe(naked)]
unsafe extern "C" fn _start() {
    core::arch::naked_asm!(
        "
        lea esp, kernel_end
        sub esp, {stack_size}
        push ebx
        push eax
        call {}
        ",
        sym trampoline,
        stack_size = const KERNEL_BASE - kidneyos_shared::mem::MAIN_STACK_SIZE,
    )
}

#[allow(dead_code)]
unsafe extern "C" fn trampoline(magic: u32, multiboot2_info: *mut Info) {
    unsafe {
        assert!(
            magic == EXPECTED_MAGIC,
            "invalid magic, expected {EXPECTED_MAGIC:#X}, got {magic:#X}"
        );

        let memory_map = (*multiboot2_info)
            .iter()
            .find_map(|tag| match tag {
                InfoTag::MemoryMap(t) => Some(t),
                _ => None,
            })
            .expect("Didn't find memory map!");

        // Only physical memory in upper memory (from 1MiB) that the kernel can
        // reach through the direct map is usable.
        let direct_map_end = (usize::MAX - PHYS_MAP_OFFSET) as u64 + 1;
        let memory = largest_region(
            memory_map
                .entries()
                .filter(|entry| entry.r#type == MEMORY_AVAILABLE)
                .map(|entry| PhysRegion {
                    start: entry.base_addr,
                    end: entry.base_addr.saturating_add(entry.length),
                }),
            MB as u64,
            direct_map_end,
        )
        .expect("No usable memory!");
        let memory_start = usize::try_from(memory.start).expect("clamped to the direct map");
        let memory_end = usize::try_from(memory.end).expect("clamped to the direct map");
        println!("Usable memory: {memory_start:#X}..{memory_end:#X}");

        println!("Setting up GDTR");
        global_descriptor_table::load();
        println!("GDTR set up!");

        println!("Enabling paging");

        let pool_region = NonNull::slice_from_raw_parts(
            NonNull::new(main_stack_top() as *mut u8).expect("main_stack_top shouldn't be null"),
            trampoline_heap_top() - main_stack_top(),
        );
        let alloc = PoolAllocator::<PAGE_FRAME_SIZE>::new(pool_region);
        let mut page_manager =
            PageManager::from_mapping_ranges_in(kernel_mapping_ranges(), alloc, 0);

        // Trampoline mappings.
        page_manager.id_map_range(
            trampoline_start(),
            trampoline_data_start() - trampoline_start(),
            false,
            false,
        );
        page_manager.id_map_range(
            trampoline_data_start(),
            trampoline_end() - trampoline_data_start(),
            true,
            false,
        );
        page_manager.id_map_range(kernel_end(), main_stack_top() - kernel_end(), true, false);
        page_manager.id_map_range(
            main_stack_top(),
            trampoline_heap_top() - main_stack_top(),
            true,
            false,
        );

        page_manager.load();
        paging::enable();

        println!("Paging enabled!");

        println!("Starting kernel...");

        unsafe extern "C" {
            fn main(memory_start: usize, memory_end: usize, video_memory_skip_lines: usize) -> !;
        }

        asm!(
            "
        add esp, {offset} // make stack a kernel virtual address
        push {}
        push {}
        push {}
        call {}
        ",
            in(reg) video_memory_writer().cursor.div_ceil(VIDEO_MEMORY_COLS),
            in(reg) memory_end,
            in(reg) memory_start,
            sym main,
            offset = const KERNEL_BASE,
            options(noreturn)
        );
    }
}
