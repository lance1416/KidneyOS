use super::super::ThreadControlBlock;
use crate::threading::process::Tid;
use alloc::boxed::Box;

pub trait Scheduler {
    fn new() -> Self
    where
        Self: Sized,
        Self: Sync;

    fn push(&mut self, thread: Box<ThreadControlBlock>);
    fn pop(&mut self) -> Option<Box<ThreadControlBlock>>;
    // Only used by system calls (which x86_64 doesn't have yet).
    #[cfg_attr(target_arch = "x86_64", allow(dead_code))]
    fn remove(&mut self, tid: Tid) -> Option<Box<ThreadControlBlock>>;
    fn get_mut(&mut self, tid: Tid) -> Option<&mut ThreadControlBlock>;
}
