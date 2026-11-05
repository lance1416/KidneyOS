use super::process::Tid;
use super::thread_control_block::{ThreadControlBlock, ThreadStatus};
use crate::system::unwrap_system;
use crate::{
    arch,
    interrupts::{intr_disable, intr_enable},
    threading::scheduling::scheduler_yield_and_die,
};
use alloc::boxed::Box;

/// TODO: Thread arguments: Usually a void ptr, but Rust won't like that...
/// No arguments allowed for now.
///
/// A function that may be used for thread creation.
/// The return value will be the exit code of this thread.
pub type ThreadFunction = unsafe extern "C" fn() -> i32;

/// A function to safely close the current thread.
/// This is safe to call at any point in a threads runtime.
#[allow(unused)]
pub fn exit_thread(exit_code: i32) -> ! {
    // We will never return here so do not need to re-enable interrupts from here.
    intr_disable();

    // Set current thread's exit code.
    let threads = &unwrap_system().threads;
    let mut guard = threads.running_thread.lock();
    let mut current_thread = guard.as_mut().expect("Why is nothing running!?");
    current_thread.set_exit_code(exit_code);
    drop(guard);
    // Yield.
    scheduler_yield_and_die();
}

pub unsafe fn clean_up_thread(mut dying_thread: Box<ThreadControlBlock>) {
    unsafe {
        let threads = &unwrap_system().threads;

        dying_thread.reap();

        // Page manager must be loaded to be dropped.
        dying_thread.page_manager.load();
        drop(dying_thread);
        threads
            .running_thread
            .lock()
            .as_ref()
            .expect("a thread is running after switching")
            .page_manager
            .load();
    }
}

// Focibly stops the thread specified by Tid
pub fn stop_thread(tid: Tid) {
    let mut scheduler = unwrap_system().threads.scheduler.lock();
    let tcb = scheduler.remove(tid).expect("Why is nothing running !?");
    unsafe { clean_up_thread(tcb) };
}

/// A wrapper function to execute a thread's true function.
///
/// Newly created threads start here (via the architecture's thread entry
/// trampoline) the first time they are switched to.
pub(crate) unsafe extern "C" fn run_thread(
    switched_from: *mut ThreadControlBlock,
    switched_to: *mut ThreadControlBlock,
) -> ! {
    unsafe {
        let threads = &unwrap_system().threads;
        let mut switched_to = Box::from_raw(switched_to);

        // We assume that switched_from had its status changed already.
        // We must only mark this thread as running.
        switched_to.status = ThreadStatus::Running;

        let ThreadControlBlock {
            ip, sp, is_kernel, ..
        } = *switched_to;

        // Reschedule our threads.
        *threads.running_thread.lock() = Some(switched_to);

        let switched_from = Box::from_raw(switched_from);

        if switched_from.status == ThreadStatus::Dying {
            clean_up_thread(switched_from);
        } else {
            threads.scheduler.lock().push(switched_from);
        }

        // Our scheduler will operate without interrupts.
        // Every new thread should start with them enabled.
        intr_enable();

        // Kernel threads have no associated PCB, denoted by its PID being 0
        if is_kernel {
            let entry_function: ThreadFunction = core::mem::transmute(ip.as_ptr());
            let exit_code = entry_function();

            // Safely exit the thread.
            exit_thread(exit_code);
        } else {
            arch::enter_user_mode(ip, sp);
        }
    }
}
