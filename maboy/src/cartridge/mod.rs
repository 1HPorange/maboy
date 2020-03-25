//! This module contains everything needed to describe a physical cartridge to
//! the emulator. Frontends will usually want to start by creating a [`CartridgeVariant`]
//! from a ROM file on disk, then use the [`Savegame`] and [`Metadata`] traits to
//! attempt to load previously saved user-data.

mod cram;
mod desc;
mod mbc;
mod variant;

use super::address::{CRamAddr, CRomAddr};
use cram::CartridgeRam;
use mbc::CartridgeMBC;

pub use desc::CartridgeDesc;
pub use variant::{CartridgeParseError, CartridgeVariant};

/// The one and only implementation of [`Cartridge`]. Technically, we could directly
/// implement [`Cartridge`] for all MBCs, but by wrapping it here we keep the option
/// to store some metadata about the cartridge in later versions. If that turns out
/// to be unnceccesary, this struct will be removed.
///
/// Note that [`Cartridge`] is also implemented for `&mut CartridgeImpl<_>`, meaning
/// that you can pass a mutable reference to the emulator instead of passing by value.
/// This allows you to store savegames and metadata after the emulator has concluded
/// its run.
pub struct CartridgeImpl<MBC> {
    mbc: MBC,
}

impl<MBC: CartridgeMBC> CartridgeImpl<MBC> {
    fn new(mbc: MBC) -> CartridgeImpl<MBC> {
        CartridgeImpl { mbc }
    }
}

/// Interface between the CPU and the cartridge. This trait is mainly used so we don't
/// have to write out the MBC type parameter in a million places, and instead can just
/// accept any type that implements this trait.
pub trait Cartridge {
    /// The MBC (memory bank controller) used in the cartridge
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

/// This trait is used to provide access to the internal cartridge RAM. This is
/// necessary for providing savegame support for games.
///
/// For cartridges without RAM, as well as for cartridges with RAM but *without a
/// battery*, all methods of this trait should return `None`.
///
/// This trait could be implemented *only* for eligible cartridges by using some type-level
/// magic, but this would make cartridge handling for the frontend even more annoying
/// than it already is, since they would have to dispatch different types of cartridges.
/// to different methods. These trait methods are also not going to be called in a tight loop,
/// so optimizing for performance is not a priority.
///
/// # Examples
///
/// Loading a savegame from disk:
/// ```
/// if let Some(cram) = cartridge.savegame() {
///     fs::write(savegame_path, cram).expect("Could not write savegame to disk");
/// }
/// ```
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

/// Some cartridges can use external metadata to provide some functionality. MBC3, for
/// example, can use metadata to persist real-time clock state across multiple emulator
/// runs. This trait provides access to load and store such metadata, if present.
///
/// This trait is similar to ['Savegame'], which contains some useful further information.
pub trait Metadata {
    fn supports_metadata(&self) -> bool {
        false
    }

    fn serialize_metadata(&self) -> Result<Vec<u8>, CartridgeParseError> {
        Err(CartridgeParseError::MetadataNotSuported)
    }

    fn deserialize_metadata(&mut self, _data: Vec<u8>) -> Result<(), CartridgeParseError> {
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

impl<C: Savegame> Savegame for &mut C {
    fn savegame(&self) -> Option<&[u8]> {
        C::savegame(self)
    }

    fn savegame_mut(&mut self) -> Option<&mut [u8]> {
        C::savegame_mut(self)
    }
}

impl<C: Metadata> Metadata for &mut C {
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
