use super::cartridge_mem::{CartridgeMem, CartridgeRam};

pub struct MemoryMap {
    crom_0_lo: &'static [u8],
    crom_0_hi: &'static [u8],
    crom_n: &'static [u8],
}

impl MemoryMap {
    pub fn new<CRAM: CartridgeRam>(cartridge_mem: &CartridgeMem<CRAM>) -> MemoryMap {
        MemoryMap {
            crom_0_lo: &super::BIOS,
            crom_0_hi: &cartridge_mem.rom[0x100..0x4000],
            crom_n: &cartridge_mem.rom[0x4000..0x8000],
        }
    }
}
