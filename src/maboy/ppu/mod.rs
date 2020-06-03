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
use num_enum::UnsafeFromPrimitive;
use oam::OAM;
use palette::Palette;
use pixel_queue::PixelQueue;
use ppu_registers::PPURegisters;
use tile_data::TileData;
use tile_maps::TileMaps;

pub use lcdc::LCDC;
pub use lcds::LCDS;
pub use mem_frame::MemPixel;

// TODO: This whole file is kind of messy. Rethink the state machine approach.

// TODO: Consistent naming of PPU vs Ppu

/// This thing is NOT cycle-accurate yet, because it is a nightmare to figure out.
/// However, if you understand how to do it, it should be possible without any
/// changes to the public-facing API of this struct.
pub struct PPU {
    scanline_mcycle: u8,
    // How many mcycles mode 0 is delayed due to sprites in the current scanline
    scanline_sprite_delay: u8,
    // TODO: Think about if we still need this thing, or should just use LCDS
    mode: Mode,
    reg: PPURegisters,
    /// We need an internal copy of ly due to weird behaviour on scanline 153
    ly: u8,
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

#[derive(Copy, Clone, Debug, UnsafeFromPrimitive)]
#[repr(u8)]
pub enum Mode {
    LCDOff = 4,

    /// Mode 0
    HBlank = 0,

    /// Mode 1
    VBlank = 1,

    /// Mode 2
    OAMSearch = 2,

    /// Mode 3
    PixelTransfer = 3,
}

impl PPU {
    pub fn new() -> PPU {
        PPU {
            scanline_mcycle: 0,
            scanline_sprite_delay: 0,
            mode: Mode::LCDOff,
            reg: PPURegisters::new(),
            ly: 0,
            wy: 0,
            tile_data: TileData::new(),
            tile_maps: TileMaps::new(),
            oam: OAM::new(),
            pixel_queue: PixelQueue::new(),
            mem_frame: MemFrame::new(),
            frame_ready: None,
        }
    }

