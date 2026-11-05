pub mod handlers;
pub mod mutex_irq;
pub mod pic;
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
