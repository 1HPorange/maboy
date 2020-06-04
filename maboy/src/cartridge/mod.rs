mod cram;
mod desc;
mod mbc;
mod variant;

use super::address::{CRamAddr, CRomAddr};
use cram::CartridgeRam;
use mbc::CartridgeMBC;

pub use desc::CartridgeDesc;
pub use variant::CartridgeVariant;

pub struct Cartridge<MBC> {
    path: String,
    mbc: MBC,
}

impl<MBC: CartridgeMBC> Cartridge<MBC> {
    fn new(path: String, mbc: MBC) -> Cartridge<MBC> {
        Cartridge { path, mbc }
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

pub trait CartridgeMem {
    type MBC: CartridgeMBC;

    fn read_rom(&self, addr: CRomAddr) -> u8;
    fn write_rom(&mut self, addr: CRomAddr, val: u8);

    fn read_cram(&self, addr: CRamAddr) -> u8;
    fn write_cram(&mut self, addr: CRamAddr, val: u8);

    fn cram(&self) -> &[u8];
    fn cram_mut(&mut self) -> &mut [u8];
}

impl<MBC: CartridgeMBC> CartridgeMem for Cartridge<MBC> {
    type MBC = MBC;

    fn read_rom(&self, addr: CRomAddr) -> u8 {
        self.mbc.read_rom(addr)
    }

    fn write_rom(&mut self, addr: CRomAddr, val: u8) {
        self.mbc.write_rom(addr, val);
    }

    fn read_cram(&self, addr: CRamAddr) -> u8 {
        self.mbc.read_cram(addr)
    }

    fn write_cram(&mut self, addr: CRamAddr, val: u8) {
        self.mbc.write_cram(addr, val);
    }

    fn cram(&self) -> &[u8] {
        self.mbc.cram().data()
    }

    fn cram_mut(&mut self) -> &mut [u8] {
        self.mbc.cram_mut().data_mut()
    }
}

// TODO: See if this can be made any nicer

impl<T: CartridgeMem> CartridgeMem for &mut T {
    type MBC = T::MBC;

    fn read_rom(&self, addr: CRomAddr) -> u8 {
        T::read_rom(self, addr)
    }

    fn write_rom(&mut self, addr: CRomAddr, val: u8) {
        T::write_rom(self, addr, val);
    }

    fn read_cram(&self, addr: CRamAddr) -> u8 {
        T::read_cram(self, addr)
    }

    fn write_cram(&mut self, addr: CRamAddr, val: u8) {
        T::write_cram(self, addr, val);
    }

    fn cram(&self) -> &[u8] {
        T::cram(self)
    }

    fn cram_mut(&mut self) -> &mut [u8] {
        T::cram_mut(self)
    }
}
