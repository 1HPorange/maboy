use super::desc::RamSize;
use crate::address::CRamAddr;

pub trait CartridgeRam {
    fn read(&self, addr: CRamAddr) -> u8;
    fn write(&mut self, addr: CRamAddr, val: u8);
    fn select_bank(&mut self, bank: u8);
    fn data(&self) -> &[u8];
    fn data_mut(&mut self) -> &mut [u8];
}

pub struct NoCRam;

impl CartridgeRam for NoCRam {
    fn read(&self, _addr: CRamAddr) -> u8 {
        0xff
    }

    fn write(&mut self, _addr: CRamAddr, _val: u8) {}

    fn select_bank(&mut self, _bank: u8) {}

    fn data(&self) -> &[u8] {
        &[]
    }

    fn data_mut(&mut self) -> &mut [u8] {
        &mut []
    }
}

pub struct CRamUnbanked(Box<[u8]>);

impl CRamUnbanked {
    pub fn new(ram_size: RamSize) -> CRamUnbanked {
        let ram = match ram_size {
            RamSize::RamNone => panic!("Invalid ram size for CRAMUnbanked"),
            RamSize::Ram2Kb => vec![0; 0x800].into_boxed_slice(),
            RamSize::Ram8Kb => vec![0; 0x2000].into_boxed_slice(),
            RamSize::Ram32Kb => panic!("Invalid ram size for CRAMUnbanked"),
        };

        CRamUnbanked(ram)
    }
}

impl CartridgeRam for CRamUnbanked {
    fn read(&self, addr: CRamAddr) -> u8 {
        *self.0.get(addr.raw() as usize).unwrap_or(&0xff)
    }

    fn write(&mut self, addr: CRamAddr, val: u8) {
        if let Some(mem) = self.0.get_mut(addr.raw() as usize) {
            *mem = val;
        }
    }

    fn select_bank(&mut self, _bank: u8) {}

    fn data(&self) -> &[u8] {
        &self.0
    }

    fn data_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

/// MBC2 has a weird half-byte RAM, where only the lower 4 bits of each addressable byte are used.
/// We store this in a compressed format so we use all 8 bits of each byte. The lower half of the
/// byte contains the lower address.
pub struct CRamMBC2(Box<[u8]>);

impl CRamMBC2 {
    pub fn new() -> Self {
        Self(vec![0u8; 256].into_boxed_slice())
    }
}

impl CartridgeRam for CRamMBC2 {
    fn read(&self, addr: CRamAddr) -> u8 {
        let shift = (addr.raw() & 1) * 4;
        let sub_addr = (addr.raw() >> 1) as usize;

        self.0
            .get(sub_addr)
            .map(|val| (val >> shift) & 0xF)
            .unwrap_or(0xF) // TODO: Check if illegal reads here return 0xF or 0xFF (or something wild)
    }

    fn write(&mut self, addr: CRamAddr, val: u8) {
        let sub_addr = (addr.raw() >> 1) as usize;

        if let Some(mem) = self.0.get_mut(sub_addr) {
            let shift = (addr.raw() & 1) * 4;

            // Clear the old content
            *mem &= 0xF0u8.rotate_left(shift as u32);

            // Write the new value
            *mem |= (val & 0xF) << shift;
        }
    }

    fn select_bank(&mut self, _bank: u8) {}

    fn data(&self) -> &[u8] {
        &self.0
    }

    fn data_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}
