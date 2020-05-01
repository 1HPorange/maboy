use std::convert::TryFrom;

pub const VRAM_START_ADDR: u16 = 0x8000;

pub enum ReadAddr {
    Mem(MemAddr),
    VideoMem(VideoMemAddr),
    Unusable,  // 0xFEA0 - 0xFF7F
    IO(IOReg), // 0xFF00 - 0xFF7F
    IE,        // 0xFFFF
}

pub enum WriteAddr {
    ROM(u16), // 0x0000 - 0x7FFF
    Mem(ReadWriteMemAddr),
    VideoMem(VideoMemAddr),
    Unusable,  // 0xFEA0 - 0xFF7F
    IO(IOReg), // 0xFF00 - 0xFF7F
    IE,        // 0xFFFF
}

pub enum MemAddr {
    ReadOnly(ReadOnlyMemAddr),
    ReadWrite(ReadWriteMemAddr),
}

pub enum ReadOnlyMemAddr {
    CROM0lo(u16), // 0x0000 - 0x00FF
    CROM0hi(u16), // 0x0100 - 0x3FFF
    CROMn(u16),   // 0x4000 - 0x7FFF
}

pub enum ReadWriteMemAddr {
    CRAM(u16), // 0xA000 - 0xBFFF
    WRAM(u16), // 0xC000 - 0xDFFF
    ECHO(u16), // 0xE000 - 0xFDFF
    HRAM(u16), // 0xFF80 - 0xFFFE
}

pub enum VideoMemAddr {
    VRAM(u16), // 0x8000 - 0x9FFF
    OAM(u16),  // 0xFE00 - 0xFE9F
}

// TODO: Think about moving Unusable, IO, and IE into this struct so
// they can share code... is that necessary???
pub enum HighAddr {}

// 0xFF00 - 0xFF7F
#[derive(Debug)]
pub enum IOReg {
    Serial(SerialReg),
    IF, // 0xFF0F
    Apu(ApuReg),
    Ppu(PpuReg),
    BOOT_ROM_DISABLE,   // 0xFF50
    Unimplemented(u16), // TODO: Get rid of this variant
}

impl TryFrom<u16> for IOReg {
    type Error = ();

    fn try_from(addr: u16) -> Result<Self, Self::Error> {
        use IOReg::*;

        Ok(match addr {
            0xFF01 => Serial(SerialReg::SB),
            0xFF02 => Serial(SerialReg::SC),
            0xFF0F => IF,
            0xFF14 => Apu(ApuReg::NR14),
            0xFF24 => Apu(ApuReg::NR50),
            0xFF25 => Apu(ApuReg::NR51),
            0xFF26 => Apu(ApuReg::NR52),
            0xFF40 => Ppu(PpuReg::LCDC),
            0xFF41 => Ppu(PpuReg::LCDS),
            0xFF42 => Ppu(PpuReg::SCY),
            0xFF43 => Ppu(PpuReg::SCX),
            0xFF44 => Ppu(PpuReg::LY),
            0xFF45 => Ppu(PpuReg::LYC),
            0xFF47 => Ppu(PpuReg::BGP),
            0xFF50 => BOOT_ROM_DISABLE,
            _ if addr >= 0xFF00 && addr <= 0xFF7F => IOReg::Unimplemented(addr),
            _ => return Err(()),
        })
    }
}

#[derive(Debug)]
pub enum SerialReg {
    SB, // 0xFF01
    SC, // 0xFF02
}

#[derive(Debug)]
pub enum ApuReg {
    NR14, // 0xFF14
    NR50, // 0xFF24
    NR51, // 0xFF25
    NR52, // 0xFF26
}

#[derive(Debug)]
pub enum PpuReg {
    LCDC, // 0xFF40
    LCDS, // 0xFF41
    SCY,  // 0xFF42
    SCX,  // 0xFF43
    LY,   // 0xFF44
    LYC,  // 0xFF45
    BGP,  // 0xFF47
}

