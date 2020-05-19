use std::convert::TryFrom;

pub enum Addr {
    Mem(MemAddr),
    VideoMem(VideoMemAddr),
    Unusable,  // 0xFEA0 - 0xFF7F
    IO(IOReg), // 0xFF00 - 0xFF7F
    IE,        // 0xFFFF
}

pub enum MemAddr {
    CROM(CRomAddr), // 0x0000 - 0x7FFF
    CRAM(CRamAddr), // 0xA000 - 0xBFFF
    WRAM(u16),      // 0xC000 - 0xDFFF
    ECHO(u16),      // 0xE000 - 0xFDFF
    HRAM(u16),      // 0xFF80 - 0xFFFE
}

pub enum CRomAddr {
    CROM0(u16), // 0x0000 - 0x3FFF
    CROMn(u16), // 0x4000 - 0x7FFF
}

pub struct CRamAddr(pub u16);

pub enum VideoMemAddr {
    TileData(u16), // 0x8000 - 0x97FF
    TileMaps(u16), // 0x9800 - 0x9FFF
    OAM(u16),      // 0xFE00 - 0xFE9F
}

// TODO: Think about moving Unusable, IO, and IE into this struct so
// they can share code... is that necessary???
pub enum _HighAddr {}

// 0xFF00 - 0xFF7F
#[derive(Debug)]
pub enum IOReg {
    P1, // 0xFF00
    Serial(SerialReg),
    Timer(TimerReg),
    IF, // 0xFF0F
    Apu(ApuReg),
    Ppu(PpuReg),
    OamDma,             // 0xFF46
    BootRomDisable,     // 0xFF50
    Unimplemented(u16), // TODO: Get rid of this variant
}

impl TryFrom<u16> for IOReg {
    type Error = ();

    fn try_from(addr: u16) -> Result<Self, Self::Error> {
        use IOReg::*;

        Ok(match addr {
            0xFF00 => P1,
            0xFF01 => Serial(SerialReg::SB),
            0xFF02 => Serial(SerialReg::SC),
            0xFF04 => Timer(TimerReg::DIV),
            0xFF05 => Timer(TimerReg::TIMA),
            0xFF06 => Timer(TimerReg::TMA),
            0xFF07 => Timer(TimerReg::TAC),
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
            0xFF46 => OamDma,
            0xFF47 => Ppu(PpuReg::BGP),
            0xFF48 => Ppu(PpuReg::OBP0),
            0xFF49 => Ppu(PpuReg::OBP1),
            0xFF4A => Ppu(PpuReg::WY),
            0xFF4B => Ppu(PpuReg::WX),
            0xFF50 => BootRomDisable,
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

// TODO: Nice Copy derives for all of these
#[derive(Debug, Copy, Clone)]
pub enum PpuReg {
    LCDC, // 0xFF40
    LCDS, // 0xFF41
    SCY,  // 0xFF42
    SCX,  // 0xFF43
    LY,   // 0xFF44
    LYC,  // 0xFF45
    BGP,  // 0xFF47
    OBP0, // 0xFF48
    OBP1, // 0xFF49
    WY,   // 0xFF4A
    WX,   // 0xFF4B
}

#[derive(Debug, Copy, Clone)]
pub enum TimerReg {
    DIV,  // 0xFF04
    TIMA, // 0xFF05
    TMA,  // 0xFF06
    TAC,  // 0xFF07
}

impl From<u16> for Addr {
    fn from(addr: u16) -> Self {
        use Addr::*;
        use MemAddr::*;
        use VideoMemAddr::*;

        match addr & 0xF000 {
            0x0000 => Mem(CROM(CRomAddr::CROM0(addr))),
            0x1000 => Mem(CROM(CRomAddr::CROM0(addr))),
            0x2000 => Mem(CROM(CRomAddr::CROM0(addr))),
            0x3000 => Mem(CROM(CRomAddr::CROM0(addr))),
            0x4000 => Mem(CROM(CRomAddr::CROMn(addr - 0x4000))),
            0x5000 => Mem(CROM(CRomAddr::CROMn(addr - 0x4000))),
            0x6000 => Mem(CROM(CRomAddr::CROMn(addr - 0x4000))),
            0x7000 => Mem(CROM(CRomAddr::CROMn(addr - 0x4000))),
            0x8000 => VideoMem(TileData(addr - 0x8000)),
            0x9000 => {
                if addr < 0x9800 {
                    VideoMem(TileData(addr - 0x8000))
                } else {
                    VideoMem(TileMaps(addr - 0x9800))
                }
            }
            0xA000 => Mem(CRAM(CRamAddr(addr - 0xA000))),
            0xB000 => Mem(CRAM(CRamAddr(addr - 0xA000))),
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
