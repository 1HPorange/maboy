use super::{banked_rom::BankedRom, CartridgeMBC};
use crate::address::{CRamAddr, CRomAddr};
use crate::cartridge::cram::CRamMBC2;
use crate::{cartridge::CartridgeRam, util::BitOps};

pub struct MBC2 {
    rom: BankedRom,
    cram: CRamMBC2,
    cram_enabled: bool,
}

impl MBC2 {
    pub fn new(rom: Box<[u8]>) -> MBC2 {
        MBC2 {
            rom: BankedRom::new(rom),
            cram: CRamMBC2::new(),
            cram_enabled: false,
        }
    }
}

impl CartridgeMBC for MBC2 {
    type CRAM = CRamMBC2;

    fn read_rom(&self, addr: CRomAddr) -> u8 {
        self.rom.read(addr)
    }

    fn write_rom(&mut self, addr: CRomAddr, val: u8) {
        if let CRomAddr::CROM0(addr) = addr {
            if addr < 0x2000 {
                if !addr.bit(8) {
                    // TODO: Check if this conditions is correct. I just assume it's
                    // the same as for MBC1
                    self.cram_enabled = val & 0xA == 0xA;
                }
            } else {
                if addr.bit(8) {
                    self.rom.select_bank(val & 0xF)
                }
            }
        }
    }

    fn read_cram(&self, addr: CRamAddr) -> u8 {
        if self.cram_enabled {
            self.cram.read(addr)
        } else {
            0xff
        }
    }

    fn write_cram(&mut self, addr: CRamAddr, val: u8) {
        if self.cram_enabled {
            self.cram.write(addr, val)
        }
    }

    fn cram(&self) -> &Self::CRAM {
        &self.cram
    }

    fn cram_mut(&mut self) -> &mut Self::CRAM {
        &mut self.cram
    }
}
