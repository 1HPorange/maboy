pub mod cartridge_mem;
mod internal_mem;
mod memory_map;

use cartridge_mem::{CartridgeMem, CartridgeRam};
use internal_mem::InternalMem;
use memory_map::MemoryMap;

pub struct Memory<'m, CRAM: CartridgeRam> {
    map: MemoryMap<'m>,
    internal: &'m mut InternalMem,
    cartridge: &'m mut CartridgeMem<CRAM>,
}

impl<'m, CRAM: CartridgeRam> Memory<'m, CRAM> {
    pub fn new(
        internal_mem: &'m mut InternalMem,
        cartridge_mem: &'m mut CartridgeMem<CRAM>,
    ) -> Memory<'m, CRAM> {
        unimplemented!()
    }

    pub fn read8(&self, addr: u16) -> u8 {
        unimplemented!()
    }

    pub fn write8(&mut self, addr: u16, val: u8) {
        unimplemented!()
    }
}
