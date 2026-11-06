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

#[cfg_attr(not(test), global_allocator)]
static mut KERNEL_ALLOCATOR: KernelAllocator = KernelAllocator::new();

/// Returns the global kernel allocator.
///
/// # Safety
///
/// The caller must not keep the returned reference across anything that uses
/// the allocator, like the global allocator interface. (Its methods hold
/// interrupts off, so one call at a time is fine.)
pub unsafe fn kernel_allocator() -> &'static mut KernelAllocator {
    let ptr = &raw mut KERNEL_ALLOCATOR;
    // SAFETY: Exclusive access is guaranteed by the caller.
    unsafe { &mut *ptr }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    arch::disable_interrupts();
    kidneyos_shared::eprintln!("KidneyOS panic: {info}");
    arch::halt_forever()
}

/// The user programs to start when booting, each as its own process (one
/// after the other, so they get consecutive pids). Chosen at build time with
/// `make KIDNEYOS_INIT=name,...` (default: pipes); see build.rs.
pub const INIT_PROGRAMS: &[&[u8]] = include!(concat!(env!("OUT_DIR"), "/init_programs.rs"));

/// The architecture-independent part of booting, after the architecture's
/// entry point (`kmain`) has set up memory (including the kernel allocator), the
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
