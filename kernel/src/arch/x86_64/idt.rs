//! The IDT, the interrupt entry stubs and the Rust interrupt dispatcher.
//!
//! https://wiki.osdev.org/Interrupt_Descriptor_Table#Structure_on_x86-64
//! https://wiki.osdev.org/Exceptions
//!
//! Every vector has a 16-byte stub (`interrupt_stubs + 16 * vector`) that
//! pushes a dummy error code if the CPU doesn't push one, then the vector
//! number, and jumps to `interrupt_common`. That saves every general purpose
//! register, so that the Rust handler gets the whole [`TrapFrame`] (system
//! calls and context switches will need it), aligns the stack to 16 bytes as
//! the SysV ABI requires at a call, calls [`interrupt_dispatch`], restores the
//! registers and returns with `iretq`. The kernel is built without SSE, so
//! there is no other register state to save yet.

use super::gdt::{DOUBLE_FAULT_IST, KERNEL_CODE_SELECTOR, NMI_IST};
use crate::{
    drivers::{ata::ata_interrupt, input::keyboard},
    interrupts::{pic, timer},
    threading::scheduling,
};
use core::{
    arch::{asm, global_asm},
    mem::size_of,
    sync::atomic::{AtomicU64, Ordering},
};
use kidneyos_shared::{eprintln, println};
use spin::Mutex;

pub const DIVIDE_ERROR: u8 = 0;
pub const NMI: u8 = 2;
pub const BREAKPOINT: u8 = 3;
pub const DOUBLE_FAULT: u8 = 8;
pub const GENERAL_PROTECTION: u8 = 13;
pub const PAGE_FAULT: u8 = 14;
/// First vector of the (remapped) PIC's IRQs.
pub const IRQ_BASE: u8 = pic::PIC1_OFFSET;
const IRQ_COUNT: u8 = 16;
/// Reserved for system calls; the only gate user mode may use with `int`.
pub const SYSCALL: u8 = 0x80;

const STUB_SIZE: usize = 16;

/// Registers as saved by `interrupt_common`, followed by what the stub and the
/// CPU pushed, from the lowest address up.
#[repr(C)]
#[derive(Debug)]
pub struct TrapFrame {
    pub r15: u64,
    pub r14: u64,
    pub r13: u64,
    pub r12: u64,
    pub r11: u64,
    pub r10: u64,
    pub r9: u64,
    pub r8: u64,
    pub rbp: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub rcx: u64,
    pub rbx: u64,
    pub rax: u64,
    pub vector: u64,
    /// The CPU's error code, or 0 for vectors without one.
    pub error_code: u64,
    // Pushed by the CPU.
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

const _: () = assert!(size_of::<TrapFrame>() == 22 * 8);

global_asm!(
    "
    .section .text.interrupt_stubs, \"ax\"
    .balign {stub_size}
    .global interrupt_stubs
interrupt_stubs:
    vector = 0
    .rept 256
    .balign {stub_size}
    // Exceptions that push an error code: #DF, #TS, #NP, #SS, #GP, #PF, #AC,
    // #CP, #VC and #SX.
    .if (vector == 8) || (vector == 10) || (vector == 11) || (vector == 12) || (vector == 13) || (vector == 14) || (vector == 17) || (vector == 21) || (vector == 29) || (vector == 30)
    .else
    pushq $0
    .endif
    pushq $vector
    jmp interrupt_common
    vector = vector + 1
    .endr

interrupt_common:
    push %rax
    push %rbx
    push %rcx
    push %rdx
    push %rsi
    push %rdi
    push %rbp
    push %r8
    push %r9
    push %r10
    push %r11
    push %r12
    push %r13
    push %r14
    push %r15
    cld
    mov %rsp, %rdi // &mut TrapFrame
    // rbx is callee-saved, so it survives the call. The stack has to be
    // 16-byte aligned at the call; the CPU only aligns it when it switches
    // stacks.
    mov %rsp, %rbx
    and $-16, %rsp
    call {dispatch}
    mov %rbx, %rsp
    pop %r15
    pop %r14
    pop %r13
    pop %r12
    pop %r11
    pop %r10
    pop %r9
    pop %r8
    pop %rbp
    pop %rdi
    pop %rsi
    pop %rdx
    pop %rcx
    pop %rbx
    pop %rax
    add $16, %rsp // vector and error code
    iretq
    ",
    stub_size = const STUB_SIZE,
    dispatch = sym interrupt_dispatch,
    options(att_syntax),
);

#[repr(C)]
#[derive(Clone, Copy)]
struct GateDescriptor {
    offset_low: u16,
    selector: u16,
    ist: u8,
    type_attributes: u8,
    offset_mid: u16,
    offset_high: u32,
    _reserved: u32,
}

const _: () = assert!(size_of::<GateDescriptor>() == 16);

impl GateDescriptor {
    const MISSING: Self = Self {
        offset_low: 0,
        selector: 0,
        ist: 0,
        type_attributes: 0,
        offset_mid: 0,
        offset_high: 0,
        _reserved: 0,
    };

