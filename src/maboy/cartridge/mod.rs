mod cram;
mod desc;
mod mbc;
mod variant;

use super::address::{CRamAddr, CRomAddr};
use cram::CartridgeRam;
use mbc::CartridgeMBC;
use std::pin::Pin;

pub use variant::CartridgeVariant;

pub struct Cartridge<MBC>(MBC);

impl<MBC: CartridgeMBC> Cartridge<MBC> {
    fn new(mbc: MBC) -> Cartridge<MBC> {
        Cartridge(mbc)
    }
}

// This thing might be replaced by MBC at some point, but for now we
// keep it in this trait incase some cartridges do some vodoo stuff.
pub trait CartridgeMem {
    type MBC: CartridgeMBC;

    fn read_rom(&self, addr: CRomAddr) -> u8;
    fn write_rom(&mut self, addr: CRomAddr, val: u8);

    fn read_cram(&self, addr: CRamAddr) -> u8;
    fn write_cram(&mut self, addr: CRamAddr, val: u8);
}

impl<MBC: CartridgeMBC> CartridgeMem for Cartridge<MBC> {
    type MBC = MBC;

    fn read_rom(&self, addr: CRomAddr) -> u8 {
        self.0.read_rom(addr)
    }

    fn write_rom(&mut self, addr: CRomAddr, val: u8) {
        self.0.write_rom(addr, val);
    }

    fn read_cram(&self, addr: CRamAddr) -> u8 {
        self.0.read_cram(addr)
    }

    fn write_cram(&mut self, addr: CRamAddr, val: u8) {
        self.0.write_cram(addr, val);
    }
}
