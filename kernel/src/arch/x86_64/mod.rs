//! x86_64 support.
//!
//! So far only enough to boot: `boot.s` takes the CPU from the multiboot2
//! entry into long mode in the higher half, and [`kmain`] prints a greeting
//! and the amount of available memory, then halts. The rest of the kernel is
//! still i686 only; descriptor tables, interrupts and the kernel's own page
//! tables come next.

mod boot;
mod kmain;
