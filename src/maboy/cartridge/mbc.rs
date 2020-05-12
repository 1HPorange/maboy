use super::cram::CartridgeRam;
use crate::maboy::address::{CRamAddr, CRomAddr};
use std::pin::Pin;

pub trait CartridgeMBC {
    type CRAM: CartridgeRam;

    fn read_rom(&self, addr: CRomAddr) -> u8;
    fn write_rom(&mut self, addr: CRomAddr, val: u8);

    fn read_cram(&self, addr: CRamAddr) -> u8;
    fn write_cram(&mut self, addr: CRamAddr, val: u8);
}

pub struct NoMBC<CRAM> {
    rom: Pin<Box<[u8]>>,
    cram: CRAM,
}

impl<CRAM: CartridgeRam> NoMBC<CRAM> {
    pub(super) fn new(rom: Box<[u8]>, cram: CRAM) -> NoMBC<CRAM> {
        NoMBC {
            rom: Pin::new(rom),
            cram,
        }
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
}
