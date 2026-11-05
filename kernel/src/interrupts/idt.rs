// https://wiki.osdev.org/Interrupt_Descriptor_Table
// https://wiki.osdev.org/Interrupts_tutorial
// https://wiki.osdev.org/Exceptions

use core::{arch::asm, mem::size_of};
use kidneyos_shared::{bit_array::BitArray, bitfield};

use crate::interrupts::intr_handler::{
    general_protection_fault_handler, ide_prim_interrupt_handler, ide_secd_interrupt_handler,
    keyboard_handler, page_fault_handler, syscall_handler, timer_interrupt_handler,
    unhandled_handler,
};

bitfield!(
    GateDescriptor, u64
    {
        (u16, offset_low, 0, 15),
        (u16, offset_high, 48, 63),
        (u16, segment_selector, 16, 31),
        (u8, gate_type, 40, 43),
        (u8, descriptor_privilege_level, 45, 46),
    }
    { (present, 47) }
);

impl GateDescriptor {
    #[allow(dead_code)]
    pub fn offset(&self) -> u32 {
        (u32::from(self.offset_high()) << 16) | u32::from(self.offset_low())
    }

    // Intentional: split the handler offset into its two 16-bit halves.
    #[allow(dead_code, clippy::cast_possible_truncation)]
    pub fn with_offset(self, value: u32) -> Self {
        self.with_offset_low(value as u16)
            .with_offset_high((value >> 16) as u16)
    }
}

#[repr(C, packed)]
struct IDTDescriptor {
    #[allow(unused)]
    size: u16,
    offset: u32,
}

const IDT_LEN: usize = 256;
static mut IDT: [GateDescriptor; IDT_LEN] = [GateDescriptor::default(); IDT_LEN];

// TODO: Set up stack on entry to handlers from kernel, the current behaviour is
// horribly dangerous... The current behaviour is currently safe fine for cases
// where we're entering a handler from usermode though, because when doing that
// we get the new stack from the TSS.

// The IDT size is a small compile-time constant; `try_from` isn't const.
#[allow(clippy::cast_possible_truncation)]
static mut IDT_DESCRIPTOR: IDTDescriptor = IDTDescriptor {
    size: size_of::<[GateDescriptor; IDT_LEN]>() as u16 - 1,
    offset: 0, // Will fetch pointer and set at runtime below.
};

/// # Safety
///
/// Can only be executed within code that expects the interrupt handlers to be
/// defined as they are described in intr_handler.rs
// 32-bit x86 specific: gate descriptors hold 32-bit handler addresses. Revisit
// for the x86_64 port.
#[allow(clippy::cast_possible_truncation)]
pub unsafe fn load() {
    unsafe {
        IDT_DESCRIPTOR.offset = (&raw const IDT).cast::<GateDescriptor>() as u32;

        let idt_ptr = &raw mut IDT;
        let idt = &mut *idt_ptr;
        for gate_descriptor in idt.iter_mut() {
            *gate_descriptor = GateDescriptor::default()
                .with_offset(unhandled_handler as *const () as usize as u32)
                .with_segment_selector(0x8)
                .with_gate_type(0xEu8)
                .with_descriptor_privilege_level(3u8)
                .with_present(true);
        }
        idt[0xd] =
            idt[0xd].with_offset(general_protection_fault_handler as *const () as usize as u32);
        idt[0xe] = idt[0xe].with_offset(page_fault_handler as *const () as usize as u32);
        idt[0x20] = idt[0x20].with_offset(timer_interrupt_handler as *const () as usize as u32); // PIC1_OFFSET (IRQ0)
        idt[0x21] = idt[0x21].with_offset(keyboard_handler as *const () as usize as u32); // Keyboard (IRQ1)
        idt[0x2E] = idt[0x2E].with_offset(ide_prim_interrupt_handler as *const () as usize as u32); // IDE Primary (IRQ14)
        idt[0x2F] = idt[0x2F].with_offset(ide_secd_interrupt_handler as *const () as usize as u32); // IDE Secondary (IRQ15)
        idt[0x80] = idt[0x80].with_offset(syscall_handler as *const () as usize as u32);

        asm!("lidt [{}]", sym IDT_DESCRIPTOR);
    }
}
