use crate::maboy::address::CRamAddr;
pub trait CartridgeRam {
    fn read(&self, addr: CRamAddr) -> u8;
    fn write(&mut self, addr: CRamAddr, val: u8);
}

pub struct NoCRAM;

impl CartridgeRam for NoCRAM {
    fn read(&self, addr: CRamAddr) -> u8 {
        0xff
    }

    fn write(&mut self, addr: CRamAddr, val: u8) {}
}
