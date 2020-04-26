pub struct CartridgeMem<CRAM: CartridgeRam> {
    ram: CRAM,
}
pub trait CartridgeRam {
    fn read8(&self, addr: u16) -> u8;
    fn write8(&self, addr: u16, val: u8);
}

pub struct WithoutCartridgeRam;

impl CartridgeRam for WithoutCartridgeRam {
    fn read8(&self, addr: u16) -> u8 {
        0xff
    }

    fn write8(&self, addr: u16, val: u8) {}
}
