#![no_std]

pub type Pid = u16;

#[repr(C)]
pub struct Timespec {
    pub tv_sec: i64,
    pub tv_nsec: i64,
}

pub mod defs;
pub use defs::*;

/// Defines a Rust program's entry point, `_start`, which calls `$main` (an
/// `extern "C" fn() -> !`).
///
/// `_start` isn't called but jumped to, with no return address on the stack:
/// on x86_64 the kernel starts programs with the stack pointer 16-byte
/// aligned, as the System V ABI specifies for the entry point, whereas a
/// function expects the stack pointer to be 8 bytes off that (because of its
/// return address). Calling `$main` from `_start` gets that right.
#[macro_export]
macro_rules! entry_point {
    ($main:path) => {
        #[unsafe(no_mangle)]
        #[unsafe(naked)]
        pub unsafe extern "C" fn _start() -> ! {
            ::core::arch::naked_asm!(
                "xor ebp, ebp", // the end of the frame pointer chain
                "call {main}",
                "ud2", // $main doesn't return.
                main = sym $main,
            )
        }
    };
}

/// The `int 0x80` system call instruction, for each architecture.
///
/// The system call number goes in eax/rax, the arguments in ebx, ecx, edx on
/// i686 and rdi, rsi, rdx (then r10, r8, r9) on x86_64, and the result comes
/// back in eax/rax. The kernel preserves every other register.
#[cfg(feature = "stubs")]
mod raw {
    use core::arch::asm;

    #[cfg(target_arch = "x86")]
    pub unsafe fn syscall3(number: usize, arg0: usize, arg1: usize, arg2: usize) -> isize {
        let result;
        unsafe {
            asm!(
                "int 0x80",
                inlateout("eax") number => result,
                in("ebx") arg0,
                in("ecx") arg1,
                in("edx") arg2,
            );
        }
        result
    }

    // rbx can't be an inline asm operand on x86_64 (LLVM uses it), which is
    // one reason the x86_64 ABI uses the System V argument registers.
    #[cfg(target_arch = "x86_64")]
    pub unsafe fn syscall3(number: usize, arg0: usize, arg1: usize, arg2: usize) -> isize {
        let result;
        unsafe {
            asm!(
                "int 0x80",
                inlateout("rax") number => result,
                in("rdi") arg0,
                in("rsi") arg1,
                in("rdx") arg2,
            );
        }
        result
    }

    pub unsafe fn syscall2(number: usize, arg0: usize, arg1: usize) -> isize {
        unsafe { syscall3(number, arg0, arg1, 0) }
    }

    pub unsafe fn syscall1(number: usize, arg0: usize) -> isize {
        unsafe { syscall3(number, arg0, 0, 0) }
    }

    pub unsafe fn syscall0(number: usize) -> isize {
        unsafe { syscall3(number, 0, 0, 0) }
    }

    #[cfg(target_arch = "x86")]
    pub unsafe fn exit(code: i32) -> ! {
        unsafe { asm!("int 0x80", in("eax") super::SYS_EXIT, in("ebx") code, options(noreturn)) }
    }

    #[cfg(target_arch = "x86_64")]
    pub unsafe fn exit(code: i32) -> ! {
        unsafe {
            asm!(
                "int 0x80",
                in("rax") super::SYS_EXIT,
                // Sign-extended, like every other argument.
                in("rdi") i64::from(code),
                options(noreturn),
            )
        }
    }
}

