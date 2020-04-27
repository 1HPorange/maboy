use super::cpu::Interrupt;
use arrayvec::ArrayVec;
use std::ops::IndexMut;
use std::ptr::{self, NonNull};

pub const CROM_BANK_LEN: usize = 0x8000 - 0x4000;
pub const VRAM_BANK_LEN: usize = 0xA000 - 0x8000;
pub const CRAM_BANK_LEN: usize = 0xC000 - 0xA000;
pub const WRAM_BANK_LEN: usize = 0xE000 - 0xD000;

pub const UPPER_RAM_LEN: usize = 0x10000 - 0xFE00;

// According to http://bgb.bircd.org/pandocs.htm#memorymap
/// Contains pointers to all *switchable* areas of memory.
// pub struct MappedMem {
//     crom_bank_0_low: NonNull<[u8; 0x100]>,
//     crom_bank_n: NonNull<[u8; CROM_BANK_LEN]>,
//     vram: NonNull<[u8; VRAM_BANK_LEN]>,
//     cram: *mut [u8; CRAM_BANK_LEN],
//     wram_bank_n: NonNull<[u8; WRAM_BANK_LEN]>,
// }
pub struct MemMap {
    crom_bank_0_low: NonNull<[u8]>,  // 0x0000 - 0x3FFF
    crom_bank_0_high: NonNull<[u8]>, // cont.
    crom_bank_n: NonNull<[u8]>,      // 0x4000 - 0x7FFF
    vram: NonNull<[u8]>,             // 0x8000 - 0x9FFF
    cram: *mut [u8],                 // 0xA000 - 0xBFFF
    wram_bank_0: NonNull<[u8]>,      // 0xC000 - 0xCFFF
    wram_bank_n: NonNull<[u8]>,      // 0xD000 - 0xDFFF
    // Echo RAM - Don't touch!       // 0xE000 - 0xFDFF
    upper_ram: NonNull<[u8]>, //        0xFE00 - 0xFFFF
}

pub struct MMU<'a> {
    map: MemMap,
    cartridge: &'a mut CartridgeMem,
    builtin: &'a mut BuiltinMem,
}

// TODO: Hande reads/writes of partially readable/writeable adresses correctly. Mostly in 0xFFxx region
impl<'a> MMU<'a> {
    pub fn TEMP_NEW(
        builtin_mem: &'a mut BuiltinMem,
        cartridge_mem: &'a mut CartridgeMem,
    ) -> MMU<'a> {
        // TODO: Assert that invariants for this construction always hold
        let map = MemMap {
            crom_bank_0_low: NonNull::from(&BIOS),
            crom_bank_0_high: NonNull::from(&cartridge_mem.crom_banks[0][..]),
            crom_bank_n: NonNull::from(&cartridge_mem.crom_banks[1][..]),
            vram: NonNull::from(&mut builtin_mem.vram_banks[0][..]),
            cram: cartridge_mem
                .cram_banks
                .get_mut(0)
                .map(|bank| &mut bank[..] as *mut [u8])
                .unwrap_or(ptr::slice_from_raw_parts_mut(ptr::null_mut(), 0)),
            wram_bank_0: NonNull::from(&mut builtin_mem.wram_banks[0][..]),
            wram_bank_n: NonNull::from(&mut builtin_mem.wram_banks[1][..]),
            upper_ram: NonNull::from(&mut builtin_mem.upper_ram[..]),
        };

