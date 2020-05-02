mod color;
mod lcdc;
mod lcds;
pub mod mem_frame;
mod palette;

use crate::maboy::address::{PpuReg, VideoMemAddr};
use crate::maboy::interrupt_system::{Interrupt, InterruptSystem};
use color::Color;
use lcdc::LCDC;
use lcds::LCDS;
use mem_frame::{MemFrame, MemPixel};
use palette::Palette;
use std::pin::Pin;

const VRAM_LEN: usize = 0xA000 - 0x8000;
const OAM_LEN: usize = 0xFEA0 - 0xFE00;

// TODO: This whole file is kind of messy. Rethink the state machine approach.

/// This thing is NOT cycle-accurate yet, because it is a nightmare to figure out.
/// However, if you understand how to do it, it should be possible without any
/// changes to the public-facing API of this struct.
pub struct PPU {
    ly_reg: u8,
    lyc_reg: u8,
    scx_reg: u8,
    scy_reg: u8,
    bgp_reg: Palette,
    mode: Mode,
    ly: u8,
    lcdc: LCDC,
    lcds: LCDS,
    vram: &'static mut [u8],
    oam: &'static mut [u8],
    mem_frame: MemFrame,
    vmem_backing: Pin<Box<[u8]>>,
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
        use std::mem::transmute as forget_lifetime;

        let mut vmem_backing = Pin::new(vec![0; VRAM_LEN + OAM_LEN].into_boxed_slice());

