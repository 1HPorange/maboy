use crate::maboy::address::{PpuReg, VideoMemAddr};
use std::pin::Pin;
pub struct PPU {
    ly_reg: u8,
    scy_reg: u8,
    vram: &'static mut [u8],
    oam: &'static mut [u8],
    vmem_backing: Pin<Box<[u8]>>,
}

const VRAM_LEN: usize = 0xA000 - 0x8000;
const OAM_LEN: usize = 0xFEA0 - 0xFE00;

impl PPU {
    pub fn new() -> PPU {
        use std::mem::transmute as forget_lifetime;

        let mut vmem_backing = Pin::new(vec![0; VRAM_LEN + OAM_LEN].into_boxed_slice());

        unsafe {
            PPU {
                ly_reg: 0,
                scy_reg: 0,
                vram: forget_lifetime(&mut vmem_backing[..VRAM_LEN]),
                oam: forget_lifetime(&mut vmem_backing[VRAM_LEN..VRAM_LEN + OAM_LEN]),
                vmem_backing,
            }
        }
    }

    pub fn read_reg(&self, reg: PpuReg) -> u8 {
        match reg {
            PpuReg::LCDC => unimplemented!(),
            PpuReg::SCY => self.scy_reg,
            PpuReg::LY => self.ly_reg,
        }
    }

    pub fn write_reg(&mut self, reg: PpuReg, val: u8) {
        match reg {
            PpuReg::LCDC => println!("Wrote to unimplemented LCDC"),
            PpuReg::SCY => self.scy_reg = val,
            PpuReg::LY => self.ly_reg = 0,
        }
    }

    pub fn read_video_mem(&self, addr: VideoMemAddr) -> u8 {
        // TODO: Access restrictions when in wrong mode
        match addr {
            VideoMemAddr::VRAM(addr) => self.vram[addr as usize],
            VideoMemAddr::OAM(addr) => self.oam[addr as usize],
        }
    }

    pub fn write_video_mem(&mut self, addr: VideoMemAddr, val: u8) {
        // TODO: Access restrictions when in wrong mode
        match addr {
            VideoMemAddr::VRAM(addr) => self.vram[addr as usize] = val,
            VideoMemAddr::OAM(addr) => self.oam[addr as usize] = val,
        }
    }

    pub fn advance_mcycle(&mut self) {
        // TODO: Implement properly

        self.ly_reg = 0x90;
    }
}
