use super::cartridge_mem::{CartridgeMem, CartridgeRam};

pub struct MemoryMap {
    pub crom_0_lo: &'static [u8],
    pub crom_0_hi: &'static [u8],
    pub crom_n: &'static [u8],
}

impl MemoryMap {
    pub fn new<CRAM: CartridgeRam>(cartridge_mem: &CartridgeMem<CRAM>) -> MemoryMap {
        use std::mem::transmute as forget_lifetime;

        // We need unsafe code here because Memory is self-referential (via MemoryMap)
        // This is safe because:
        // - All areas of memory are pinned
        // - MemoryMap will never outlive any area of memory
        unsafe {
            MemoryMap {
                crom_0_lo: &super::BIOS,
                crom_0_hi: forget_lifetime(&cartridge_mem.rom[0x100..0x4000]),
                crom_n: forget_lifetime(&cartridge_mem.rom[0x4000..0x8000]),
            }
        }
    }
}