    // TODO: Accurate timings for Mode 2 interrupt
    pub fn advance_mcycle(&mut self, ir_system: &mut InterruptSystem) {
        if matches!(self.mode, Mode::LCDOff) {
            return;
        }

        match self.ly {
            0 => match self.scanline_mcycle {
                0 => {
                    self.reg.ly = 0;
                    // TODO: Check if this can cause HBlank interrupts. If yes, use
                    // self.update_mode(ir_system, Mode::HBlank);
                    self.mode = Mode::HBlank;
                    self.reg.lcds.set_mode(Mode::HBlank);
                }
                1 => {
                    self.update_mode(ir_system, Mode::OAMSearch);
                }
                21 => {
                    self.update_mode(ir_system, Mode::PixelTransfer);
                    self.oam.rebuild();
                    self.tile_data.rebuild();
                    let num_sprites = self.pixel_queue.push_scanline(
                        &self.reg,
                        &self.tile_maps,
                        &self.tile_data,
                        &self.oam,
                    );
                    self.scanline_sprite_delay = num_sprites * 2;
                }
                n if n > 21 && n <= 61 => {
                    self.pixel_queue.pop_pixel_quad(
                        &self.tile_data,
                        &self.tile_maps,
                        &self.reg,
                        self.mem_frame.line(self.ly),
                        n - 22,
                    );
                }
                n if n == 64 + self.scanline_sprite_delay => {
                    self.update_mode(ir_system, Mode::HBlank);
                }
                _ => (),
            },
            144 => match self.scanline_mcycle {
                0 => {
                    self.reg.ly = 144;
                    self.reg.lcds.set_lyc_equals_ly(false);
                }
                1 => {
                    ir_system.schedule_interrupt(Interrupt::VBlank);
                    self.update_lyc_equals_ly(ir_system, 144);
                    // TODO: VBLANK IR isn't triggered when IF is manually written to this cycle... JESUS
                    // Actually, this might already happen... hmmm
                    self.update_mode(ir_system, Mode::VBlank);
                }
                _ => (),
            },
            153 => match self.scanline_mcycle {
                0 => {
                    self.reg.ly = 153;
                    self.reg.lcds.set_lyc_equals_ly(false);
                }
                1 => {
                    self.reg.ly = 0;
                    self.update_lyc_equals_ly(ir_system, 153);
                }
                2 => self.reg.lcds.set_lyc_equals_ly(false),
                3 => {
                    self.update_lyc_equals_ly(ir_system, 0);
                }
                _ => (),
            },
            line if line < 144 => match self.scanline_mcycle {
                0 => {
                    self.reg.ly = line;
                    self.reg.lcds.set_lyc_equals_ly(false);
                }
                1 => {
                    self.update_mode(ir_system, Mode::OAMSearch);
                    self.update_lyc_equals_ly(ir_system, line);
                }
                21 => {
                    self.update_mode(ir_system, Mode::PixelTransfer);
                    self.oam.rebuild();
                    self.tile_data.rebuild();
                    let num_sprites = self.pixel_queue.push_scanline(
                        &self.reg,
                        &self.tile_maps,
                        &self.tile_data,
                        &self.oam,
                    );
                    self.scanline_sprite_delay = num_sprites * 2;
                }
                n if n > 21 && n <= 61 => {
                    self.pixel_queue.pop_pixel_quad(
                        &self.tile_data,
                        &self.tile_maps,
                        &self.reg,
                        self.mem_frame.line(self.ly),
                        n - 22,
                    );
                }
                n if n == 64 + self.scanline_sprite_delay => {
                    self.update_mode(ir_system, Mode::HBlank);
                }
                _ => (),
            },
            line => match self.scanline_mcycle {
                0 => {
                    self.reg.ly = line;
                    self.reg.lcds.set_lyc_equals_ly(false)
                }
                1 => {
                    self.update_lyc_equals_ly(ir_system, line);
                }
                _ => (),
            },
        };

        self.scanline_mcycle += 1;

        if self.scanline_mcycle == 114 {
            self.scanline_mcycle = 0;

            self.ly += 1;
            if self.ly == 154 {
                // TODO: Investigate this whole WY timing more
                self.wy = self.reg.wy;

                self.ly = 0;
                self.frame_ready = Some(FrameReady::VideoFrame);
            }
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
            PpuReg::LYC => self.update_lyc_equals_ly(ir_system, self.reg.ly), // TODO: Check if this behaviour is correct
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
        !matches!(self.mode, Mode::PixelTransfer)
    }

    fn oam_accessible(&self) -> bool {
        !matches!(self.mode, Mode::OAMSearch | Mode::PixelTransfer)
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

                // TODO: Investigate the timing of this...
                self.update_mode(ir_system, Mode::HBlank);

                // Save value of WY register for the duration of a frame
                self.wy = self.reg.wy;
            }
        } else {
            if !matches!(self.mode, Mode::LCDOff) {
                // TODO: TURN OFF function

                // Turn LCD off
                self.frame_ready = Some(FrameReady::LcdOffFrame);

                // Does NOT trigger LCD_STAT interrupt
                self.reg.ly = 0;

                // TODO: Move this into some sort TURN ON function
                self.ly = 0;
                self.scanline_mcycle = 0;

                self.update_mode(ir_system, Mode::LCDOff);
            }
        }
    }

    fn update_lyc_equals_ly(&mut self, ir_system: &mut InterruptSystem, ly: u8) {
        let ly_lyc_equal = ly == self.reg.lyc;

        if ly_lyc_equal
            && self.reg.lcds.ly_coincidence_interrupt()
            && (!self.reg.lcds.any_conditions_met())
        {
            ir_system.schedule_interrupt(Interrupt::LcdStat);
        }

        self.reg.lcds.set_lyc_equals_ly(ly_lyc_equal);
    }

    fn update_mode(&mut self, ir_system: &mut InterruptSystem, mode: Mode) {
        self.mode = mode;

        if !self.reg.lcds.any_conditions_met() {
            match mode {
                Mode::OAMSearch if self.reg.lcds.oam_search_interrupt() => {
                    ir_system.schedule_interrupt(Interrupt::LcdStat)
                }
                Mode::VBlank => {
                    if self.reg.lcds.v_blank_interrupt() {
                        ir_system.schedule_interrupt(Interrupt::LcdStat);
                    }
                }
                Mode::HBlank if self.reg.lcds.h_blank_interrupt() => {
                    ir_system.schedule_interrupt(Interrupt::LcdStat)
                }
                _ => (),
            }
        }

        self.reg.lcds.set_mode(mode);
    }
}
