//! Implementation of the Game Boy's pixel processing unit (PPU). The PPU has
//! some fairly complex behaviour, especially in edge-cases like turning the
//! LCD on or off, or weird scanline timings. It is also *driven by the CPU*,
//! meaning that is has to maintain an internal state machine to know what to
//! do each cycle. For more info, see [`PPU`].

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

use crate::address::{PpuReg, VideoMemAddr};
use crate::interrupt_system::{Interrupt, InterruptSystem};
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

// TODO: Replace some debug logs with PpuEvt
// TODO: This whole file is kind of messy. Rethink the state machine approach.
// TODO: Consistent naming of PPU vs Ppu

/// Holds the PPU state as well as VRAM and OAM RAM. Also manages CPU access to all
/// PPU-related IO registers, and blocks VRAM and OAM RAM from being accessed outside
/// of the allowed PPU modes.
///
/// Since the PPU is driven by the CPU, this struct maintains an internal state machine
/// that is advanced by one machine cycle at a time via [`Board::advance_cycle`].
/// This current state of this state machine is determined by the combination of
/// - [`self.scanline_mcycle`]
/// - [`self.ly`] (not to confuse with the LY register, which is *sometimes* different)
/// - [`self.mode`]
pub struct PPU {
    /// Current mcycle within one *internal* scanline (!= LY register value) between
    /// 0..114 (exclusive). Does no weird thing in scanline 153, unlike the LY register.
    scanline_mcycle: u8,
    /// How many mcycles mode 0 is delayed in the current scanline due to the number of
    /// sprites in the current scanline
    scanline_sprite_delay: u8,
    /// *Internal* mode of the PPU, used to determine state machine actions and CPU
    /// access restrictions on VRAM and OAM RAM. Not to be confiused with the mode
    /// bits in LCDS, which can sometimes report a different value.
    mode: Mode,
    /// IO registers that directly affect PPU operation.
    reg: PPURegisters,
    /// *Internal* vertical scanline, not to be confused with the LY register. This is
    /// necessary because the LY register has some odd timing-related behaviour that
    /// would make it difficult to use as a state-machine state indicator.
    ///
    /// This field can have values in the range 0..=153
    ly: u8,
    /// Cached value of the WY register. Apparently, the effect of this register persists
    /// during an entire frame, but the backing value can still be changed arbitrarily.
    /// This field saves the value of WY at the beginning of a frame.
    wy: u8,
    /// The part of VRAM responsible for the content of each tile (0x8000 - 0x97FF)
    tile_data: TileData,
    /// The part of VRAM responsible for indexes into the tile data that are rendered on
    /// screen (0x9800 - 0x9FFF).
    tile_maps: TileMaps,
    /// Sprite memory
    oam: OAM,
    /// Artificial construct that helps to draw a scanline more efficiently
    pixel_queue: PixelQueue,
    /// The backing data of the current frame. This data gets exposed via the API at the
    /// beginning of each VBlank period.
    mem_frame: MemFrame,
    /// Used as an indicator for the frontend whether a frame is ready / should be rendered.
    frame_ready: Option<FrameReady>,
    /// Used to skip the drawing of frames in case the LCD was just turned on. This behaviour
    /// is present on hardware.
    skip_frames: u8,
}

/// The (internally stored) type of frame that is ready to be drawn by the frontend
enum FrameReady {
    /// A normal video frame
    VideoFrame,
    /// A blank frame, indicating the the LCD has been turned off
    LcdOffFrame,
}

