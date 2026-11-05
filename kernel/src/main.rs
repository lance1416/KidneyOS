#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
#![feature(negative_impls)]
// Only used by the buddy allocator tests.
#![cfg_attr(test, feature(btreemap_alloc, slice_ptr_get))]

mod arch;
mod block;
mod drivers;
pub mod fs;
mod interrupts;
pub mod mem;
mod paging;
mod rush;
pub mod sync;
mod system;
mod threading;
mod user_program;
pub mod vfs;

extern crate alloc;

use crate::block::block_core::BlockManager;
use crate::drivers::ata::ata_core::ide_init;
use crate::drivers::input::input_core::InputBuffer;
use crate::fs::fs_manager::RootFileSystem;
use crate::sync::mutex::Mutex;
use crate::sync::rwlock::sleep::RwLock;
use crate::system::SystemState;
use crate::threading::process::create_process_state;
use crate::threading::thread_control_block::ThreadControlBlock;
use crate::threading::thread_functions::ThreadFunction;
use alloc::boxed::Box;
use kidneyos_shared::println;
use mem::KernelAllocator;
use paging::PageManager;
use threading::{create_thread_state, thread_system_start};
use vfs::tempfs::TempFS;
#[cfg(target_arch = "x86")]
use {
    interrupts::pic,
    kidneyos_shared::{global_descriptor_table, video_memory::video_memory_writer},
};

#[cfg_attr(not(test), global_allocator)]
static mut KERNEL_ALLOCATOR: KernelAllocator = KernelAllocator::new();

/// Returns the global kernel allocator.
///
/// # Safety
///
/// The caller must ensure that nothing else (including the global allocator
/// interface, e.g. from an interrupt handler) uses the allocator while the
/// returned reference is in use.
pub unsafe fn kernel_allocator() -> &'static mut KernelAllocator {
    let ptr = &raw mut KERNEL_ALLOCATOR;
    // SAFETY: Exclusive access is guaranteed by the caller.
    unsafe { &mut *ptr }
}

#[cfg(all(target_arch = "x86", not(test)))]
#[panic_handler]
fn panic(args: &core::panic::PanicInfo) -> ! {
    kidneyos_shared::eprintln!("{}", args);
    loop {}
}

/// The user programs to start when booting, each as its own process (one
/// after the other, so they get consecutive pids). Chosen at build time with
/// `make KIDNEYOS_INIT=name,...` (default: pipes); see build.rs.
pub const INIT_PROGRAMS: &[&[u8]] = include!(concat!(env!("OUT_DIR"), "/init_programs.rs"));

#[cfg(target_arch = "x86")]
#[cfg_attr(not(test), unsafe(no_mangle))]
extern "C" fn main(memory_start: usize, memory_end: usize, video_memory_skip_lines: usize) -> ! {
    unsafe {
        video_memory_writer().skip_lines(video_memory_skip_lines);
    }

    // SAFETY: Single core, interrupts disabled.
    let page_manager = unsafe {
        kernel_allocator().init(memory_start..memory_end);

        println!("Setting up IDTR");
        arch::load_idt();
        println!("IDTR set up!");

        println!("Enabling paging");
        let page_manager = paging::enable();
        println!("Paging enabled!");

        println!("Setting up GDTR");
        global_descriptor_table::load();
        println!("GDTR set up!");

        println!("Setting up PIT");
        pic::pic_remap(pic::PIC1_OFFSET, pic::PIC2_OFFSET);
        pic::init_pit();
        println!("PIT set up!");

        page_manager
    };

    start_kernel(page_manager, INIT_PROGRAMS, &[])
}

/// The architecture-independent part of booting, after the architecture's
/// entry point has set up memory (including the kernel allocator), the
/// descriptor tables and the PIT. Starts the threads, including a process for
/// each of the user programs in `init`, and continues as the kernel shell
/// (rush).
///
/// `page_manager` is the address space of the thread this becomes, and
/// `kernel_threads` are kernel threads to start besides the IDE driver's.
pub fn start_kernel(
    page_manager: PageManager,
    init: &[&[u8]],
    kernel_threads: &[ThreadFunction],
) -> ! {
    println!("Initializing Thread System...");
    let threads = create_thread_state();
    let mut process = create_process_state();
    println!("Finished Thread System initialization. Ready to start threading.");

    println!("Mounting root filesystem...");
    let mut root = RootFileSystem::new();
    // for now, we just use TempFS for the root filesystem
    root.mount_root(TempFS::new())
        .expect("Couldn't mount root FS");
    println!("Root filesystem mounted");

    let ide_tcb = ThreadControlBlock::new_with_setup(ide_init, true, 0, &mut root, &mut process);
    threads.scheduler.lock().push(Box::new(ide_tcb));
    for &function in kernel_threads {
        let tcb = ThreadControlBlock::new_with_setup(function, true, 0, &mut root, &mut process);
        threads.scheduler.lock().push(Box::new(tcb));
    }

    let block_manager = BlockManager::default();
    let input_buffer = Mutex::new(InputBuffer::new());

    crate::system::init_system(SystemState {
        threads,
        process,
        block_manager: RwLock::new(block_manager),
        root_filesystem: Mutex::new(root),
        input_buffer,
    });
    println!("initialized system");

    thread_system_start(page_manager, init);
}
