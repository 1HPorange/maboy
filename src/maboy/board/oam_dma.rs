use crate::maboy::address::{ReadAddr, VideoMemAddr};
use crate::maboy::board::Board;
use crate::maboy::memory::cartridge_mem::CartridgeRam;

pub struct OamDma {
    src_addr: u16,
    oam_dst_idx: u8,
    read_buf: u8,
}

#[must_use]
pub struct OamReadCmd<'a>(u16, &'a mut OamDma);

impl OamDma {
    pub fn new() -> OamDma {
        OamDma {
            src_addr: 0,
            oam_dst_idx: 0xFF,
            read_buf: 0,
        }
    }

    pub fn is_active(&self) -> bool {
        self.oam_dst_idx < 0xA0
    }

    pub fn write_ff46(&mut self, val: u8) {
        if self.is_active() {
            unimplemented!("Attempting to start DMA while DMA is active - IDK what happens here");
        }

        if val > 0xf1 {
            unimplemented!("Illegal source address range for OAM DMA");
        }

        self.src_addr = (val as u16) * 0x100;
        self.oam_dst_idx = 0;
    }

    /// This function has a weird signature because OAM DMA kinda needs a mutable reference to itself
    /// AND to `Board`, which it is a member of. Rust doesn't like this. There are better ways to
    /// design this (like moving this OamDma onto `Emulator`), but then we would have uglier code
    /// in a lot more places. I think this solution is the lesser evil.

    pub fn advance_mcycle<CRAM: CartridgeRam>(board: &mut Board<CRAM>) {
        // TODO: Don't progress when CPU is in halt or stop
        if board.oam_dma.is_active() {
            // In the very first cycle of OAM DMA, we just fill the read buffer,
            // while in all other cycles, we first write out the buffer and
            // then fetch the next entry.

            if board.oam_dma.src_addr & 0xff != 0 {
                // Write most recently read byte
                board.ppu.write_video_mem(
                    VideoMemAddr::OAM(board.oam_dma.oam_dst_idx as u16),
                    board.oam_dma.read_buf,
                );
                board.oam_dma.oam_dst_idx += 1;
            }

            // Read next byte (we read one too much at the very end, but noone cares ;)
            board.oam_dma.read_buf = board.read8_instant(ReadAddr::from(board.oam_dma.src_addr));
            board.oam_dma.src_addr += 1;
        }
    }
}
