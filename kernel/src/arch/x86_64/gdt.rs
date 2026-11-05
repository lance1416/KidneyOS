//! The 64-bit GDT and TSS.
//!
//! https://wiki.osdev.org/GDT_Tutorial#Long_Mode_Setup
//! https://wiki.osdev.org/Task_State_Segment#Long_Mode
//!
//! In long mode segmentation is mostly gone: the code segments only select the
//! privilege level and 64-bit mode, and the TSS only holds stack pointers, for
//! interrupts from user mode (`rsp0`) and for the interrupt stack table (IST).

use core::{arch::asm, mem::size_of, ptr::NonNull};

pub const KERNEL_CODE_SELECTOR: u16 = 0x08;
pub const KERNEL_DATA_SELECTOR: u16 = 0x10;
// User data before user code, in the order `sysret` expects them in, should
// KidneyOS ever use it.
#[allow(dead_code)]
pub const USER_DATA_SELECTOR: u16 = 0x18 | 3;
#[allow(dead_code)]
pub const USER_CODE_SELECTOR: u16 = 0x20 | 3;
pub const TSS_SELECTOR: u16 = 0x28;

/// IST entry (1-based, as in IDT gates) used for double faults.
pub const DOUBLE_FAULT_IST: u8 = 1;
/// IST entry used for NMIs.
pub const NMI_IST: u8 = 2;

// Flat segments: limit 0xFFFFF with 4KiB granularity, base 0.
// Access byte 0x9A/0x92 (present, DPL 0, code/data), 0xFA/0xF2 the same with
// DPL 3; flags 0xA (granularity, long mode) for code, 0xC (granularity, 32-bit)
// for data.
const KERNEL_CODE: u64 = 0x00AF_9A00_0000_FFFF;
const KERNEL_DATA: u64 = 0x00CF_9200_0000_FFFF;
const USER_DATA: u64 = 0x00CF_F200_0000_FFFF;
const USER_CODE: u64 = 0x00AF_FA00_0000_FFFF;

#[repr(C, packed(4))]
struct TaskStateSegment {
    _reserved0: u32,
    /// Stack pointers loaded on interrupts from ring 3 (only rsp0 is used).
    rsp: [u64; 3],
    _reserved1: u64,
    /// The interrupt stack table: IDT gates with a nonzero IST field always
    /// switch to `ist[ist_field - 1]`.
    ist: [u64; 7],
    _reserved2: u64,
    _reserved3: u16,
    iomap_base: u16,
}

const _: () = assert!(size_of::<TaskStateSegment>() == 104);

// The TSS has no I/O permission bitmap: an iomap_base past its end means
// every port is denied to user mode.
#[allow(clippy::cast_possible_truncation)]
static mut TSS: TaskStateSegment = TaskStateSegment {
    _reserved0: 0,
    rsp: [0; 3],
    _reserved1: 0,
    ist: [0; 7],
    _reserved2: 0,
    _reserved3: 0,
    iomap_base: size_of::<TaskStateSegment>() as u16,
};

const IST_STACK_SIZE: usize = 16 * 1024;

#[repr(C, align(16))]
struct Stack([u8; IST_STACK_SIZE]);

static mut DOUBLE_FAULT_STACK: Stack = Stack([0; IST_STACK_SIZE]);
static mut NMI_STACK: Stack = Stack([0; IST_STACK_SIZE]);

/// Null, kernel code and data, user data and code, and the TSS (which takes
/// two entries).
const GDT_LEN: usize = 7;
static mut GDT: [u64; GDT_LEN] = [0, KERNEL_CODE, KERNEL_DATA, USER_DATA, USER_CODE, 0, 0];

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

/// Returns the two GDT entries of a 64-bit TSS descriptor.
// Intentional: the base and limit are split into their fields.
#[allow(clippy::cast_possible_truncation)]
fn tss_descriptor(base: u64, limit: u32) -> [u64; 2] {
    const PRESENT_AVAILABLE_TSS: u64 = 0x89;
    let limit = u64::from(limit);
    let low = (limit & 0xFFFF)
        | (base & 0xFF_FFFF) << 16
        | PRESENT_AVAILABLE_TSS << 40
        | ((limit >> 16) & 0xF) << 48
        | ((base >> 24) & 0xFF) << 56;
    [low, base >> 32]
}

fn stack_top(stack: *mut Stack) -> u64 {
    // SAFETY: One past the end of the same allocation.
    unsafe { stack.add(1) as u64 }
}

/// Loads the GDT and TSS, replacing the boot stub's GDT, and reloads every
/// segment register.
///
/// # Safety
///
/// Must only be called once, on the boot CPU, with interrupts disabled.
pub unsafe fn init() {
    unsafe extern "C" {
        /// The stack kmain runs on (see boot.s).
        static kernel_stack_top: u8;
    }

    unsafe {
        let tss = &raw mut TSS;
        (*tss).rsp[0] = &raw const kernel_stack_top as u64;
        (*tss).ist[usize::from(DOUBLE_FAULT_IST - 1)] = stack_top(&raw mut DOUBLE_FAULT_STACK);
        (*tss).ist[usize::from(NMI_IST - 1)] = stack_top(&raw mut NMI_STACK);

        let gdt = &raw mut GDT;
        let tss_index = usize::from(TSS_SELECTOR / 8);
        #[allow(clippy::cast_possible_truncation)]
        let [low, high] = tss_descriptor(tss as u64, size_of::<TaskStateSegment>() as u32 - 1);
        (*gdt)[tss_index] = low;
        (*gdt)[tss_index + 1] = high;

        #[allow(clippy::cast_possible_truncation)]
        let pointer = DescriptorTablePointer {
            limit: size_of::<[u64; GDT_LEN]>() as u16 - 1,
            base: gdt as u64,
        };
        asm!(
            "lgdt [{pointer}]",
            // cs can only be changed by a far jump, call or return: return
            // to the next instruction with the new selector.
            "push {code}",
            "lea {tmp}, [rip + 2f]",
            "push {tmp}",
            "retfq",
            "2:",
            "mov ds, {data:e}",
            "mov es, {data:e}",
            "mov ss, {data:e}",
            "xor {tmp:e}, {tmp:e}",
            "mov fs, {tmp:e}",
            "mov gs, {tmp:e}",
            "ltr {tss:x}",
            pointer = in(reg) &raw const pointer,
            code = const KERNEL_CODE_SELECTOR,
            data = in(reg) u32::from(KERNEL_DATA_SELECTOR),
            tss = in(reg) TSS_SELECTOR,
            tmp = out(reg) _,
        );
    }
}

/// Sets the stack the CPU switches to when an interrupt arrives in user mode.
///
/// # Safety
///
/// `top` must be the 16-byte aligned top of a kernel stack that stays valid
/// for as long as it is set, and that nothing else uses while in user mode.
#[allow(dead_code)]
pub unsafe fn set_kernel_stack(top: NonNull<u8>) {
    let tss = &raw mut TSS;
    unsafe { (*tss).rsp[0] = top.as_ptr() as u64 };
}
