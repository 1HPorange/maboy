use super::desc::RamSize;
use crate::{address::CRamAddr, Savegame};

pub trait CartridgeRam: Savegame {
    fn read(&self, addr: CRamAddr) -> u8;
    fn write(&mut self, addr: CRamAddr, val: u8);
    fn select_bank(&mut self, bank: u8);
}

pub struct NoCRam;

impl Savegame for NoCRam {}

impl CartridgeRam for NoCRam {
    fn read(&self, _addr: CRamAddr) -> u8 {
        0xff
    }

    fn write(&mut self, _addr: CRamAddr, _val: u8) {}

    fn select_bank(&mut self, _bank: u8) {}
}

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

    fn select_bank(&mut self, _bank: u8) {}
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

impl Savegame for CRamMBC2 {}

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

    fn select_bank(&mut self, _bank: u8) {}
}
