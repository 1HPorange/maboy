use super::memory::{cartridge_mem::CartridgeRam, Memory};
pub struct Board<'m, CRAM: CartridgeRam> {
    mem: Memory<'m, CRAM>,
}

impl<'m, CRAM: CartridgeRam> Board<'m, CRAM> {
    pub fn new() -> Board<'m, CRAM> {
        unimplemented!()
    }

    pub fn advance_mcycle(&mut self) {}

    pub fn read8(&mut self, addr: u16) -> u8 {
        let result = self.mem.read8(addr);
        self.advance_mcycle();
        result
    }

    pub fn write8(&mut self, addr: u16, val: u8) {
        unimplemented!();
        self.advance_mcycle();
    }

    pub fn read16(&mut self, addr: u16) -> u16 {
        u16::from_le_bytes([self.read8(addr), self.read8(addr.wrapping_add(1))])
    }

    pub fn write16(&mut self, addr: u16, val: u16) {
        self.write8(addr, (val & 0xff) as u8);
        self.write8(addr.wrapping_add(1), (val >> 8) as u8);
    }
}
