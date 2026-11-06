use super::{scheduling::scheduler_yield_and_block, thread_control_block::ThreadStatus};
use crate::interrupts::{IntrLevel, mutex_irq::hold_interrupts};
use crate::system::unwrap_system;
use crate::threading::process::Tid;

/// Blocks the running thread until [`thread_wakeup`] is called for it, or
/// doesn't if that already happened since it last slept. So a thread can tell
/// others that it is waiting (say, by joining a wait queue) and then sleep,
/// without missing a wakeup that comes in between. It may return early, so
/// always check what you are waiting for again afterwards.
pub fn thread_sleep() {
    scheduler_yield_and_block();
}

/// Makes the thread `tid` runnable if it is blocked, and otherwise makes its
/// next [`thread_sleep`] return at once. Does nothing if it doesn't exist.
pub fn thread_wakeup(tid: Tid) {
    let _guard = hold_interrupts(IntrLevel::IntrOff);
    let threads = &unwrap_system().threads;

    // An interrupt handler may be waking the thread it interrupted.
    if let Some(tcb) = threads.running_thread.lock().as_mut()
        && tcb.tid == tid
    {
        tcb.wake_pending = true;
        return;
    }

    if let Some(tcb) = threads.scheduler.lock().get_mut(tid) {
        if tcb.status == ThreadStatus::Blocked {
            tcb.status = ThreadStatus::Ready;
        } else {
            tcb.wake_pending = true;
        }
    }
}

/// Returns whether a wakeup is pending for the running thread, and clears it.
pub fn take_wakeup() -> bool {
    let mut running_thread = unwrap_system().threads.running_thread.lock();
    let tcb = running_thread.as_mut().expect("no running thread");
    core::mem::take(&mut tcb.wake_pending)
}
