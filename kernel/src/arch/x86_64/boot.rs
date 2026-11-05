//! The multiboot2 header and the entry point `_start`, which enters long mode
//! and calls [`kmain`](super::kmain::kmain). See the comments in `boot.s`.

core::arch::global_asm!(include_str!("boot.s"), options(att_syntax));