        MMU {
            map,
            cartridge: cartridge_mem,
            builtin: builtin_mem,
        }
    }

    /// Will never fail, but yield garbage on failed reads
    pub fn read8(&self, addr: u16) -> u8 {
        match addr {
            0xFF00 => input.asd,
            _ => *self.ref8(addr).unwrap_or(&0xFF), // TODO: Research correct "garbage" returns
        }
    }

    /// Will never fail, but yield garbage on failed reads
    pub fn read16(&self, addr: u16) -> u16 {
        // TODO: Prevent out of bounds reads
        unsafe {
            if let Some(const_ref) = self.ref8(addr) {
                *std::mem::transmute::<&u8, &u16>(const_ref)
            } else {
                0xFFFF // TODO: Research correct "garbage" returns
            }
        }
    }

    /// Will never fail, but throw away the bits that are not writeable
    pub fn write8(&mut self, addr: u16, value: u8) {
        // TODO: Nicer
        match addr {
            0xFF00 => input.asd,
            0xFF02 => {
                print!("{}", self.read8(0xFF01) as char);
            }
            0xFF50 if value == 1 => {
                self.unmap_boot_rom();
            }
            _ => {
                if let Some(mut_ref) = self.mut8(addr) {
                    *mut_ref = value;
                }
            }
        }
    }

    /// Will never fail, but throw away the bits that are not writeable
    pub fn write16(&mut self, addr: u16, value: u16) {
        // TODO: Prevent out of bounds writes
        // TODO: See what happens on real hardware if you write 16 bit values to SPECIAL 8 bit registers
        unsafe {
            if let Some(mut_ref) = self.mut8(addr) {
                *std::mem::transmute::<&mut u8, &mut u16>(mut_ref) = value;
            }
        }
    }

    pub fn ref8(&self, addr: u16) -> Option<&u8> {
        unsafe {
            let addr = addr as usize;

            match addr & 0xF000 {
                0x0000 => {
                    if addr < 0x100 {
                        Some(&self.map.crom_bank_0_low.as_ref()[addr])
                    } else {
                        Some(&self.map.crom_bank_0_high.as_ref()[addr])
                    }
                }
                0x1000 => Some(&self.map.crom_bank_0_high.as_ref()[addr]),
                0x2000 => Some(&self.map.crom_bank_0_high.as_ref()[addr]),
                0x3000 => Some(&self.map.crom_bank_0_high.as_ref()[addr]),
                0x4000 => Some(&self.map.crom_bank_n.as_ref()[addr - 0x4000]),
                0x5000 => Some(&self.map.crom_bank_n.as_ref()[addr - 0x4000]),
                0x6000 => Some(&self.map.crom_bank_n.as_ref()[addr - 0x4000]),
                0x7000 => Some(&self.map.crom_bank_n.as_ref()[addr - 0x4000]),
                0x8000 => Some(&self.map.vram.as_ref()[addr - 0x8000]),
                0x9000 => Some(&self.map.vram.as_ref()[addr - 0x8000]),
                0xA000 => self.map.cram.as_ref().map(|cram| &cram[addr - 0xA000]),
                0xB000 => self.map.cram.as_ref().map(|cram| &cram[addr - 0xA000]),
                0xC000 => Some(&self.map.wram_bank_0.as_ref()[addr - 0xC000]),
                0xD000 => Some(&self.map.wram_bank_n.as_ref()[addr - 0xD000]),
                0xE000 => None, // This is actually a read of echo RAM, so we should let it succeed!
                0xF000 => {
                    if addr < 0xFE00 {
                        None // This is actually a read of echo RAM, so we should let it succeed!
                    } else {
                        Some(&self.map.upper_ram.as_ref()[addr - 0xFE00])
                    }
                }
                _ => std::hint::unreachable_unchecked(),
            }
        }
    }

    // TODO: Delegate reads and writes of special registers to other modules
    pub fn mut8(&mut self, addr: u16) -> Option<&mut u8> {
        unsafe {
            let addr = addr as usize;

            if addr < 0x8000 {
                return None; // Write to cartridge ROM
            }

            match addr & 0xF000 {
                0x8000 => Some(&mut self.map.vram.as_mut()[addr - 0x8000]),
                0x9000 => Some(&mut self.map.vram.as_mut()[addr - 0x8000]),
                0xA000 => self.map.cram.as_mut().map(|cram| &mut cram[addr - 0xA000]),
                0xB000 => self.map.cram.as_mut().map(|cram| &mut cram[addr - 0xA000]),
                0xC000 => Some(&mut self.map.wram_bank_0.as_mut()[addr - 0xC000]),
                0xD000 => Some(&mut self.map.wram_bank_n.as_mut()[addr - 0xD000]),
                0xE000 => None, // This is actually a write to echo RAM, so we should let it succeed!
                0xF000 => {
                    if addr < 0xFE00 {
                        None // This is actually a write to echo RAM, so we should let it succeed!
                    } else {
                        Some(&mut self.map.upper_ram.as_mut()[addr - 0xFE00])
                    }
                }
                _ => std::hint::unreachable_unchecked(),
            }
        }
    }

    // TODO: Think about if this belongs here
    pub fn request_interrupt(&mut self, ir: Interrupt) {
        *self.mut8(0xFF0F).unwrap() |= 1 << ir as u8;
    }

    pub fn unmap_boot_rom(&mut self) {
        // TODO: Investigate if this is actually safe, or if we have to use pointers
        self.map.crom_bank_0_low =
            unsafe { std::mem::transmute(&self.cartridge.crom_banks[0][..256]) }
    }

    // TODO: DMA timing and access restrictions!!!
    pub fn oam_dma(&mut self, src_addr_high: u8) {
        unsafe {
            const OAM_LEN: u16 = 160;

            // TODO: PERF: Make this operation much faster by doing slice copy!
            // self.builtin.upper_ram[..OAM_LEN].copy_from_slice(src);

            let src_start = (src_addr_high as u16) * 100;
            for i in 0..OAM_LEN {
                *self.mut8(0xFE00 + i).unwrap() = self.read8(src_start + i);
            }
        }
    }
}

