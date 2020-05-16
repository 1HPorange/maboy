mod color;
mod lcdc;
mod lcds;
mod mem_frame;
mod oam;
mod palette;
mod pixel_queue;
mod ppu_registers;
mod sprite;
mod tile_data;
mod tile_maps;

use crate::maboy::address::{PpuReg, VideoMemAddr};
use crate::maboy::interrupt_system::{Interrupt, InterruptSystem};
use mem_frame::MemFrame;
use oam::OAM;
use palette::Palette;
use pixel_queue::PixelQueue;
use ppu_registers::PPURegisters;
use tile_data::TileData;
use tile_maps::TileMaps;

pub use mem_frame::MemPixel;

// TODO: This whole file is kind of messy. Rethink the state machine approach.

// TODO: Consistent naming of PPU vs Ppu

/// This thing is NOT cycle-accurate yet, because it is a nightmare to figure out.
/// However, if you understand how to do it, it should be possible without any
/// changes to the public-facing API of this struct.
pub struct PPU {
    frame_mcycle: u16,
    mode: Mode,
    reg: PPURegisters,
    /// Changes to the WY register are only recognized at frame start,
    /// so we save the original value in here for the duration of 1 frame.
    wy: u8,
    tile_data: TileData,
    tile_maps: TileMaps,
    oam: OAM,
    pixel_queue: PixelQueue,
    mem_frame: MemFrame,
    frame_ready: Option<FrameReady>,
}

enum FrameReady {
    VideoFrame,
    LcdOffFrame,
}
pub enum VideoFrameStatus<'a> {
    NotReady,
    LcdTurnedOff,
    Ready(&'a [MemPixel]),
}

#[derive(Copy, Clone, Debug)]
pub(super) enum Mode {
    LCDOff,

    /// Mode 0
    HBlank,

    /// Mode 1
    VBlank,

    /// Mode 2
    OAMSearch,

    /// Mode 3
    PixelTransfer,
}

impl PPU {
    pub fn new() -> PPU {
        PPU {
            frame_mcycle: 0,
            mode: Mode::LCDOff,
            reg: PPURegisters::new(),
            wy: 0,
            tile_data: TileData::new(),
            tile_maps: TileMaps::new(),
            oam: OAM::new(),
            pixel_queue: PixelQueue::new(),
            mem_frame: MemFrame::new(),
            frame_ready: None,
        }
    }

    pub fn advance_mcycle(&mut self, ir_system: &mut InterruptSystem) {
        if matches!(self.mode, Mode::LCDOff) {
            return;
        }

        match self.frame_mcycle {
            0 => {
                // TODO: Check if this cycle 0 stuff is necessary
                self.reg.ly = 0;
                self.mode = Mode::HBlank;
                self.update_lcds_mode(ir_system);
            },
            _ => unimplemented!()
        }

        self.frame_mcycle += 1;

        if self.frame_ready == asd {
            self.frame_ready = 0;
        }
    }

    pub fn query_frame_status(&mut self) -> VideoFrameStatus {
        match self.frame_ready.take() {
            Some(FrameReady::VideoFrame) => VideoFrameStatus::Ready(self.mem_frame.data()),
            Some(FrameReady::LcdOffFrame) => VideoFrameStatus::LcdTurnedOff,
            None => VideoFrameStatus::NotReady,
        }
    }

    pub fn read_reg(&self, reg: PpuReg) -> u8 {
        self.reg.cpu_read(reg)
    }

    pub fn write_reg(&mut self, ir_system: &mut InterruptSystem, reg: PpuReg, val: u8) {
        self.reg.cpu_write(reg, val);

        match reg {
            PpuReg::LCDC => self.notify_lcdc_changed(ir_system),
            _ => (),
        }
    }

    pub fn read_video_mem(&self, addr: VideoMemAddr) -> u8 {
        match addr {
            VideoMemAddr::TileData(addr) if self.vram_accessible() => self.tile_data[addr],
            VideoMemAddr::TileMaps(addr) if self.vram_accessible() => {
                self.tile_maps.mem[addr as usize]
            }
            VideoMemAddr::OAM(addr) if self.oam_accessible() => self.oam[addr],
            _ => 0xff,
        }
    }

    pub fn write_video_mem(&mut self, addr: VideoMemAddr, val: u8) {
        match addr {
            VideoMemAddr::TileData(addr) if self.vram_accessible() => self.tile_data[addr] = val,

            VideoMemAddr::TileMaps(addr) if self.vram_accessible() => {
                self.tile_maps.mem[addr as usize] = val
            }
            VideoMemAddr::OAM(addr) if self.oam_accessible() => self.oam[addr] = val,
            _ => (),
        }
    }

    fn vram_accessible(&self) -> bool {
        !matches!(self.mode, Mode::PixelTransfer(_))
    }

    fn oam_accessible(&self) -> bool {
        !matches!(self.mode, Mode::OAMSearch(_) | Mode::PixelTransfer(_))
    }

    fn notify_lcdc_changed(&mut self, ir_system: &mut InterruptSystem) {
        self.tile_maps.notify_lcdc_changed(self.reg.lcdc);
        self.oam.notify_lcdc_changed(self.reg.lcdc);

        if self.reg.lcdc.lcd_enabled() {
            if self.reg.ly < 144 && !matches!(self.mode, Mode::LCDOff) {
                log::warn!("Didn't wait for VBlank to disable LCD. This may cause damage on real hardware!")
            }

            if matches!(self.mode, Mode::LCDOff) {
                // Turn LCD on
                self.change_mode_with_interrupts(ir_system, Mode::OAMSearch;

                // When turning back on, we can trigger a potentially outstanding LYC interrupt
                self.set_ly(ir_system, 0);

                // Save value of WY register for the duration of a frame
                self.wy = self.reg.wy;
            }
        } else {
            if !matches!(self.mode, Mode::LCDOff) {
                // Turn LCD off
                self.frame_ready = Some(FrameReady::LcdOffFrame);

                // Do NOT use set_ly here, since we don't trigger LYC interrupts here
                // even if LYC = 0
                self.reg.ly = 0;

                self.change_mode_with_interrupts(ir_system, Mode::LCDOff);
            }
        }
    }

    fn set_ly_equals_lyc(&mut self, ir_system: &mut InterruptSystem, ly: u8) {
        self.reg.ly = ly;

        let lyc_equals_ly = ly == self.reg.lyc;
        self.reg.lcds.set_lyc_equals_ly(lyc_equals_ly);

        if lyc_equals_ly && self.reg.lcds.ly_coincidence_interrupt() {
            ir_system.schedule_interrupt(Interrupt::LcdStat);
        }
    }

    fn update_lcds_mode(&mut self, ir_system: &mut InterruptSystem) {
        self.reg.lcds.set_mode(self.mode);

        match self.mode {
            Mode::OAMSearch(_) if self.reg.lcds.oam_search_interrupt() => {
                ir_system.schedule_interrupt(Interrupt::LcdStat)
            }
            Mode::VBlank(_) if self.reg.lcds.v_blank_interrupt() => {
                ir_system.schedule_interrupt(Interrupt::LcdStat);
                ir_system.schedule_interrupt(Interrupt::VBlank);
            }
            Mode::VBlank(_) => ir_system.schedule_interrupt(Interrupt::VBlank),
            Mode::HBlank(_) if self.reg.lcds.h_blank_interrupt() => {
                ir_system.schedule_interrupt(Interrupt::LcdStat)
            }
            _ => (),
        }
    }
}