// The stubs are a C library of sorts for KidneyOS's user programs, and are
// exported under the names of the C functions. The kernel only uses the
// definitions above, and disables the `stubs` feature: otherwise these
// symbols would replace the C library's functions of the same names in the
// kernel's host tests. (cbindgen expands this crate with its default
// features, so include/kidneyos.h always has them.)
#[cfg(feature = "stubs")]
mod stubs {
    use super::raw::{self, syscall0, syscall1, syscall2, syscall3};
    use super::{
        Dirent, MMapOptions, Pid, SYS_CHDIR, SYS_CLOCK_GETTIME, SYS_CLOSE, SYS_DUP, SYS_DUP2,
        SYS_EXECVE, SYS_FORK, SYS_FSTAT, SYS_FTRUNCATE, SYS_GETCWD, SYS_GETDENTS, SYS_GETPID,
        SYS_GETPPID, SYS_GETRANDOM, SYS_LINK, SYS_LSEEK64, SYS_MKDIR, SYS_MMAP, SYS_MOUNT,
        SYS_NANOSLEEP, SYS_OPEN, SYS_PIPE, SYS_READ, SYS_RENAME, SYS_RMDIR, SYS_SCHED_YIELD,
        SYS_SYMLINK, SYS_SYNC, SYS_UNLINK, SYS_UNMOUNT, SYS_WAITPID, SYS_WRITE, Stat, Timespec,
    };
    use core::ffi::{c_char, c_void};

