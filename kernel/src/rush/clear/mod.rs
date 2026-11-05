use kidneyos_shared::video_memory::video_memory_writer;

/// Clear the screen.
pub fn clear() {
    unsafe { video_memory_writer().clear_screen() };
}
