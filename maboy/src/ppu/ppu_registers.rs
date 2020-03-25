use super::lcdc::LCDC;
use super::lcds::LCDS;
use super::palette::Palette;
use crate::address::PpuReg;

/// A wrapper struct to group all PPU IO registers
#[derive(Clone)]
pub struct PPURegisters {
    pub ly: u8,
    pub lyc: u8,
    pub scx: u8,
    pub scy: u8,
    pub wy: u8,
    pub wx: u8,
    pub bgp: Palette,
    pub obp0: Palette,
    pub obp1: Palette,
    pub lcdc: LCDC,
    pub lcds: LCDS,
}

impl PPURegisters {
    pub fn new() -> PPURegisters {
        PPURegisters {
            ly: 0,
            lyc: 0,
            scx: 0,
            scy: 0,
            wy: 0,
            wx: 0,
            bgp: Palette(0),
            obp0: Palette(0),
            obp1: Palette(0),
            lcdc: LCDC(0),
            lcds: LCDS::new(),
        }
    }

    pub fn cpu_read(&self, reg: PpuReg) -> u8 {
        match reg {
            PpuReg::LCDC => self.lcdc.0,
            PpuReg::LCDS => self.lcds.read(),
            PpuReg::SCX => self.scx,
            PpuReg::SCY => self.scy,
            PpuReg::LY => self.ly,
            PpuReg::LYC => self.lyc,
            PpuReg::BGP => self.bgp.0,
            PpuReg::OBP0 => self.obp0.0,
            PpuReg::OBP1 => self.obp1.0,
            PpuReg::WY => self.wy,
            PpuReg::WX => self.wx,
        }
    }

    pub fn cpu_write(&mut self, reg: PpuReg, val: u8) {
        match reg {
            PpuReg::LCDC => self.lcdc.0 = val,
            PpuReg::LCDS => self.lcds.write(val),
            PpuReg::SCX => self.scx = val,
            PpuReg::SCY => self.scy = val,
            // There is an error in many sources claiming that LY resets to 0 on write. It doesn't.
            // Instead, any write is a NOOP. LY resets to 0 only when the LCD is turned off.
            PpuReg::LY => (),
            PpuReg::LYC => self.lyc = val,
            PpuReg::BGP => self.bgp.0 = val,
            PpuReg::OBP0 => self.obp0.0 = val,
            PpuReg::OBP1 => self.obp1.0 = val,
            PpuReg::WY => self.wy = val,
            PpuReg::WX => self.wx = val,
        }
    }
}