impl From<u16> for ReadAddr {
    fn from(addr: u16) -> Self {
        use MemAddr::*;
        use ReadAddr::*;
        use ReadOnlyMemAddr::*;
        use ReadWriteMemAddr::*;
        use VideoMemAddr::*;

        match addr & 0xF000 {
            0x0000 => {
                if addr < 0x0100 {
                    Mem(ReadOnly(CROM0lo(addr)))
                } else {
                    Mem(ReadOnly(CROM0hi(addr - 0x0100)))
                }
            }
            0x1000 => Mem(ReadOnly(CROM0hi(addr - 0x0100))),
            0x2000 => Mem(ReadOnly(CROM0hi(addr - 0x0100))),
            0x3000 => Mem(ReadOnly(CROM0hi(addr - 0x0100))),
            0x4000 => Mem(ReadOnly(CROMn(addr - 0x4000))),
            0x5000 => Mem(ReadOnly(CROMn(addr - 0x4000))),
            0x6000 => Mem(ReadOnly(CROMn(addr - 0x4000))),
            0x7000 => Mem(ReadOnly(CROMn(addr - 0x4000))),
            0x8000 => VideoMem(VRAM(addr - VRAM_START_ADDR)),
            0x9000 => VideoMem(VRAM(addr - VRAM_START_ADDR)),
            0xA000 => Mem(ReadWrite(CRAM(addr - 0xA000))),
            0xB000 => Mem(ReadWrite(CRAM(addr - 0xA000))),
            0xC000 => Mem(ReadWrite(WRAM(addr - 0xC000))),
            0xD000 => Mem(ReadWrite(WRAM(addr - 0xC000))),
            0xE000 => Mem(ReadWrite(ECHO(addr - 0xE000))),
            0xF000 => {
                if addr == 0xFFFF {
                    IE
                } else if addr >= 0xFF80 {
                    Mem(ReadWrite(HRAM(addr - 0xFF80)))
                } else if addr >= 0xFF00 {
                    IO(IOReg::try_from(addr).unwrap())
                } else if addr >= 0xFEA0 {
                    Unusable
                } else if addr >= 0xFE00 {
                    VideoMem(OAM(addr - 0xFE00))
                } else {
                    Mem(ReadWrite(ECHO(addr - 0xE000)))
                }
            }
            _ => unsafe { std::hint::unreachable_unchecked() },
        }
    }
}

impl From<u16> for WriteAddr {
    fn from(addr: u16) -> Self {
        use ReadWriteMemAddr::*;
        use VideoMemAddr::*;
        use WriteAddr::*;

        match addr & 0xF000 {
            0x0000 => ROM(addr),
            0x1000 => ROM(addr),
            0x2000 => ROM(addr),
            0x3000 => ROM(addr),
            0x4000 => ROM(addr),
            0x5000 => ROM(addr),
            0x6000 => ROM(addr),
            0x7000 => ROM(addr),
            0x8000 => VideoMem(VRAM(addr - VRAM_START_ADDR)),
            0x9000 => VideoMem(VRAM(addr - VRAM_START_ADDR)),
            0xA000 => Mem(CRAM(addr - 0xA000)),
            0xB000 => Mem(CRAM(addr - 0xA000)),
            0xC000 => Mem(WRAM(addr - 0xC000)),
            0xD000 => Mem(WRAM(addr - 0xC000)),
            0xE000 => Mem(ECHO(addr - 0xE000)),
            0xF000 => {
                if addr == 0xFFFF {
                    IE
                } else if addr >= 0xFF80 {
                    Mem(HRAM(addr - 0xFF80))
                } else if addr >= 0xFF00 {
                    IO(IOReg::try_from(addr).unwrap())
                } else if addr >= 0xFEA0 {
                    Unusable
                } else if addr >= 0xFE00 {
                    VideoMem(OAM(addr - 0xFE00))
                } else {
                    Mem(ECHO(addr - 0xE000))
                }
            }
            _ => unsafe { std::hint::unreachable_unchecked() },
        }
    }
}
