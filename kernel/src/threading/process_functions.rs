use crate::system::{running_process, running_thread_tid};
use core::fmt::Arguments;
use kidneyos_shared::{eprintln, println};

use super::{
    thread_functions::{self, stop_thread},
    thread_sleep::thread_wakeup,
};

pub fn exit_process(exit_code: i32) -> ! {
    let pcb = running_process();
    let mut pcb = pcb.lock();
    pcb.exit_code = Some(exit_code);
    println!("Process {} exited with code {exit_code}", pcb.pid);

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

/// Ends the running process because of something it did wrong (like a page
/// fault outside its memory), with exit code -1.
pub fn kill_process(reason: Arguments) -> ! {
    eprintln!("Process {} killed: {reason}", running_process().lock().pid);
    exit_process(-1);
}