// TODO: Consider moving these structs

pub struct CartridgeMem {
    pub crom_banks: Vec<Box<[u8]>>,
    pub cram_banks: Vec<Box<[u8]>>,
}

impl CartridgeMem {
    pub fn empty() -> CartridgeMem {
        CartridgeMem {
            crom_banks: vec![vec![0; CROM_BANK_LEN].into_boxed_slice(); 2],
            cram_banks: vec![],
        }
    }

    // TODO: Docs!
    pub fn test_rom(rom: &[u8]) -> CartridgeMem {
        CartridgeMem {
            crom_banks: vec![
                rom.iter()
                    .copied()
                    .chain(std::iter::repeat(0))
                    .take(CROM_BANK_LEN)
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
                vec![0; CROM_BANK_LEN].into_boxed_slice(),
            ],
            cram_banks: vec![],
        }
    }
}

pub struct BuiltinMem {
    vram_banks: Vec<Box<[u8]>>,
    wram_banks: Vec<Box<[u8]>>,
    upper_ram: Box<[u8]>,
}

impl BuiltinMem {
    pub fn new() -> BuiltinMem {
        let mut upper_ram = vec![0; UPPER_RAM_LEN].into_boxed_slice();

        BuiltinMem {
            vram_banks: vec![vec![0; VRAM_BANK_LEN].into_boxed_slice()],
            wram_banks: vec![vec![0; WRAM_BANK_LEN].into_boxed_slice(); 2],
            upper_ram,
        }
    }
}

const BIOS: [u8; 256] = [
    0x31, 0xFE, 0xFF, 0xAF, 0x21, 0xFF, 0x9F, 0x32, 0xCB, 0x7C, 0x20, 0xFB, 0x21, 0x26, 0xFF, 0x0E,
    0x11, 0x3E, 0x80, 0x32, 0xE2, 0x0C, 0x3E, 0xF3, 0xE2, 0x32, 0x3E, 0x77, 0x77, 0x3E, 0xFC, 0xE0,
    0x47, 0x11, 0x04, 0x01, 0x21, 0x10, 0x80, 0x1A, 0xCD, 0x95, 0x00, 0xCD, 0x96, 0x00, 0x13, 0x7B,
    0xFE, 0x34, 0x20, 0xF3, 0x11, 0xD8, 0x00, 0x06, 0x08, 0x1A, 0x13, 0x22, 0x23, 0x05, 0x20, 0xF9,
    0x3E, 0x19, 0xEA, 0x10, 0x99, 0x21, 0x2F, 0x99, 0x0E, 0x0C, 0x3D, 0x28, 0x08, 0x32, 0x0D, 0x20,
    0xF9, 0x2E, 0x0F, 0x18, 0xF3, 0x67, 0x3E, 0x64, 0x57, 0xE0, 0x42, 0x3E, 0x91, 0xE0, 0x40, 0x04,
    0x1E, 0x02, 0x0E, 0x0C, 0xF0, 0x44, 0xFE, 0x90, 0x20, 0xFA, 0x0D, 0x20, 0xF7, 0x1D, 0x20, 0xF2,
    0x0E, 0x13, 0x24, 0x7C, 0x1E, 0x83, 0xFE, 0x62, 0x28, 0x06, 0x1E, 0xC1, 0xFE, 0x64, 0x20, 0x06,
    0x7B, 0xE2, 0x0C, 0x3E, 0x87, 0xF2, 0xF0, 0x42, 0x90, 0xE0, 0x42, 0x15, 0x20, 0xD2, 0x05, 0x20,
    0x4F, 0x16, 0x20, 0x18, 0xCB, 0x4F, 0x06, 0x04, 0xC5, 0xCB, 0x11, 0x17, 0xC1, 0xCB, 0x11, 0x17,
    0x05, 0x20, 0xF5, 0x22, 0x23, 0x22, 0x23, 0xC9, 0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B,
    0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00, 0x0D, 0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E,
    0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD, 0xD9, 0x99, 0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC,
    0xDD, 0xDC, 0x99, 0x9F, 0xBB, 0xB9, 0x33, 0x3E, 0x3c, 0x42, 0xB9, 0xA5, 0xB9, 0xA5, 0x42, 0x4C,
    0x21, 0x04, 0x01, 0x11, 0xA8, 0x00, 0x1A, 0x13, 0xBE, 0x20, 0xFE, 0x23, 0x7D, 0xFE, 0x34, 0x20,
    0xF5, 0x06, 0x19, 0x78, 0x86, 0x23, 0x05, 0x20, 0xFB, 0x86, 0x20, 0xFE, 0x3E, 0x01, 0xE0, 0x50,
];