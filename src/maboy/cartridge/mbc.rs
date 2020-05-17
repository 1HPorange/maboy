use super::cram::CartridgeRam;
use crate::maboy::address::{CRamAddr, CRomAddr};
use std::pin::Pin;

pub trait CartridgeMBC {
    type CRAM: CartridgeRam;

    fn read_rom(&self, addr: CRomAddr) -> u8;
    fn write_rom(&mut self, addr: CRomAddr, val: u8);

    fn read_cram(&self, addr: CRamAddr) -> u8;
    fn write_cram(&mut self, addr: CRamAddr, val: u8);

    fn cram(&self) -> &Self::CRAM;
    fn cram_mut(&mut self) -> &mut Self::CRAM;
}

// No MBC (although CRAM is supported)

pub struct NoMBC<CRAM> {
    rom: Box<[u8]>,
    cram: CRAM,
}

impl<CRAM: CartridgeRam> NoMBC<CRAM> {
    pub(super) fn new(rom: Box<[u8]>, cram: CRAM) -> NoMBC<CRAM> {
        debug_assert!(rom.len() == 0x8000);
        NoMBC { rom, cram }
    }
}

impl<CRAM: CartridgeRam> CartridgeMBC for NoMBC<CRAM> {
    type CRAM = CRAM;

    fn read_rom(&self, addr: CRomAddr) -> u8 {
        let addr = match addr {
            CRomAddr::CROM0(n) => n,
            CRomAddr::CROMn(n) => n + 0x4000,
        };

        self.rom[addr as usize]
    }

    fn write_rom(&mut self, _addr: CRomAddr, _val: u8) {}

    fn read_cram(&self, addr: CRamAddr) -> u8 {
        self.cram.read(addr)
    }

    fn write_cram(&mut self, addr: CRamAddr, val: u8) {
        self.cram.write(addr, val);
    }

    fn cram(&self) -> &Self::CRAM {
        &self.cram
    }

    fn cram_mut(&mut self) -> &mut Self::CRAM {
        &mut self.cram
    }
}

// MBC 1

pub struct MBC1<CRAM> {
    rom: Pin<Box<[u8]>>,
    mapped_bank: Option<&'static [u8]>,
    cram: CRAM,
    ram_enabled: bool,
    mode: MBC1Mode,
    mapped_bank_index: u8,
}

enum MBC1Mode {
    RomBanking,
    RamBanking,
}

impl<CRAM: CartridgeRam> MBC1<CRAM> {
    pub(super) fn new(rom: Box<[u8]>, cram: CRAM) -> MBC1<CRAM> {
        debug_assert!(rom.len() >= 0x8000 && rom.len() % 0x4000 == 0);

        let rom = Pin::new(rom);

        // Forgets about the lifetime of our slice
        let mapped_bank = Some(unsafe { std::mem::transmute(&rom[0x4000..]) });

        MBC1 {
            rom,
            mapped_bank,
            cram,
            ram_enabled: false,
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

        let bank_idx = self.mapped_bank_index as usize * 0x4000;

        self.mapped_bank = if self.rom.len() >= bank_idx + 0x4000 {
            Some(unsafe { std::mem::transmute(&self.rom[bank_idx..]) })
        } else {
            None
        }
    }
}

impl<CRAM: CartridgeRam> CartridgeMBC for MBC1<CRAM> {
    type CRAM = CRAM;

    fn read_rom(&self, addr: CRomAddr) -> u8 {
        match addr {
            CRomAddr::CROM0(addr) => self.rom[addr as usize],
            CRomAddr::CROMn(addr) => self
                .mapped_bank
                .map(|bank| bank[addr as usize])
                .unwrap_or(0xff),
        }
    }

    fn write_rom(&mut self, addr: CRomAddr, val: u8) {
        match addr {
            CRomAddr::CROM0(n) if n < 0x2000 => self.ram_enabled = val & 0xA == 0xA,
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
                MBC1Mode::RamBanking => self.cram.select_bank(val),
            },
            CRomAddr::CROMn(_) => match val {
                0 => {
                    self.mode = MBC1Mode::RomBanking;
                    self.cram.select_bank(0);
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
        self.cram.read(addr)
    }

    fn write_cram(&mut self, addr: CRamAddr, val: u8) {
        self.cram.write(addr, val);
    }

    fn cram(&self) -> &Self::CRAM {
        &self.cram
    }

    fn cram_mut(&mut self) -> &mut Self::CRAM {
        &mut self.cram
    }
}
