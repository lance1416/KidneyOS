use core::arch::asm;

/// Generates a random int using the CPU's RDRAND instruction.
fn generate_random_i32() -> Option<i32> {
    let mut random_int: i32;
    let success: u8;

    unsafe {
        asm!(
            "rdrand {0:e}",
            "setc {1}",
            out(reg) random_int,
            out(reg_byte) success,
            options(nostack, nomem),
        );
    }

    (success == 1).then_some(random_int)
}

/// Fills a buffer with random bytes from the CPU's RDRAND instruction.
/// Returns the number of bytes written.
/// Currently no flags are implemented, if there is no random data available,
/// the random data generated so far is returned.
pub fn getrandom(buffer: &mut [u8], _flags: usize) -> isize {
    let mut bytes_written: usize = 0;

    for chunk in buffer.chunks_mut(size_of::<i32>()) {
        let Some(random_int) = generate_random_i32() else {
            break;
        };
        chunk.copy_from_slice(&random_int.to_le_bytes()[..chunk.len()]);
        bytes_written += chunk.len();
    }

    bytes_written.try_into().expect("byte count fits in isize")
}