        unsafe {
            PPU {
                ly_reg: 0,
                lyc_reg: 0,
                scx_reg: 0,
                scy_reg: 0,
                bgp_reg: Palette(0), // TODO: Consider a nicer default, maybe
                mode: Mode::LCDOff(1),
                ly: 0,
                lcdc: LCDC(0),
                lcds: LCDS::new(),
                vram: forget_lifetime(&mut vmem_backing[..VRAM_LEN]),
                oam: forget_lifetime(&mut vmem_backing[VRAM_LEN..VRAM_LEN + OAM_LEN]),
                mem_frame: MemFrame::new(),
                vmem_backing,
            }
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
            Mode::OAMSearch(20) => self.change_mode(ir_system, Mode::PixelTransfer(1)),
            Mode::OAMSearch(n) => Mode::OAMSearch(n + 1),

            // Pixel Transfer
            Mode::PixelTransfer(43) => self.change_mode(ir_system, Mode::HBlank(1)),
            Mode::PixelTransfer(n) if n <= 40 => {
                self.push_pixels(n - 1);
                Mode::PixelTransfer(n + 1)
            }
            Mode::PixelTransfer(n) => Mode::PixelTransfer(n + 1),

            // HBlank
            Mode::HBlank(51) if self.ly == 143 => {
                self.set_ly(ir_system, self.ly + 1);
                self.change_mode(ir_system, Mode::VBlank(1))
            }
            Mode::HBlank(51) => {
                self.set_ly(ir_system, self.ly + 1);
                self.change_mode(ir_system, Mode::OAMSearch(1))
            }
            Mode::HBlank(n) => Mode::HBlank(n + 1),

            // VBlank
            Mode::VBlank(n) if n % 114 == 0 && n < 1140 => {
                self.set_ly(ir_system, self.ly + 1);
                Mode::VBlank(n + 1)
            }
            Mode::VBlank(1140) => {
                self.set_ly(ir_system, 0); // TODO: I think this happens earlier
                self.change_mode(ir_system, Mode::OAMSearch(1))
            }
            Mode::VBlank(n) => Mode::VBlank(n + 1),
        }
    }

    pub fn read_reg(&self, reg: PpuReg) -> u8 {
        match reg {
            PpuReg::LCDC => self.lcdc.0,
            PpuReg::LCDS => self.lcds.read(),
            PpuReg::SCX => self.scx_reg,
            PpuReg::SCY => self.scy_reg,
            PpuReg::LY => self.ly_reg,
            PpuReg::LYC => self.lyc_reg,
            PpuReg::BGP => self.bgp_reg.0,
        }
    }

    pub fn write_reg(&mut self, ir_system: &mut InterruptSystem, reg: PpuReg, val: u8) {
        match reg {
            PpuReg::LCDC => self.write_lcdc(ir_system, val),
            PpuReg::LCDS => self.lcds.write(val),
            PpuReg::SCX => self.scx_reg = val,
            PpuReg::SCY => self.scy_reg = val,
            PpuReg::LY => self.ly_reg = 0, // Not a typo! LY resets (temporarily) on write!
            PpuReg::LYC => self.lyc_reg = val,
            PpuReg::BGP => self.bgp_reg.0 = val,
        }
    }

    pub fn read_video_mem(&mut self, addr: VideoMemAddr) -> u8 {
        self.get_mem_addr(addr).map(|val| *val).unwrap_or(0xff)
    }

    pub fn write_video_mem(&mut self, addr: VideoMemAddr, val: u8) {
        if let Some(mut_ref) = self.get_mem_addr(addr) {
            *mut_ref = val;
        }
    }

    pub fn query_frame_status(&self) -> VideoFrameStatus {
        match self.mode {
            Mode::VBlank(1) => VideoFrameStatus::Ready(self.mem_frame.data()),
            Mode::LCDOff(1) => VideoFrameStatus::LcdTurnedOff,
            _ => VideoFrameStatus::NotReady,
        }
    }

    /// Retrieves an address in OAM or VRAM if currently accessible
    fn get_mem_addr(&mut self, addr: VideoMemAddr) -> Option<&mut u8> {
        match addr {
            VideoMemAddr::VRAM(addr) if !matches!(self.mode, Mode::PixelTransfer(_)) => {
                Some(&mut self.vram[addr as usize])
            }
            VideoMemAddr::OAM(addr) if !matches!(self.mode, Mode::OAMSearch(_) | Mode::PixelTransfer(_)) => {
                Some(&mut self.oam[addr as usize])
            }
            _ => None,
        }
    }

    fn write_lcdc(&mut self, ir_system: &mut InterruptSystem, val: u8) {
        self.lcdc.0 = val;
        if !self.lcdc.lcd_enabled() {
            if self.ly < 144 && !matches!(self.mode, Mode::LCDOff(_)) {
                log::warn!("Didn't wait for VBlank to disable LCD. This may cause damage on real hardware!")
            }
            self.change_mode(ir_system, Mode::LCDOff(1));
        } else if matches!(self.mode, Mode::LCDOff(_)) {
            self.ly = 0; // This isn't very nice to have... maybe should go into the mode transitions? Hmmm.
            self.change_mode(ir_system, Mode::OAMSearch(1));
        }
    }

    fn set_ly(&mut self, ir_system: &mut InterruptSystem, ly: u8) {
        self.ly = ly;
        self.ly_reg = ly;

        let lyc_equals_ly = ly == self.lyc_reg;
        self.lcds.set_lyc_equals_ly(lyc_equals_ly);

        if lyc_equals_ly && self.lcds.ly_coincidence_interrupt() {
            ir_system.schedule_interrupt(Interrupt::LcdStat);
        }
    }

    fn change_mode(&mut self, ir_system: &mut InterruptSystem, mode: Mode) -> Mode {
        self.mode = mode;
        self.lcds.set_mode(mode);

        match mode {
            Mode::OAMSearch(_) if self.lcds.oam_search_interrupt() => {
                ir_system.schedule_interrupt(Interrupt::LcdStat)
            }
            Mode::VBlank(_) if self.lcds.v_blank_interrupt() => {
                ir_system.schedule_interrupt(Interrupt::LcdStat);
                ir_system.schedule_interrupt(Interrupt::VBlank);
            }
            Mode::VBlank(_) => ir_system.schedule_interrupt(Interrupt::VBlank),
            Mode::HBlank(_) if self.lcds.h_blank_interrupt() => {
                ir_system.schedule_interrupt(Interrupt::LcdStat)
            }
            Mode::LCDOff(_) => self.ly_reg = 0,
            _ => (),
        }

        mode
    }

    fn push_pixels(&mut self, pixel_group_idx: u8) {
        let line = self.mem_frame.line(self.ly);

        let tm = &self.vram[self.lcdc.bg_tile_map_addr() as usize..];
        let td = &self.vram[self.lcdc.bg_window_tile_data_addr() as usize..];

        let tmy = self.ly.wrapping_add(self.scy_reg) / 8;
        let tdy = self.ly.wrapping_add(self.scy_reg) % 8;

        for px in pixel_group_idx * 4..pixel_group_idx * 4 + 4 {
            let tmx = px.wrapping_add(self.scx_reg) / 8;
            let tdx = 7 - (px.wrapping_add(self.scx_reg) % 8);

            let tile_id = self
                .lcdc
                .transform_tile_map_index(tm[tmy as usize * 32 + tmx as usize]);
            let tile_row_idx = tile_id as usize * 16 + tdy as usize * 2;

            let td_lower = td[tile_row_idx];
            let td_upper = td[tile_row_idx + 1];

            let col_raw = (((td_upper >> tdx) & 1) << 1) + ((td_lower >> tdx) & 1);

            let col = self.bgp_reg.apply(col_raw);

            line[px as usize] = MemPixel::from(col);
        }
    }
}
