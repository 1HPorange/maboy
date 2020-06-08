mod banked_rom;
mod mbc1;
mod mbc2;

// TODO: Consistent naming: CRam, Mbc, Ppu, Cpu, ...

use super::cram::CartridgeRam;
use crate::address::{CRamAddr, CRomAddr};

pub(super) use mbc1::MBC1;
pub(super) use mbc2::MBC2;

// TODO: consistent hex digit formatiing (0xff vs 0xFF)

pub trait CartridgeMBC {
    type CRAM: CartridgeRam;

    fn read_rom(&self, addr: CRomAddr) -> u8;
    fn write_rom(&mut self, addr: CRomAddr, val: u8);

    fn read_cram(&self, addr: CRamAddr) -> u8;
    fn write_cram(&mut self, addr: CRamAddr, val: u8);

    fn cram(&self) -> &Self::CRAM;
    fn cram_mut(&mut self) -> &mut Self::CRAM;
}

pub struct NoMBC<CRAM> {
    rom: Box<[u8]>,
    cram: CRAM,
}

impl<CRAM: CartridgeRam> NoMBC<CRAM> {
    pub(super) fn new(rom: Box<[u8]>, cram: CRAM) -> NoMBC<CRAM> {
        debug_assert!(rom.len() == 0x8000);
        NoMBC { rom, cram }
    }
}

impl<CRAM: CartridgeRam> CartridgeMBC for NoMBC<CRAM> {
    type CRAM = CRAM;

    fn read_rom(&self, addr: CRomAddr) -> u8 {
        let addr = match addr {
            CRomAddr::CROM0(n) => n,
            CRomAddr::CROMn(n) => n + 0x4000,
        };

        self.rom[addr as usize]
    }

    fn write_rom(&mut self, _addr: CRomAddr, _val: u8) {}

    fn read_cram(&self, addr: CRamAddr) -> u8 {
        self.cram.read(addr)
    }

    fn write_cram(&mut self, addr: CRamAddr, val: u8) {
        self.cram.write(addr, val);
    }

    fn cram(&self) -> &Self::CRAM {
        &self.cram
    }

    fn cram_mut(&mut self) -> &mut Self::CRAM {
        &mut self.cram
    }
}
