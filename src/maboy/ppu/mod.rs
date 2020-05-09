mod color;
mod lcdc;
mod lcds;
pub mod mem_frame;
mod oam;
mod palette;
mod pixel_queue;
mod ppu_registers;
mod sprite;
mod tile_data;
mod tile_maps;

use crate::maboy::address::{PpuReg, VideoMemAddr};
use crate::maboy::interrupt_system::{Interrupt, InterruptSystem};
use lcdc::LCDC;
use lcds::LCDS;
use mem_frame::{MemFrame, MemPixel};
use oam::OAM;
use palette::Palette;
use pixel_queue::PixelQueue;
use ppu_registers::PPURegisters;
use tile_data::TileData;
use tile_maps::TileMaps;

const VRAM_LEN: usize = 0xA000 - 0x8000;
const OAM_LEN: usize = 0xFEA0 - 0xFE00;

// TODO: This whole file is kind of messy. Rethink the state machine approach.

// TODO: Consistent naming of PPU vs Ppu

/// This thing is NOT cycle-accurate yet, because it is a nightmare to figure out.
/// However, if you understand how to do it, it should be possible without any
/// changes to the public-facing API of this struct.
pub struct PPU {
    reg: PPURegisters,
    mode: Mode, // TODO: Extract this thing into its own struct
    /// Changes to the WY register are only recognized at frame start,
    /// so we save the original value in here for the duration of 1 frame.
    wy: u8,
    tile_data: TileData,
    tile_maps: TileMaps,
    oam: OAM,
    pixel_queue: PixelQueue,
    mem_frame: MemFrame,
}

pub enum VideoFrameStatus<'a> {
    NotReady,
    LcdTurnedOff,
    Ready(&'a [MemPixel]),
}

#[derive(Copy, Clone)]
pub(super) enum Mode {
    LCDOff(u8),

    /// Mode 0
    HBlank(u8),

    /// Mode 1
    VBlank(u16),

    /// Mode 2
    OAMSearch(u8),

    /// Mode 3
    PixelTransfer(u8),
}

impl PPU {
    pub fn new() -> PPU {
        PPU {
            reg: PPURegisters::new(),
            mode: Mode::LCDOff(1),
            wy: 0,
            tile_data: TileData::new(),
            tile_maps: TileMaps::new(),
            oam: OAM::new(),
            pixel_queue: PixelQueue::new(),
            mem_frame: MemFrame::new(),
        }
    }

    pub fn advance_mcycle(&mut self, ir_system: &mut InterruptSystem) {
        self.mode = match self.mode {
            Mode::LCDOff(1) => Mode::LCDOff(2),
            // We don't count up any further here on purpose, as this
            // could lead to an overflow at some point. We just need the
            // count on this enum variant to make sure that we only
            // trigger VideoFrameStatus::LcdTurnedOff once:
            Mode::LCDOff(n) => Mode::LCDOff(n),

            // OAM Search
            Mode::OAMSearch(1) => {
                self.oam.rebuild();

                self.pixel_queue.push_scanline(
                    &self.reg,
                    &self.tile_maps,
                    &self.tile_data,
                    &self.oam,
                );
                Mode::OAMSearch(2)
            }
            Mode::OAMSearch(20) => {
                self.tile_data.rebuild();
                self.change_mode_with_interrupts(ir_system, Mode::PixelTransfer(1))
            }
            Mode::OAMSearch(n) => Mode::OAMSearch(n + 1),

            // Pixel Transfer
            Mode::PixelTransfer(43) => self.change_mode_with_interrupts(ir_system, Mode::HBlank(1)),
            Mode::PixelTransfer(n) if n <= 40 => {
                self.pixel_queue.pop_pixel_quad(
                    &self.tile_data,
                    &self.tile_maps,
                    &self.reg,
                    self.mem_frame.line(self.reg.ly),
                    n - 1,
                );
                Mode::PixelTransfer(n + 1)
            }
            Mode::PixelTransfer(n) => Mode::PixelTransfer(n + 1),

            // HBlank
            Mode::HBlank(51) if self.reg.ly == 143 => {
                self.set_ly(ir_system, self.reg.ly + 1);
                self.change_mode_with_interrupts(ir_system, Mode::VBlank(1))
            }
            Mode::HBlank(51) => {
                self.set_ly(ir_system, self.reg.ly + 1);
                self.change_mode_with_interrupts(ir_system, Mode::OAMSearch(1))
            }
            Mode::HBlank(n) => Mode::HBlank(n + 1),

            // VBlank
            Mode::VBlank(1140) => {
                self.set_ly(ir_system, 0); // TODO: I think this happens earlier

                // Save value of WY register for the duration of a frame
                self.wy = self.reg.wy;

                self.change_mode_with_interrupts(ir_system, Mode::OAMSearch(1))
            }
            Mode::VBlank(n) if n % 114 == 0 => {
                self.set_ly(ir_system, self.reg.ly + 1);
                Mode::VBlank(n + 1)
            }
            Mode::VBlank(n) => Mode::VBlank(n + 1),
        }
    }

    pub fn query_frame_status(&self) -> VideoFrameStatus {
        match self.mode {
            Mode::VBlank(1) => VideoFrameStatus::Ready(self.mem_frame.data()),
            Mode::LCDOff(1) => VideoFrameStatus::LcdTurnedOff,
            _ => VideoFrameStatus::NotReady,
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
            if self.reg.ly < 144 && !matches!(self.mode, Mode::LCDOff(_)) {
                log::warn!("Didn't wait for VBlank to disable LCD. This may cause damage on real hardware!")
            }

            if matches!(self.mode, Mode::LCDOff(_)) {
                // Turn LCD on
                self.change_mode_with_interrupts(ir_system, Mode::OAMSearch(1));

                // When turning back on, we can trigger a potentially outstanding LYC interrupt
                self.set_ly(ir_system, 0);

                // Save value of WY register for the duration of a frame
                self.wy = self.reg.wy;
            }
        } else {
            if !matches!(self.mode, Mode::LCDOff(_)) {
                // Turn LCD off

                // Do NOT use set_ly here, since we don't trigger LYC interrupts here
                // even if LYC = 0
                self.reg.ly = 0;

                self.change_mode_with_interrupts(ir_system, Mode::LCDOff(1));
            }
        }
    }

    fn set_ly(&mut self, ir_system: &mut InterruptSystem, ly: u8) {
        self.reg.ly = ly;

        let lyc_equals_ly = ly == self.reg.lyc;
        self.reg.lcds.set_lyc_equals_ly(lyc_equals_ly);

        if lyc_equals_ly && self.reg.lcds.ly_coincidence_interrupt() {
            ir_system.schedule_interrupt(Interrupt::LcdStat);
        }
    }

    // TODO: Replace with individual "change-to-mode" functions
    fn change_mode_with_interrupts(&mut self, ir_system: &mut InterruptSystem, mode: Mode) -> Mode {
        self.mode = mode;
        self.reg.lcds.set_mode(mode);

        match mode {
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

        mode
    }
}
