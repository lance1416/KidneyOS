use crate::{
    arch,
    interrupts::{IntrLevel, intr_get_level},
    threading::thread_functions::clean_up_thread,
};

use super::thread_control_block::{ThreadControlBlock, ThreadStatus};
use crate::system::unwrap_system;
use alloc::boxed::Box;

/// Public facing method to perform a context switch between two threads.
/// # Safety
/// This function should only be called by methods within the Scheduler crate.
/// Interrupts must be disabled.
pub unsafe fn switch_threads(
    status_for_current_thread: ThreadStatus,
    switch_to: Box<ThreadControlBlock>,
) {
    unsafe {
        assert_eq!(intr_get_level(), IntrLevel::IntrOff);
        let threads = &unwrap_system().threads;

        let switch_from = Box::into_raw(
            threads
                .running_thread
                .lock()
                .take()
                .expect("Why is nothing running!?"),
        );
        let switch_to = Box::into_raw(switch_to);

        // Ensure we are switching to a valid thread.
        assert_eq!(
            (*switch_to).status,
            ThreadStatus::Ready,
            "Cannot switch to a non-ready thread."
        );

        // Ensure that the previous thread is running.
        assert_eq!(
            (*switch_from).status,
            ThreadStatus::Running,
            "The thread to switch out of must be in the running state."
        );

        // Update the status of the current thread.
        (*switch_from).status = status_for_current_thread;

        let page_manager = &(*switch_to).page_manager;
        page_manager.load();

        // Interrupts taken in user mode switch to the stack in the TSS, so it
        // must point at the top of the kernel stack of the thread we resume,
        // whether it runs for the first time or not. Kernel threads never
        // leave ring 0, so the TSS isn't used for them.
        if !(*switch_to).is_kernel {
            arch::set_kernel_stack((*switch_to).kernel_stack_top());
        }

        let previous = Box::from_raw(arch::context_switch(switch_from, switch_to));

        // We must mark this thread as running once again.
        (*switch_from).status = ThreadStatus::Running;

        // After threads have switched, we must update the scheduler and running thread.
        *threads.running_thread.lock() = Some(Box::from_raw(switch_from));

        if previous.status == ThreadStatus::Dying {
            clean_up_thread(previous);
        } else {
            threads.scheduler.lock().push(previous);
        }
    }
}
