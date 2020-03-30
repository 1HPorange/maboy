use arrayvec::ArrayVec;
use std::ops::IndexMut;
use std::ptr::NonNull;

// According to http://bgb.bircd.org/pandocs.htm#memorymap
pub struct MappedMemory {
    crom_bank_0_low: NonNull<[u8; 0x100]>,
    // crom_bank_0_high: NonNull<[u8; 0x4000 - 0x100]>,
    crom_bank_n: NonNull<[u8; 0x8000 - 0x4000]>,
    vram: NonNull<[u8; 0xA000 - 0x8000]>,
    cram: *mut [u8; 0xC000 - 0xA000],
    // wram_bank_0: NonNull<[u8; 0xD000 - 0xC000]>,
    wram_bank_n: NonNull<[u8; 0xE000 - 0xD000]>,
    // echo_ram: [u8; 0xFE00 - 0xE000>]
    // oam: NonNull<[u8; 0xFEA0 - 0xFE00]>,
    // not_usable: [u8; 0xFF00 - 0xFEA0]
    // io: [u8; 0xFF80 - 0xFF00]
    // hram: NonNull<[u8; 0xFFFF - 0xFF80]>,
    // ie: bool,
}

pub struct AvailableMemory<'cr> {
    bios: &'static [u8; 256],
    crom_banks: Vec<&'cr [u8; 0x8000 - 0x4000]>,
    vram_bank: [&'cr mut [u8; 0xA000 - 0x8000]; 2],
    cram: Vec<&'cr mut [u8; 0xC000 - 0xA000]>,
    wram_banks: ArrayVec<[&'cr mut [u8; 0xE000 - 0xD000]; 7]>,
    high: [u8; 0x10000 - 0xFE00],
}

pub struct MMU<'cr> {
    mapped: MappedMemory,
    available: AvailableMemory<'cr>,
}

impl MMU<'_> {
    pub fn read8(&self, addr: u16) -> u8 {
        *self.ref8(addr)
    }

    pub fn read16(&self, addr: u16) -> u16 {
        if addr & 1 == 0 {
            unsafe { *std::mem::transmute::<&u8, &u16>(self.ref8(addr)) }
        } else {
            // TODO: This is a hack to prevent 16 bit read from
            // going over the boundary between memory regions
            // (which all have a size that's divisible by 2).
            // I need to figure out what the real Game Boy does
            // in this case...
            panic!("Unaligned read of 16 bit value");
        }
    }

    pub fn write8(&mut self, addr: u16, value: u8) {
        let addr = addr as usize;

        if addr < 0x8000 {
            panic!("Attempted to write to ROM");
        }

        unsafe {
            *match addr & 0xF000 {
                0x8000 => &mut self.mapped.vram.as_mut()[addr],
                0x9000 => &mut self.mapped.vram.as_mut()[addr],
                0xA000 => &mut self
                    .mapped
                    .cram
                    .as_mut()
                    .expect("Tried to write to cartridge RAM, but it doesn't exist")[addr],
                0xB000 => &mut self
                    .mapped
                    .cram
                    .as_mut()
                    .expect("Tried to write to cartridge RAM, but it doesn't exist")[addr],
                0xC000 => &mut self.available.wram_banks[0][addr],
                0xD000 => &mut self.mapped.wram_bank_n.as_mut()[addr],
                0xE000 => panic!("Tried to write into echo RAM"),
                0xF000 => {
                    if addr < 0xFE00 {
                        panic!("Tried to write into echo RAM");
                    } else {
                        &mut self.available.high[addr]
                    }
                }
                _ => std::hint::unreachable_unchecked(),
            } = value;
        }
    }

    fn ref8(&self, addr: u16) -> &u8 {
        let addr = addr as usize;

        unsafe {
            match addr & 0xF000 {
                0x0000 => {
                    if addr < 0x100 {
                        &self.mapped.crom_bank_0_low.as_ref()[addr]
                    } else {
                        &self.available.crom_banks[0][addr]
                    }
                }
                0x1000 => &self.available.crom_banks[0][addr],
                0x2000 => &self.available.crom_banks[0][addr],
                0x3000 => &self.available.crom_banks[0][addr],
                0x4000 => &self.mapped.crom_bank_n.as_ref()[addr],
                0x5000 => &self.mapped.crom_bank_n.as_ref()[addr],
                0x6000 => &self.mapped.crom_bank_n.as_ref()[addr],
                0x7000 => &self.mapped.crom_bank_n.as_ref()[addr],
                0x8000 => &self.mapped.vram.as_ref()[addr],
                0x9000 => &self.mapped.vram.as_ref()[addr],
                0xA000 => &self
                    .mapped
                    .cram
                    .as_ref()
                    .expect("Tried to access cartridge RAM, but it doesn't exist")[addr],
                0xB000 => &self
                    .mapped
                    .cram
                    .as_ref()
                    .expect("Tried to access cartridge RAM, but it doesn't exist")[addr],
                0xC000 => &self.available.wram_banks[0][addr],
                0xD000 => &self.mapped.wram_bank_n.as_ref()[addr],
                0xE000 => panic!("Accessed echo ram - DON'T"),
                0xF000 => {
                    if addr < 0xFE00 {
                        panic!("Accessed echo ram - DON'T");
                    } else {
                        &self.available.high[addr]
                    }
                }
                _ => std::hint::unreachable_unchecked(),
            }
        }
    }
}
