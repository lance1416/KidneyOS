use super::super::ThreadControlBlock;
use super::super::thread_control_block::ThreadStatus;
use super::scheduler::Scheduler;
use crate::threading::process::Tid;
use alloc::{boxed::Box, collections::VecDeque};

pub struct FIFOScheduler {
    ready_queue: VecDeque<Box<ThreadControlBlock>>,
}

// TODO: Will be removed, requires a change to stack type.
// SAFETY: Schedulers should be run with interrupts disabled.
unsafe impl Sync for FIFOScheduler {}

impl Scheduler for FIFOScheduler {
    fn new() -> FIFOScheduler {
        FIFOScheduler {
            ready_queue: VecDeque::new(),
        }
    }

    fn push(&mut self, thread: Box<ThreadControlBlock>) {
        self.ready_queue.push_back(thread);
    }

    fn pop_runnable(&mut self) -> Option<Box<ThreadControlBlock>> {
        let pos = self
            .ready_queue
            .iter()
            .position(|tcb| tcb.status != ThreadStatus::Blocked);
        self.ready_queue.remove(pos?)
    }

    fn remove(&mut self, _tid: Tid) -> Option<Box<ThreadControlBlock>> {
        let pos = self.ready_queue.iter().position(|tcb| tcb.tid == _tid);
        self.ready_queue.remove(pos?)
    }

    fn get_mut(&mut self, _tid: Tid) -> Option<&mut ThreadControlBlock> {
        let pos = self.ready_queue.iter().position(|tcb| tcb.tid == _tid);
        pos.and_then(|index| self.ready_queue.get_mut(index).map(|tcb| &mut **tcb))
    }
}
