use crate::drivers::ata::ata_channel::CHANNEL_INTERRUPTS;
use kidneyos_shared::eprintln;

/// Handles IRQ `irq` (14 or 15, the primary and secondary channel's).
///
/// It mustn't lock the channel (see `ChannelInterrupt`).
pub fn on_ide_interrupt(irq: u8) {
    let channel_num = match irq {
        14 => 0,
        15 => 1,
        _ => return,
    };
    // SAFETY: This is the channel's interrupt.
    if !unsafe { CHANNEL_INTERRUPTS[channel_num].handle() } {
        eprintln!("IDE: Spurious interrupt on channel {channel_num} (ide{channel_num})");
    }
}
