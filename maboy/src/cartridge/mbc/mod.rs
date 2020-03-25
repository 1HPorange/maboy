//! Support for all memory bank controllers (MBCs) that can sit inside of Game Boy
//! cartridges. They are usually used for switching out ROM banks and providing
//! cartridge ram (CRAM) to the system, but some of the also include special
//! features like a real-time clock.
//!
//! The Game Boy interacts with MBCs by writing values to ROM. These values do not
//! change ROM (hence the name ;), but are intercepted and interpreted by the MBC.
//! The exact interpretation depends on the type of MBC (MBC1, MBC2, ...).
//!
//! Alls MBCs implement the [`CartridgeMBC`] trait, which is used by the memory
//! controller to interface between the CPU and MBC. The actual implementations
//! are never made public beyond the [`crate:::maboy::cartridge`] module.

mod banked_rom;
mod mbc1;
mod mbc2;
mod mbc3;
mod rtc;

// TODO: Consistent naming: CRam, Mbc, Ppu, Cpu, ...
// TODO: consistent hex digit formatiing (0xff vs 0xFF)

use super::cram::CartridgeRam;
use crate::{
    address::{CRamAddr, CRomAddr},
    Metadata, Savegame,
};

pub(super) use mbc1::MBC1;
pub(super) use mbc2::MBC2;
pub(super) use mbc3::{MBC3Rtc, MBC3};

/// The public interface of all MBCs. The CPU only communicates with cartridge memory
/// via this trait.
pub trait CartridgeMBC: Savegame + Metadata {
    type CRAM: CartridgeRam;

    fn read_rom(&self, addr: CRomAddr) -> u8;
    fn write_rom(&mut self, addr: CRomAddr, val: u8);

    fn read_cram(&self, addr: CRamAddr) -> u8;
    fn write_cram(&mut self, addr: CRamAddr, val: u8);
}

/// Cartridges with no MBC (e.g. Tetris) can use this MBC implementation where any
/// writes to ROM compile to NOOPs
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

impl<CRAM: CartridgeRam> Savegame for NoMBC<CRAM> {
    fn savegame(&self) -> Option<&[u8]> {
        self.cram.savegame()
    }

    fn savegame_mut(&mut self) -> Option<&mut [u8]> {
        self.cram.savegame_mut()
    }
}

impl<CRAM: CartridgeRam> Metadata for NoMBC<CRAM> {}

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
