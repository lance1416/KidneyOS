use crate::system::{running_process, running_thread_tid};

use super::{
    thread_functions::{self, stop_thread},
    thread_sleep::thread_wakeup,
};

// Only used by system calls (which x86_64 doesn't have yet).
#[cfg_attr(target_arch = "x86_64", allow(dead_code))]
pub fn exit_process(exit_code: i32) -> ! {
    let pcb = running_process();
    let mut pcb = pcb.lock();
    pcb.exit_code = Some(exit_code);

    if let Some(wait_tid) = pcb.waiting_thread {
        thread_wakeup(wait_tid);
    }

    let running_tid = running_thread_tid();

    // Kill all threads which are part of this process
    pcb.child_tids.iter().for_each(|tid| {
        if *tid != running_tid {
            stop_thread(*tid)
        }
    });
    drop(pcb);

    thread_functions::exit_thread(-1);
}
