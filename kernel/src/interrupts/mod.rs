// The x86_64 kernel only uses the PIC so far.
#![cfg_attr(target_arch = "x86_64", allow(dead_code))]

#[cfg(target_arch = "x86")]
pub mod handlers;
#[cfg(target_arch = "x86")]
pub mod mutex_irq;
pub mod pic;
#[cfg(target_arch = "x86")]
pub mod timer;

use crate::arch;

#[allow(unused)]
#[derive(Debug, PartialEq)]
pub enum IntrLevel {
    IntrOn,
    IntrOff,
}

#[allow(unused)]
pub fn intr_get_level() -> IntrLevel {
    if arch::interrupts_enabled() {
        IntrLevel::IntrOn
    } else {
        IntrLevel::IntrOff
    }
}

#[inline(always)]
pub fn intr_enable() {
    arch::enable_interrupts();
}

#[inline(always)]
pub fn intr_disable() {
    arch::disable_interrupts();
}
