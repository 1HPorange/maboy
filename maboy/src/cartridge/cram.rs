use super::desc::RamSize;
use crate::address::CRamAddr;

pub trait CartridgeRam {
    fn read(&self, addr: CRamAddr) -> u8;
    fn write(&mut self, addr: CRamAddr, val: u8);
    fn select_bank(&mut self, bank: u8);
    fn data(&self) -> &[u8];
    fn data_mut(&mut self) -> &mut [u8];
}

pub struct NoCRAM;

impl CartridgeRam for NoCRAM {
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

pub struct CRAMUnbanked(Box<[u8]>);

impl CRAMUnbanked {
    pub fn new(ram_size: RamSize) -> CRAMUnbanked {
        let ram = match ram_size {
            RamSize::RamNone => panic!("Invalid ram size for CRAMUnbanked"),
            RamSize::Ram2Kb => vec![0; 0x800].into_boxed_slice(),
            RamSize::Ram8Kb => vec![0; 0x2000].into_boxed_slice(),
            RamSize::Ram32Kb => panic!("Invalid ram size for CRAMUnbanked"),
        };

        CRAMUnbanked(ram)
    }
}

impl CartridgeRam for CRAMUnbanked {
    fn read(&self, addr: CRamAddr) -> u8 {
        *self.0.get(addr.0 as usize).unwrap_or(&0xff)
    }

    fn write(&mut self, addr: CRamAddr, val: u8) {
        if let Some(mem) = self.0.get_mut(addr.0 as usize) {
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
