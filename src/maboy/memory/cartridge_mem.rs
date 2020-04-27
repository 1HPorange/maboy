use crate::maboy::cartridge::Cartridge;
pub trait CartridgeRam {
    fn read8(&self, addr: u16) -> u8;
    fn write8(&self, addr: u16, val: u8);
}

pub struct CartridgeMem<CRAM: CartridgeRam> {
    pub(super) rom: Box<[u8]>,
    pub(super) ram: CRAM,
}

impl From<Cartridge> for CartridgeMem<WithoutCartridgeRam> {
    fn from(cartridge: Cartridge) -> Self {
        // TODO: Do this properly
        CartridgeMem {
            rom: cartridge.bytes,
            ram: WithoutCartridgeRam,
        }
    }
}

pub struct WithoutCartridgeRam;

impl CartridgeRam for WithoutCartridgeRam {
    fn read8(&self, addr: u16) -> u8 {
        0xff
    }

    fn write8(&self, addr: u16, val: u8) {}
}
