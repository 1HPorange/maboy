use super::{banked_rom::BankedRom, CartridgeMBC};
use crate::{
    address::{CRamAddr, CRomAddr},
    cartridge::cram::CartridgeRam,
    Metadata, Savegame,
};

pub struct MBC1<CRAM> {
    rom: BankedRom,
    cram: CRAM,
    cram_enabled: bool,
    mode: MBC1Mode,
    mapped_bank_index: u8,
}

enum MBC1Mode {
    RomBanking,
    RamBanking,
}

impl<CRAM: CartridgeRam> MBC1<CRAM> {
    pub fn new(rom: Box<[u8]>, cram: CRAM) -> MBC1<CRAM> {
        MBC1 {
            rom: BankedRom::new(rom),
            cram,
            cram_enabled: false,
            mode: MBC1Mode::RomBanking,
            mapped_bank_index: 1,
        }
    }

    fn update_mapped_bank(&mut self) {
        self.mapped_bank_index = match self.mapped_bank_index {
            0x00 => 0x01,
            0x20 => 0x21,
            0x40 => 0x41,
            0x60 => 0x61,
            n => n,
        };

        self.rom.select_bank(self.mapped_bank_index);
    }
}

impl<CRAM: CartridgeRam> Savegame for MBC1<CRAM> {
    fn savegame(&self) -> Option<&[u8]> {
        self.cram.savegame()
    }

    fn savegame_mut(&mut self) -> Option<&mut [u8]> {
        self.cram.savegame_mut()
    }
}

impl<CRAM> Metadata for MBC1<CRAM> {}

impl<CRAM: CartridgeRam> CartridgeMBC for MBC1<CRAM> {
    type CRAM = CRAM;

    fn read_rom(&self, addr: CRomAddr) -> u8 {
        self.rom.read(addr)
    }

    fn write_rom(&mut self, addr: CRomAddr, val: u8) {
        match addr {
            CRomAddr::CROM0(n) if n < 0x2000 => self.cram_enabled = val & 0xA == 0xA,
            CRomAddr::CROM0(_) => {
                if matches!(self.mode, MBC1Mode::RomBanking) {
                    self.mapped_bank_index = (self.mapped_bank_index & (!0x1F)) + (val & 0x1F);
                    self.update_mapped_bank();
                }
            }
            CRomAddr::CROMn(n) if n < 0x2000 => match self.mode {
                MBC1Mode::RomBanking => {
                    self.mapped_bank_index = self.mapped_bank_index & 0x1F + ((val & 0b11) << 5);
                    self.update_mapped_bank();
                }
                MBC1Mode::RamBanking => self.cram.try_select_bank(val),
            },
            CRomAddr::CROMn(_) => match val {
                0 => {
                    self.mode = MBC1Mode::RomBanking;
                    self.cram.try_select_bank(0);
                }
                1 => {
                    self.mode = MBC1Mode::RamBanking;
                    self.mapped_bank_index &= 0x1F;
                    self.update_mapped_bank();
                }
                n => log::warn!("Invalid value {:#04X} written to MBC1 mode select", n),
            },
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
}
