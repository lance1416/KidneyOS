// NOTE: The video memory writer should be written to before the serial writer
// since the second may fail if the serial chip misbehaves there, but the video
// memory writer will always succeed.

// TODO: Modify uses of writers once interrupts are enabled, since we have to
// make sure they don't get left in an inconsistent state if we get interrupted
// in the middle of a print.

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        // SAFETY: Single core, no interrupts.
        unsafe {
            write!($crate::video_memory::video_memory_writer(), "{}", format_args!($($arg)*)).unwrap();
            write!($crate::serial::serial_writer(), "{}", format_args!($($arg)*)).unwrap();
        }
    }};
}

#[macro_export]
macro_rules! println {
    () => {{
        use core::fmt::Write;
        // SAFETY: Single core, no interrupts.
        unsafe {
            write!($crate::video_memory::video_memory_writer(), "\n").unwrap();
            write!($crate::serial::serial_writer(), "\n").unwrap();
        }
    }};
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        // SAFETY: Single core, no interrupts.
        unsafe {
            let video_memory_writer = $crate::video_memory::video_memory_writer();
            let serial_writer = $crate::serial::serial_writer();
            write!(video_memory_writer, "{}", format_args!($($arg)*)).unwrap();
            write!(video_memory_writer, "\n").unwrap();
            write!(serial_writer, "{}", format_args!($($arg)*)).unwrap();
            write!(serial_writer, "\n").unwrap();
        }
    }};
}

#[macro_export]
macro_rules! eprint {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        use $crate::video_memory::{Attribute, Colour};
        // SAFETY: Single core, no interrupts.
        unsafe {
            let video_memory_writer = $crate::video_memory::video_memory_writer();
            let prev_attribute = video_memory_writer.attribute;
            video_memory_writer.attribute = Attribute::new(Colour::Red, Colour::Black);
            write!(video_memory_writer, "{}", format_args!($($arg)*)).unwrap();
            video_memory_writer.attribute = prev_attribute;
            write!($crate::serial::serial_writer(), "{}", format_args!($($arg)*)).unwrap();
        }
    }};
}

#[macro_export]
macro_rules! eprintln {
    () => {{
        use core::fmt::Write;
        // SAFETY: Single core, no interrupts.
        unsafe {
            write!($crate::video_memory::video_memory_writer(), "\n").unwrap();
            write!($crate::serial::serial_writer(), "\n").unwrap();
        }
    }};
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        use $crate::video_memory::{Attribute, Colour};
        // SAFETY: Single core, no interrupts.
        unsafe {
            let video_memory_writer = $crate::video_memory::video_memory_writer();
            let prev_attribute = video_memory_writer.attribute;
            video_memory_writer.attribute = Attribute::new(Colour::Red, Colour::Black);
            write!(video_memory_writer, "{}", format_args!($($arg)*)).unwrap();
            write!(video_memory_writer, "\n").unwrap();
            video_memory_writer.attribute = prev_attribute;
            let serial_writer = $crate::serial::serial_writer();
            write!(serial_writer, "{}", format_args!($($arg)*)).unwrap();
            write!(serial_writer, "\n").unwrap();
        }
    }};
}

#[macro_export]
macro_rules! bochs_break {
    () => {
        #[cfg(debug_assertions)]
        // SAFETY: This is safe to use anywhere since it's a noop. The Bochs
        // emulator will break upon encountering it when magic_break: enabled=1
        // is enabled.
        unsafe {
            core::arch::asm!("xchg bx, bx")
        }
    };
}
