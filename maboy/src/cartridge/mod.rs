mod cram;
mod desc;
mod mbc;
mod variant;

use super::address::{CRamAddr, CRomAddr};
use cram::CartridgeRam;
use mbc::CartridgeMBC;

pub use desc::CartridgeDesc;
pub use variant::{CartridgeParseError, CartridgeVariant};

pub struct CartridgeImpl<MBC> {
    mbc: MBC,
}

impl<MBC: CartridgeMBC> CartridgeImpl<MBC> {
    fn new(mbc: MBC) -> CartridgeImpl<MBC> {
        CartridgeImpl { mbc }
    }
}

pub trait Cartridge: Savegame + Metadata {
    type MBC: CartridgeMBC;

    fn read_rom(&self, addr: CRomAddr) -> u8;
    fn write_rom(&mut self, addr: CRomAddr, val: u8);

    fn read_cram(&self, addr: CRamAddr) -> u8;
    fn write_cram(&mut self, addr: CRamAddr, val: u8);
}

impl<MBC: CartridgeMBC> Cartridge for CartridgeImpl<MBC> {
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
}

pub trait Savegame {
    fn savegame(&self) -> Option<&[u8]> {
        None
    }

    fn savegame_mut(&mut self) -> Option<&mut [u8]> {
        None
    }
}

impl<MBC: CartridgeMBC> Savegame for CartridgeImpl<MBC> {
    fn savegame(&self) -> Option<&[u8]> {
        self.mbc.savegame()
    }

    fn savegame_mut(&mut self) -> Option<&mut [u8]> {
        self.mbc.savegame_mut()
    }
}

pub trait Metadata {
    fn supports_metadata(&self) -> bool {
        false
    }

    fn serialize_metadata(&self) -> Result<Vec<u8>, CartridgeParseError> {
        Err(CartridgeParseError::MetadataNotSuported)
    }

    fn deserialize_metadata(&mut self, data: Vec<u8>) -> Result<(), CartridgeParseError> {
        Err(CartridgeParseError::MetadataNotSuported)
    }
}

impl<MBC: CartridgeMBC> Metadata for CartridgeImpl<MBC> {
    fn supports_metadata(&self) -> bool {
        self.mbc.supports_metadata()
    }

    fn serialize_metadata(&self) -> Result<Vec<u8>, CartridgeParseError> {
        self.mbc.serialize_metadata()
    }

    fn deserialize_metadata(&mut self, data: Vec<u8>) -> Result<(), CartridgeParseError> {
        self.mbc.deserialize_metadata(data)
    }
}

// Now, we implement all traits again for mutable references for caller convenience (and sanity)

impl<C: Cartridge> Savegame for &mut C {
    fn savegame(&self) -> Option<&[u8]> {
        C::savegame(self)
    }

    fn savegame_mut(&mut self) -> Option<&mut [u8]> {
        C::savegame_mut(self)
    }
}

impl<C: Cartridge> Metadata for &mut C {
    fn supports_metadata(&self) -> bool {
        C::supports_metadata(self)
    }

    fn serialize_metadata(&self) -> Result<Vec<u8>, CartridgeParseError> {
        C::serialize_metadata(self)
    }

    fn deserialize_metadata(&mut self, data: Vec<u8>) -> Result<(), CartridgeParseError> {
        C::deserialize_metadata(self, data)
    }
}

impl<C: Cartridge> Cartridge for &mut C {
    type MBC = C::MBC;

    fn read_rom(&self, addr: CRomAddr) -> u8 {
        C::read_rom(self, addr)
    }

    fn write_rom(&mut self, addr: CRomAddr, val: u8) {
        C::write_rom(self, addr, val)
    }

    fn read_cram(&self, addr: CRamAddr) -> u8 {
        C::read_cram(self, addr)
    }

    fn write_cram(&mut self, addr: CRamAddr, val: u8) {
        C::write_cram(self, addr, val)
    }
}