    // The results are i32 in the C API; the kernel's results are errors
    // (-errno) or values that fit.
    #[allow(clippy::cast_possible_truncation)]
    fn result_i32(result: isize) -> i32 {
        result as i32
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    fn result_pid(result: isize) -> Pid {
        result as Pid
    }

    // Signed arguments are passed sign-extended to the register width.
    #[allow(clippy::cast_sign_loss)]
    fn arg_i32(value: i32) -> usize {
        value as isize as usize
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn exit(code: i32) {
        unsafe { raw::exit(code) }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn fork() -> Pid {
        result_pid(unsafe { syscall0(SYS_FORK) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn read(fd: i32, buffer: *mut u8, count: usize) -> i32 {
        result_i32(unsafe { syscall3(SYS_READ, arg_i32(fd), buffer as usize, count) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn write(fd: i32, buffer: *const u8, count: usize) -> i32 {
        result_i32(unsafe { syscall3(SYS_WRITE, arg_i32(fd), buffer as usize, count) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn open(name: *const c_char, flags: usize) -> i32 {
        result_i32(unsafe { syscall2(SYS_OPEN, name as usize, flags) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn close(fd: i32) -> i32 {
        result_i32(unsafe { syscall1(SYS_CLOSE, arg_i32(fd)) })
    }

    // The offset is passed by pointer (and updated in place), since it doesn't
    // fit in a register on i686.
    #[unsafe(no_mangle)]
    pub extern "C" fn lseek64(fd: i32, offset: i64, whence: i32) -> i64 {
        let mut offset = offset;
        let result = result_i32(unsafe {
            syscall3(
                SYS_LSEEK64,
                arg_i32(fd),
                core::ptr::addr_of_mut!(offset) as usize,
                arg_i32(whence),
            )
        });
        if result < 0 { result.into() } else { offset }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn getcwd(buf: *mut i8, size: usize) -> i32 {
        result_i32(unsafe { syscall2(SYS_GETCWD, buf as usize, size) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn chdir(path: *const c_char) -> i32 {
        result_i32(unsafe { syscall1(SYS_CHDIR, path as usize) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn mkdir(path: *const c_char) -> i32 {
        result_i32(unsafe { syscall1(SYS_MKDIR, path as usize) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn fstat(fd: i32, statbuf: *mut Stat) -> i32 {
        result_i32(unsafe { syscall2(SYS_FSTAT, arg_i32(fd), statbuf as usize) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn unlink(path: *const c_char) -> i32 {
        result_i32(unsafe { syscall1(SYS_UNLINK, path as usize) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn link(source: *const c_char, dest: *const c_char) -> i32 {
        result_i32(unsafe { syscall2(SYS_LINK, source as usize, dest as usize) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn symlink(source: *const c_char, dest: *const c_char) -> i32 {
        result_i32(unsafe { syscall2(SYS_SYMLINK, source as usize, dest as usize) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn rename(source: *const c_char, dest: *const c_char) -> i32 {
        result_i32(unsafe { syscall2(SYS_RENAME, source as usize, dest as usize) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn rmdir(path: *const c_char) -> i32 {
        result_i32(unsafe { syscall1(SYS_RMDIR, path as usize) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn getdents(fd: i32, output: *mut Dirent, size: usize) -> i32 {
        result_i32(unsafe { syscall3(SYS_GETDENTS, arg_i32(fd), output as usize, size) })
    }

    // The size is passed in two halves, since it doesn't fit in a register on
    // i686.
    #[unsafe(no_mangle)]
    pub extern "C" fn ftruncate(fd: i32, size: u64) -> i32 {
        #[allow(clippy::cast_possible_truncation)]
        let size_lo = size as u32 as usize;
        #[allow(clippy::cast_possible_truncation)]
        let size_hi = (size >> 32) as u32 as usize;
        result_i32(unsafe { syscall3(SYS_FTRUNCATE, arg_i32(fd), size_lo, size_hi) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn sync() -> i32 {
        result_i32(unsafe { syscall0(SYS_SYNC) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn unmount(path: *const c_char) -> i32 {
        result_i32(unsafe { syscall1(SYS_UNMOUNT, path as usize) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn mount(
        device: *const c_char,
        target: *const c_char,
        filesystem_type: *const c_char,
    ) -> i32 {
        result_i32(unsafe {
            syscall3(
                SYS_MOUNT,
                device as usize,
                target as usize,
                filesystem_type as usize,
            )
        })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn waitpid(pid: Pid, stat: *mut i32, options: i32) -> Pid {
        result_pid(unsafe {
            syscall3(
                SYS_WAITPID,
                usize::from(pid),
                stat as usize,
                arg_i32(options),
            )
        })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn dup(fd: i32) -> i32 {
        result_i32(unsafe { syscall1(SYS_DUP, arg_i32(fd)) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn dup2(old_fd: i32, new_fd: i32) -> i32 {
        result_i32(unsafe { syscall2(SYS_DUP2, arg_i32(old_fd), arg_i32(new_fd)) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn pipe(fds: *mut i32) -> i32 {
        result_i32(unsafe { syscall1(SYS_PIPE, fds as usize) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn execve(
        filename: *const c_char,
        argv: *const *const c_char,
        envp: *const *const c_char,
    ) -> i32 {
        result_i32(unsafe { syscall3(SYS_EXECVE, filename as usize, argv as usize, envp as usize) })
    }

    // Seems to reference __kernel_timespec as the inputs for this syscall.
    // Not sure if we have this implemented.
    #[unsafe(no_mangle)]
    pub extern "C" fn nanosleep(duration: *const Timespec, remainder: *mut Timespec) -> i32 {
        result_i32(unsafe { syscall2(SYS_NANOSLEEP, duration as usize, remainder as usize) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn getpid() -> Pid {
        result_pid(unsafe { syscall0(SYS_GETPID) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn getppid() -> Pid {
        result_pid(unsafe { syscall0(SYS_GETPPID) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn scheduler_yield() -> i32 {
        result_i32(unsafe { syscall0(SYS_SCHED_YIELD) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn clock_gettime(clock_id: i32, timespec: *mut Timespec) -> i32 {
        result_i32(unsafe { syscall2(SYS_CLOCK_GETTIME, arg_i32(clock_id), timespec as usize) })
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn getrandom(buf: *mut i8, size: usize, flags: usize) -> i32 {
        result_i32(unsafe { syscall3(SYS_GETRANDOM, buf as usize, size, flags) })
    }

    // The six arguments are passed in a struct (by pointer), on every
    // architecture, since i686 doesn't have enough registers for them.
    #[unsafe(no_mangle)]
    pub extern "C" fn mmap(
        addr: *mut c_void,
        length: usize,
        prot: i32,
        flags: i32,
        fd: i32,
        offset: i64,
    ) -> *mut c_void {
        let options = MMapOptions {
            addr,
            length,
            prot,
            fd,
            flags,
            offset,
        };
        let result = unsafe { syscall1(SYS_MMAP, &raw const options as usize) };
        result as *mut c_void
    }
}

#[cfg(feature = "stubs")]
pub use stubs::*;
