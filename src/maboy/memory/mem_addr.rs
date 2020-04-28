use std::convert::TryFrom;

// TODO: Mention translation to local addresses
pub enum MemAddr {
    CROM0lo(u16),  // 0x0000 - 0x00FF
    CROM0hi(u16),  // 0x0100 - 0x3FFF
    CROMn(u16),    // 0x4000 - 0x7FFF
    VRAM(u16),     // 0x8000 - 0x9FFF
    CRAM(u16),     // 0xA000 - 0xBFFF
    WRAM(u16),     // 0xC000 - 0xDFFF
    Echo(u16),     // 0xE000 - 0xFDFF
    OAM(u16),      // 0xFE00 - 0xFE9F
    Unusable(u16), // 0xFEA0 - 0xFF7F
    IO(IOAddr),    // 0xFF00 - 0xFF7F
    HRAM(u16),     // 0xFF80 - 0xFFFE
    IE,            // 0xFFFF
}

pub enum RomAddr {}

pub enum RamAddr {}

pub enum PpuRamAddr {}

impl From<u16> for MemAddr {
    fn from(addr: u16) -> MemAddr {
        use MemAddr::*;

        match addr & 0xF000 {
            0x0000 => {
                if addr < 0x100 {
                    CROM0lo(addr)
                } else {
                    CROM0hi(addr)
                }
            }
            0x1000 => CROM0hi(addr),
            0x2000 => CROM0hi(addr),
            0x3000 => CROM0hi(addr),
            0x4000 => CROMn(addr - 0x4000),
            0x5000 => CROMn(addr - 0x4000),
            0x6000 => CROMn(addr - 0x4000),
            0x7000 => CROMn(addr - 0x4000),
            0x8000 => VRAM(addr - 0x8000),
            0x9000 => VRAM(addr - 0x8000),
            0xA000 => CRAM(addr - 0xA000),
            0xB000 => CRAM(addr - 0xA000),
            0xC000 => WRAM(addr - 0xC000),
            0xD000 => WRAM(addr - 0xC000),
            _ => {
                if addr == 0xFFFF {
                    IE
                } else if addr >= 0xFF80 {
                    HRAM(addr - 0xFF80)
                } else if addr >= 0xFF00 {
                    IO(IOAddr::try_from(addr).unwrap())
                } else if addr >= 0xFEA0 {
                    Unusable(addr - 0xFEA0)
                } else if addr >= 0xFE00 {
                    OAM(addr - 0xFE00)
                } else {
                    Echo(addr - 0xE000)
                }
            }
        }
    }
}

// 0xFF00 - 0xFF7F
#[derive(Debug)]
pub enum IOAddr {
    SB,                 // 0xFF01
    SC,                 // 0xFF02
    IF,                 // 0xFF0F
    NR52,               // 0xFF26
    SCY,                // 0xFF42
    LY,                 // 0xFF44
    BOOT_ROM_DISABLE,   // 0xFF50
    Unimplemented(u16), // TODO: Get rid of this variant
}

impl TryFrom<u16> for IOAddr {
    type Error = ();

    fn try_from(addr: u16) -> Result<Self, Self::Error> {
        Ok(match addr {
            0xFF01 => IOAddr::SB,
            0xFF02 => IOAddr::SC,
            0xFF0F => IOAddr::IF,
            0xFF26 => IOAddr::NR52,
            0xFF42 => IOAddr::SCY,
            0xFF44 => IOAddr::LY,
            _ if addr >= 0xFF00 && addr <= 0xFF7F => IOAddr::Unimplemented(addr),
            _ => return Err(()),
        })
    }
}
