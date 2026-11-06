mod fifo_scheduler;
mod scheduler;

pub use fifo_scheduler::FIFOScheduler;
pub use scheduler::Scheduler;

use alloc::boxed::Box;

use super::{
    context_switch::switch_threads, thread_control_block::ThreadStatus, thread_sleep::take_wakeup,
};
use crate::arch;
use crate::interrupts::{IntrLevel, intr_get_level, mutex_irq::hold_interrupts};
use crate::system::unwrap_system;

pub fn create_scheduler() -> Box<dyn Scheduler + Send> {
    assert_eq!(intr_get_level(), IntrLevel::IntrOff);

    // SAFETY: Interrupts should be off.
    Box::new(FIFOScheduler::new())
}

/// Voluntarily relinquishes control of the CPU to another processor in the scheduler.
///
/// If no other thread can run, a thread that can continue just does. One that
/// can't (it is blocking or dying) waits for an interrupt to make one runnable,
/// with interrupts enabled. Such an interrupt preempts it like any other thread
/// (it is still running, after all), so it may get here again after a while.
fn scheduler_yield(status_for_current_thread: ThreadStatus) {
    // Restores this thread's interrupt state when it runs again.
    let _guard = hold_interrupts(IntrLevel::IntrOff);

    loop {
        // With interrupts disabled, nothing can wake this thread between this
        // check and it being marked as blocked by `switch_threads`.
        if status_for_current_thread == ThreadStatus::Blocked && take_wakeup() {
            return;
        }

        let switch_to = unwrap_system().threads.scheduler.lock().pop_runnable();
        if let Some(switch_to) = switch_to {
            // SAFETY: Threads and Scheduler must be initialized and active.
            // Interrupts must be disabled.
            unsafe { switch_threads(status_for_current_thread, switch_to) };
            return;
        }

        if status_for_current_thread == ThreadStatus::Ready {
            return;
        }
        arch::wait_for_interrupt();
    }
}

// Voluntarily relinquishes control of the CPU and marks current thread as ready.
pub fn scheduler_yield_and_continue() {
    scheduler_yield(ThreadStatus::Ready);
}

/// Voluntarily relinquishes control of the CPU and marks the current thread to die.
pub fn scheduler_yield_and_die() -> ! {
    scheduler_yield(ThreadStatus::Dying);

    panic!("A thread was rescheduled after dying.");
}

/// Voluntarily relinquishes control of the CPU and marks the current thread as blocked.
#[allow(unused)]
pub fn scheduler_yield_and_block() {
    scheduler_yield(ThreadStatus::Blocked);
}
