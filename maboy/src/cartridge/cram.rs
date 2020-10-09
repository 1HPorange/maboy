//! All the different types of RAM that cartridges can contain. Some types of RAM
//! allow banking, similar to ROM banking. This banking is always triggered by
//! writing special values to certain sections of ROM, and requires an MBC that
//! supports it.
//!
//! Some RAM types support batteries; In that case, they should make their internal
//! state public via the [`Savegame`] trait if a battery is present.

use super::desc::RamSize;
use crate::{address::CRamAddr, Savegame};
use std::pin::Pin;

/// The interface between the RAM implementation and the MBC. The CPU will never
/// directly interact with this trait since the MBC can decide to disable RAM
/// temporarily; Thus, all communication goes through the MBC implementation.
pub trait CartridgeRam: Savegame {
    fn read(&self, addr: CRamAddr) -> u8;
    fn write(&mut self, addr: CRamAddr, val: u8);
    fn try_select_bank(&mut self, bank: u8);
}

/// Cartridges with no internal RAM should use this implementation, where every
/// write is a NOOP and every read yields 0xFF.
pub struct NoCRam;

impl Savegame for NoCRam {}

impl CartridgeRam for NoCRam {
    fn read(&self, _addr: CRamAddr) -> u8 {
        0xff
    }

    fn write(&mut self, _addr: CRamAddr, _val: u8) {}

    fn try_select_bank(&mut self, _bank: u8) {}
}

/// A fixed amount of RAM without banking support. Attempts to switch the RAM bank
/// compiles to a NOOP
pub struct CRamUnbanked {
    cram: Box<[u8]>,
    has_battery: bool,
}

impl CRamUnbanked {
    pub fn new(ram_size: RamSize, has_battery: bool) -> Self {
        let cram = match ram_size {
            RamSize::RamNone => panic!("Invalid ram size for CRAMUnbanked"),
            RamSize::Ram2Kb => vec![0; 0x800].into_boxed_slice(),
            RamSize::Ram8Kb => vec![0; 0x2000].into_boxed_slice(),
            RamSize::Ram32Kb => panic!("Invalid ram size for CRAMUnbanked"),
        };

        Self { cram, has_battery }
    }
}

impl Savegame for CRamUnbanked {
    fn savegame(&self) -> Option<&[u8]> {
        // TODO: Nicer API blocked by bool_to_option, look at other implementers too
        if self.has_battery {
            Some(&self.cram)
        } else {
            None
        }
    }

    fn savegame_mut(&mut self) -> Option<&mut [u8]> {
        if self.has_battery {
            Some(&mut self.cram)
        } else {
            None
        }
    }
}

impl CartridgeRam for CRamUnbanked {
    fn read(&self, addr: CRamAddr) -> u8 {
        *self.cram.get(addr.raw() as usize).unwrap_or(&0xff)
    }

    fn write(&mut self, addr: CRamAddr, val: u8) {
        if let Some(mem) = self.cram.get_mut(addr.raw() as usize) {
            *mem = val;
        }
    }

    fn try_select_bank(&mut self, _bank: u8) {}
}

/// MBC2 has a weird half-byte RAM, where only the lower 4 bits of each addressable byte are used.
/// We store this in a compressed format so we use all 8 bits of each byte. The lower half of the
/// byte contains the lower address.
pub struct CRamMBC2 {
    // TODO: Internally, this looks very much like CRAMUnbanked. The Savegame impl is also the same. See if it should be modularized
    cram: Box<[u8]>,
    has_battery: bool,
}

impl CRamMBC2 {
    pub fn new(has_battery: bool) -> Self {
        Self {
            cram: vec![0u8; 256].into_boxed_slice(),
            has_battery,
        }
    }
}

impl Savegame for CRamMBC2 {
    fn savegame(&self) -> Option<&[u8]> {
        if self.has_battery {
            Some(&self.cram)
        } else {
            None
        }
    }

    fn savegame_mut(&mut self) -> Option<&mut [u8]> {
        if self.has_battery {
            Some(&mut self.cram)
        } else {
            None
        }
    }
}

impl CartridgeRam for CRamMBC2 {
    fn read(&self, addr: CRamAddr) -> u8 {
        let shift = (addr.raw() & 1) * 4;
        let sub_addr = (addr.raw() >> 1) as usize;

        self.cram
            .get(sub_addr)
            .map(|val| (val >> shift) & 0xF)
            .unwrap_or(0xF) // TODO: Check if illegal reads here return 0xF or 0xFF (or something wild)
    }

    fn write(&mut self, addr: CRamAddr, val: u8) {
        let sub_addr = (addr.raw() >> 1) as usize;

        if let Some(mem) = self.cram.get_mut(sub_addr) {
            let shift = (addr.raw() & 1) * 4;

            // Clear the old content
            *mem &= 0xF0u8.rotate_left(shift as u32);

            // Write the new value
            *mem |= (val & 0xF) << shift;
        }
    }

    fn try_select_bank(&mut self, _bank: u8) {}
}

/// A large amount of RAM with banking support. Selection of the current RAM bank is done by the MBC.
/// Attempting to switch to a non-existent bank leaves the currently mapped bank unchanged.
pub struct CRamBanked {
    cram: Pin<Box<[u8]>>,
    mapped_bank: &'static mut [u8],
    has_battery: bool,
}

impl CRamBanked {
    pub fn new(has_battery: bool) -> Self {
        let mut cram = Pin::new(vec![0u8; 4 * 0x2000].into_boxed_slice());

        // We forget about the lifetime of the reference here, which is safe because we got the memory
        // inside a `Pin<Box<...>>` right here in the struct.
        let mapped_bank = unsafe { std::mem::transmute(&mut cram[..]) };

        Self {
            cram,
            mapped_bank,
            has_battery,
        }
    }
}

impl Savegame for CRamBanked {
    fn savegame(&self) -> Option<&[u8]> {
        if self.has_battery {
            Some(&self.cram)
        } else {
            None
        }
    }

    fn savegame_mut(&mut self) -> Option<&mut [u8]> {
        if self.has_battery {
            Some(&mut self.cram)
        } else {
            None
        }
    }
}

impl CartridgeRam for CRamBanked {
    fn read(&self, addr: CRamAddr) -> u8 {
        self.mapped_bank[addr.raw() as usize]
    }

    fn write(&mut self, addr: CRamAddr, val: u8) {
        self.mapped_bank[addr.raw() as usize] = val;
    }

    fn try_select_bank(&mut self, bank: u8) {
        if bank < 4 {
            // This transmute forgets the lifetime of the reference; This is safe because
            // self actually owns the memory and has it inside a pin, so this reference
            // will never become invalid
            self.mapped_bank =
                unsafe { std::mem::transmute(&mut self.cram[0x2000 * bank as usize..]) };
        }
    }
}
