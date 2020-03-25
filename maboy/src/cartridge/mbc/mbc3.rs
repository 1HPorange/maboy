use super::{banked_rom::BankedRom, rtc::Rtc, CartridgeMBC};
use crate::address::{CRamAddr, CRomAddr};
use crate::{cartridge::cram::CartridgeRam, Metadata, Savegame};

/// For speedyness reasons, we split MBC3 into a variant with an RTC module,
/// and one without it.

pub struct MBC3<CRAM> {
    rom: BankedRom,
    cram: CRAM,
    cram_enabled: bool,
}

impl<CRAM: CartridgeRam> MBC3<CRAM> {
    pub fn new(rom: Box<[u8]>, cram: CRAM) -> Self {
        Self {
            rom: BankedRom::new(rom),
            cram,
            cram_enabled: false,
        }
    }
}

impl<CRAM: CartridgeRam> Savegame for MBC3<CRAM> {
    fn savegame(&self) -> Option<&[u8]> {
        self.cram.savegame()
    }

    fn savegame_mut(&mut self) -> Option<&mut [u8]> {
        self.cram.savegame_mut()
    }
}

impl<CRAM> Metadata for MBC3<CRAM> {}

impl<CRAM: CartridgeRam> CartridgeMBC for MBC3<CRAM> {
    type CRAM = CRAM;

    fn read_rom(&self, addr: CRomAddr) -> u8 {
        self.rom.read(addr)
    }

    fn write_rom(&mut self, addr: CRomAddr, val: u8) {
        match addr {
            CRomAddr::CROM0(addr) if addr < 0x2000 => self.cram_enabled = val & 0xA == 0xA,
            CRomAddr::CROM0(_) => {
                if val != 0 {
                    self.rom.select_bank(val & 0b_0111_1111)
                } else {
                    self.rom.select_bank(1)
                }
            }
            CRomAddr::CROMn(addr) => {
                if addr < 0x6000 {
                    self.cram.try_select_bank(val)
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
            self.cram.write(addr, val);
        }
    }
}

pub struct MBC3Rtc<CRAM> {
    rom: BankedRom,
    cram_rtc_enabled: bool,
    mapping: Mapping,
    cram: CRAM,
    rtc: Rtc,
    latch_reg_last_write: u8,
}

enum Mapping {
    CRam,
    Rtc,
}

impl<CRAM: CartridgeRam> MBC3Rtc<CRAM> {
    pub fn new(rom: Box<[u8]>, cram: CRAM) -> Self {
        Self {
            rom: BankedRom::new(rom),
            cram_rtc_enabled: false,
            mapping: Mapping::CRam, // TODO: Check
            cram,
            rtc: Rtc::new(),
            latch_reg_last_write: 1, // TODO: Check if adequate or if we need an option here
        }
    }
}

impl<CRAM: CartridgeRam> Savegame for MBC3Rtc<CRAM> {
    fn savegame(&self) -> Option<&[u8]> {
        self.cram.savegame()
    }

    fn savegame_mut(&mut self) -> Option<&mut [u8]> {
        self.cram.savegame_mut()
    }
}

impl<CRAM> Metadata for MBC3Rtc<CRAM> {
    fn supports_metadata(&self) -> bool {
        true
    }

    fn serialize_metadata(&self) -> Result<Vec<u8>, crate::CartridgeParseError> {
        Ok(self.rtc.export_metadata())
    }

    fn deserialize_metadata(&mut self, data: Vec<u8>) -> Result<(), crate::CartridgeParseError> {
        self.rtc.apply_metadata(data)
    }
}

impl<CRAM: CartridgeRam> CartridgeMBC for MBC3Rtc<CRAM> {
    type CRAM = CRAM;

    fn read_rom(&self, addr: CRomAddr) -> u8 {
        self.rom.read(addr)
    }

    fn write_rom(&mut self, addr: CRomAddr, val: u8) {
        match addr {
            CRomAddr::CROM0(addr) if addr < 0x2000 => self.cram_rtc_enabled = val & 0xA == 0xA,
            CRomAddr::CROM0(_) => {
                if val != 0 {
                    self.rom.select_bank(val & 0b_0111_1111);
                } else {
                    self.rom.select_bank(1);
                }
            }
            CRomAddr::CROMn(addr) if addr < 0x6000 => {
                if val < 4 {
                    self.cram.try_select_bank(val);

                    // TODO: Check if mapping changes to cram even when a non-existing CRAM bank
                    // is selected
                    self.mapping = Mapping::CRam;
                } else if self.rtc.try_select_reg(val) {
                    self.mapping = Mapping::Rtc;
                }
            }
            CRomAddr::CROMn(_) => {
                if self.latch_reg_last_write == 0 && val == 1 {
                    self.rtc.toggle_latched()
                }

                self.latch_reg_last_write = val;
            }
        }
    }

    fn read_cram(&self, addr: CRamAddr) -> u8 {
        if self.cram_rtc_enabled {
            match self.mapping {
                Mapping::CRam => self.cram.read(addr),
                Mapping::Rtc => self.rtc.read_reg(),
            }
        } else {
            0xff
        }
    }

    fn write_cram(&mut self, addr: CRamAddr, val: u8) {
        if self.cram_rtc_enabled {
            match self.mapping {
                Mapping::CRam => self.cram.write(addr, val),
                Mapping::Rtc => self.rtc.write_reg(val),
            }
        }
    }
}