/// The type of frame *and* frame content that the frontend should draw
pub enum VideoFrameStatus<'a> {
    /// Frontend should not draw anything
    NotReady,
    /// Frontend should draw a blank frame
    LcdTurnedOff,
    /// Frontend should draw the content of the frame
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
            skip_frames: 0,
        }
    }

    /// Used to make internal state visible to debugger
    pub fn ly_internal(&self) -> u8 {
        self.ly
    }

    /// Used to make internal state visible to debugger
    pub fn wy_internal(&self) -> u8 {
        self.wy
    }

    // TODO: Accurate timings for Mode 2 interrupt.. This is hard!
    pub fn advance_mcycle(&mut self, ir_system: &mut InterruptSystem) {
        // We don't do anything if the LCD is turned off
        if matches!(self.mode, Mode::LCDOff) {
            return;
        }

        // React according to internal state machine
        match self.ly {
            0 => match self.scanline_mcycle {
                0 => {
                    // TODO: Investigate the timing of this further
                    // Save the current value of the WY register for the duration of the frame
                    self.wy = self.reg.wy;

                    self.reg.ly = 0;
                    // TODO: Check if this can cause HBlank interrupts. If yes, use
                    // self.update_mode(ir_system, Mode::HBlank);
                    self.mode = Mode::HBlank;
                    self.reg.lcds.set_mode(Mode::HBlank);
                }
                1 => {
                    self.update_mode_with_interrupts(ir_system, Mode::OAMSearch);
                }
                21 => {
                    self.update_mode_with_interrupts(ir_system, Mode::PixelTransfer);
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
                    self.update_mode_with_interrupts(ir_system, Mode::HBlank);
                }
                _ => (),
            },
            144 => match self.scanline_mcycle {
                0 => {
                    self.reg.ly = 144;
                    self.reg.lcds.set_lyc_equals_ly(false);
                }
                1 => {
                    log::debug!("Rendered frame");

                    if self.skip_frames == 0 {
                        self.frame_ready = Some(FrameReady::VideoFrame);
                    } else {
                        log::debug!("Skipped frame display");
                        self.skip_frames -= 1;
                    }

                    ir_system.schedule_interrupt(Interrupt::VBlank);
                    self.update_lyc_equals_ly(ir_system, 144);
                    // TODO: VBLANK IR isn't triggered when IF is manually written to this cycle... JESUS
                    // Actually, this might already happen... hmmm
                    self.update_mode_with_interrupts(ir_system, Mode::VBlank);
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
                    self.update_mode_with_interrupts(ir_system, Mode::OAMSearch);
                    self.update_lyc_equals_ly(ir_system, line);
                }
                21 => {
                    self.update_mode_with_interrupts(ir_system, Mode::PixelTransfer);
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
                    self.update_mode_with_interrupts(ir_system, Mode::HBlank);
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

        // Advance internal state machine
        self.scanline_mcycle += 1;
        if self.scanline_mcycle == 114 {
            self.scanline_mcycle = 0;

            self.ly += 1;
            if self.ly == 154 {
                self.ly = 0;
            }
        }
    }

    /// See [`Emulator::query_video_frame_status`]
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

        // TODO: Trigger the false LCD Stat interrupts that seem to occur when writing to LCDS
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
            _ => {
                log::debug!(
                    "Failed read from video memory at {:?} in mode {:?}",
                    addr,
                    self.mode
                );
                0xff
            }
        }
    }

    pub fn write_video_mem(&mut self, addr: VideoMemAddr, val: u8) {
        match addr {
            VideoMemAddr::TileData(addr) if self.vram_accessible() => self.tile_data[addr] = val,

            VideoMemAddr::TileMaps(addr) if self.vram_accessible() => {
                self.tile_maps.mem[addr as usize] = val
            }
            VideoMemAddr::OAM(addr) if self.oam_accessible() => self.oam[addr] = val,
            _ => log::debug!(
                "Failed write to video memory at {:?} in mode {:?}",
                addr,
                self.mode
            ),
        }
    }

    /// Necessary for OAM DMA. Ignores the PPU mode and just writes to video memory.
    pub fn write_video_mem_unchecked(&mut self, addr: VideoMemAddr, val: u8) {
        match addr {
            VideoMemAddr::TileData(addr) => self.tile_data[addr] = val,

            VideoMemAddr::TileMaps(addr) => self.tile_maps.mem[addr as usize] = val,
            VideoMemAddr::OAM(addr) => self.oam[addr] = val,
        }
    }

    fn vram_accessible(&self) -> bool {
        !matches!(self.mode, Mode::PixelTransfer)
    }

    fn oam_accessible(&self) -> bool {
        !matches!(self.mode, Mode::OAMSearch | Mode::PixelTransfer)
    }

    /// To be called after the CPU writes to LCDC. Notifies all subsystems of the change and
    /// handles the logic for turning the LCD on and off.
    fn notify_lcdc_changed(&mut self, ir_system: &mut InterruptSystem) {
        self.tile_maps.notify_lcdc_changed(self.reg.lcdc);
        self.oam.notify_lcdc_changed(self.reg.lcdc);

        if self.reg.lcdc.lcd_enabled() {
            if matches!(self.mode, Mode::LCDOff) {
                // Turn LCD on
                log::info!("Turned LCD on");

                // TODO: 5+ frames skipped fixes a graphical glitch in Pokemon Red
                // that renders garbage for a few frames. On actual hardware, however,
                // only 1 frame is supposed to be skipped ...
                self.skip_frames = 1;

                // TODO: Investigate the timing of this...
                self.update_mode_with_interrupts(ir_system, Mode::HBlank);
            }
        } else {
            if !matches!(self.mode, Mode::LCDOff) {
                if self.reg.ly < 144 {
                    log::warn!("Didn't wait for VBlank to disable LCD (LY = {}). This may cause damage on real hardware!", self.ly);
                }

                // Turn LCD off
                log::info!("Turned LCD off");

                self.frame_ready = Some(FrameReady::LcdOffFrame);

                // Does NOT trigger LCD_STAT interrupt
                self.reg.ly = 0;

                // TODO: Move this into some sort TURN ON function
                self.ly = 0;
                self.scanline_mcycle = 0;

                self.update_mode_with_interrupts(ir_system, Mode::LCDOff);
            }
        }
    }

    /// Call this whenever a LCD Stat interrupt caused by LY==LYC could happen. The `ly`
    /// parameter is the value that the LYC register is compared against to determine
    /// whether to throw the interrupt.
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

    /// Updates the internal mode and the LCDS register and triggers any potential LCD Stat interrupts.
    fn update_mode_with_interrupts(&mut self, ir_system: &mut InterruptSystem, mode: Mode) {
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