    /// An interrupt gate (which clears IF on entry) to `handler`.
    // Intentional: the handler address is split into its fields.
    #[allow(clippy::cast_possible_truncation)]
    const fn interrupt_gate(handler: u64, dpl: u8, ist: u8) -> Self {
        const PRESENT: u8 = 0x80;
        const INTERRUPT_GATE: u8 = 0xE;
        Self {
            offset_low: handler as u16,
            selector: KERNEL_CODE_SELECTOR,
            ist,
            type_attributes: PRESENT | (dpl << 5) | INTERRUPT_GATE,
            offset_mid: (handler >> 16) as u16,
            offset_high: (handler >> 32) as u32,
            _reserved: 0,
        }
    }
}

static mut IDT: [GateDescriptor; 256] = [GateDescriptor::MISSING; 256];

#[repr(C, packed)]
struct DescriptorTablePointer {
    limit: u16,
    base: u64,
}

/// Fills in and loads the IDT.
///
/// # Safety
///
/// Must only be called once, after [`super::gdt::init`], with interrupts
/// disabled.
pub unsafe fn init() {
    unsafe extern "C" {
        static interrupt_stubs: u8;
    }

    unsafe {
        let stubs = &raw const interrupt_stubs as u64;
        let idt = &raw mut IDT;
        for (vector, gate) in (0..=u8::MAX).zip((*idt).iter_mut()) {
            // Exception gates can only be used by the CPU (or the kernel);
            // `int 0x80` from user mode will be the system call.
            let dpl = if vector == SYSCALL { 3 } else { 0 };
            // Double faults and NMIs can happen with a broken stack or in the
            // middle of switching stacks, so they always get their own.
            let ist = match vector {
                DOUBLE_FAULT => DOUBLE_FAULT_IST,
                NMI => NMI_IST,
                _ => 0,
            };
            let handler = stubs + u64::from(vector) * STUB_SIZE as u64;
            *gate = GateDescriptor::interrupt_gate(handler, dpl, ist);
        }

        #[allow(clippy::cast_possible_truncation)]
        let pointer = DescriptorTablePointer {
            limit: size_of::<[GateDescriptor; 256]>() as u16 - 1,
            base: idt as u64,
        };
        asm!("lidt [{}]", in(reg) &raw const pointer, options(readonly, nostack, preserves_flags));
    }
}

static TICKS: AtomicU64 = AtomicU64::new(0);

/// The number of timer interrupts so far.
pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

/// Handles a page fault the kernel expects, returning whether it did. If it
/// returns false, the page fault is fatal.
type PageFaultFixup = fn(cr2: u64, frame: &TrapFrame) -> bool;

static PAGE_FAULT_FIXUP: Mutex<Option<PageFaultFixup>> = Mutex::new(None);

/// Sets the handler for page faults that aren't fatal (there is no demand
/// paging yet). Must be called with interrupts enabled or outside of a page
/// fault, or it deadlocks.
pub fn set_page_fault_fixup(fixup: PageFaultFixup) {
    *PAGE_FAULT_FIXUP.lock() = Some(fixup);
}

/// Called by `interrupt_common` for every interrupt, with interrupts disabled.
extern "C" fn interrupt_dispatch(frame: &mut TrapFrame) {
    let Ok(vector) = u8::try_from(frame.vector) else {
        unreachable!("vector {:#X} out of range", frame.vector);
    };
    match vector {
        PAGE_FAULT => page_fault(frame),
        GENERAL_PROTECTION => {
            // A nonzero error code is the segment selector or IDT entry
            // involved; bit 1 set means it's an IDT vector (index << 3).
            fatal(frame, "general protection fault");
        }
        DOUBLE_FAULT => fatal(frame, "double fault"),
        NMI => eprintln!("non-maskable interrupt at rip {:#X}, ignored", frame.rip),
        BREAKPOINT => println!("breakpoint at rip {:#X}", frame.rip),
        0..32 => fatal(frame, exception_name(vector)),
        SYSCALL => {
            // TODO(x86_64): The system call ABI (M4).
            eprintln!(
                "int 0x80 (rax {:#X}): system calls are not implemented yet",
                frame.rax
            );
        }
        _ if (IRQ_BASE..IRQ_BASE + IRQ_COUNT).contains(&vector) => irq(vector - IRQ_BASE),
        _ => eprintln!("unexpected interrupt {vector:#X}, ignored"),
    }
}

/// Handles IRQ `irq`, like the i686 interrupt handlers do.
fn irq(irq: u8) {
    match irq {
        0 => {
            TICKS.fetch_add(1, Ordering::Relaxed);
            timer::step_sys_clock();
        }
        1 => keyboard::atkbd::on_keyboard_interrupt(),
        14 | 15 => ata_interrupt::on_ide_interrupt(irq),
        _ => {}
    }
    // TODO(x86_64): Spurious IRQ 7 and 15 must not be acknowledged (or only
    // on the master PIC, for 15).
    // SAFETY: This IRQ is being handled.
    unsafe { pic::send_eoi(irq) };

    // The timer preempts the running thread, and a keyboard or disk interrupt
    // likely makes a waiting thread runnable. Interrupts are only enabled once
    // the threads have started, so there is always a scheduler.
    if matches!(irq, 0 | 1 | 14 | 15) {
        scheduling::scheduler_yield_and_continue();
    }
}

fn page_fault(frame: &TrapFrame) {
    let cr2: u64;
    // SAFETY: Only reads cr2.
    unsafe { asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack, preserves_flags)) };

    let error = frame.error_code;
    eprintln!(
        "page fault: cr2={cr2:#018X} error code={error:#X} ({}, {}, {} mode{}) rip={:#X}",
        if error & 1 != 0 {
            "protection violation"
        } else {
            "page not present"
        },
        if error & (1 << 4) != 0 {
            "instruction fetch"
        } else if error & (1 << 1) != 0 {
            "write"
        } else {
            "read"
        },
        if error & (1 << 2) != 0 {
            "user"
        } else {
            "kernel"
        },
        if error & (1 << 3) != 0 {
            ", reserved bit set"
        } else {
            ""
        },
        frame.rip,
    );

    let fixup = *PAGE_FAULT_FIXUP.lock();
    if !fixup.is_some_and(|fixup| fixup(cr2, frame)) {
        fatal(frame, "unhandled page fault");
    }
}

fn fatal(frame: &TrapFrame, what: &str) -> ! {
    eprintln!(
        "{what} (vector {}, error code {:#X}) at rip {:#X}, cs {:#X}, rflags {:#X}, rsp {:#X}",
        frame.vector, frame.error_code, frame.rip, frame.cs, frame.rflags, frame.rsp
    );
    eprintln!("{frame:#X?}");
    panic!("{what}");
}

fn exception_name(vector: u8) -> &'static str {
    match vector {
        DIVIDE_ERROR => "divide error",
        1 => "debug exception",
        4 => "overflow",
        5 => "bound range exceeded",
        6 => "invalid opcode",
        7 => "device not available",
        10 => "invalid TSS",
        11 => "segment not present",
        12 => "stack-segment fault",
        16 => "x87 floating-point exception",
        17 => "alignment check",
        18 => "machine check",
        19 => "SIMD floating-point exception",
        20 => "virtualization exception",
        21 => "control protection exception",
        _ => "reserved exception",
    }
}
