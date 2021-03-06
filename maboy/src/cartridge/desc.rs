//! Internal API used to identify the type of cartridge that was loaded by examining the header.

use num_enum::TryFromPrimitive;
use std::convert::TryFrom;

pub struct CartridgeDesc<'a>(&'a [u8]);

impl CartridgeDesc<'_> {
    /// The cartridge header sits at bytes 0x100..=0x14F
    pub fn from_header(header: &[u8]) -> CartridgeDesc {
        assert!(header.len() >= 0x50);
        CartridgeDesc(header)
    }

    pub fn title(&self) -> String {
        // Title is only null-terminated if less than 16 bytes,
        // so we can't rely on that
        self.0[0x34..]
            .iter()
            .copied()
            .take_while(|b| *b != 0)
            .take(16)
            .map(|b| char::from(b))
            .collect::<String>()
    }

    pub fn cartridge_type(&self) -> Option<CartridgeType> {
        CartridgeType::try_from(self.0[0x47]).ok()
    }

    pub fn rom_size(&self) -> Option<RomSize> {
        RomSize::try_from(self.0[0x48]).ok()
    }

    pub fn ram_size(&self) -> Option<RamSize> {
        RamSize::try_from(self.0[0x49]).ok()
    }

    pub fn has_valid_checksum(&self) -> bool {
        let mut checksum = 0u8;
        for i in 0x34..=0x4C {
            checksum = checksum.wrapping_sub(self.0[i]).wrapping_sub(1);
        }

        if self.0[0x4D] == checksum {
            true
        } else {
            log::warn!(
                "Header has incorrect checksum: {} (should be {})",
                self.0[0x4D],
                checksum
            );
            false
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(TryFromPrimitive, Debug, Copy, Clone)]
#[repr(u8)]
pub enum CartridgeType {
    ROM_ONLY = 0x00,
    MBC1 = 0x01,
    MBC1_RAM = 0x02,
    MBC1_RAM_BATTERY = 0x03,
    MBC2 = 0x05,
    MBC2_BATTERY = 0x06,
    ROM_RAM = 0x08,
    ROM_RAM_BATTERY = 0x09,
    MMM01 = 0x0B,
    MMM01_RAM = 0x0C,
    MMM01_RAM_BATTERY = 0x0D,
    MBC3_TIMER_BATTERY = 0x0F,
    MBC3_TIMER_RAM_BATTERY = 0x10,
    MBC3 = 0x11,
    MBC3_RAM = 0x12,
    MBC3_RAM_BATTERY = 0x13,
    MBC4 = 0x15,
    MBC4_RAM = 0x16,
    MBC4_RAM_BATTERY = 0x17,
    MBC5 = 0x19,
    MBC5_RAM = 0x1A,
    MBC5_RAM_BATTERY = 0x1B,
    MBC5_RUMBLE = 0x1C,
    MBC5_RUMBLE_RAM = 0x1D,
    MBC5_RUMBLE_RAM_BATTERY = 0x1E,
    POCKET_CAMERA = 0xFC,
    BANDAI_TAMA5 = 0xFD,
    HuC3 = 0xFE,
    HuC1_RAM_BATTERY = 0xFF,
}

impl CartridgeType {
    pub fn has_battery(&self) -> bool {
        match self {
            CartridgeType::ROM_ONLY => false,
            CartridgeType::MBC1 => false,
            CartridgeType::MBC1_RAM => false,
            CartridgeType::MBC1_RAM_BATTERY => true,
            CartridgeType::MBC2 => false,
            CartridgeType::MBC2_BATTERY => true,
            CartridgeType::ROM_RAM => false,
            CartridgeType::ROM_RAM_BATTERY => true,
            CartridgeType::MMM01 => false,
            CartridgeType::MMM01_RAM => false,
            CartridgeType::MMM01_RAM_BATTERY => true,
            CartridgeType::MBC3_TIMER_BATTERY => true,
            CartridgeType::MBC3_TIMER_RAM_BATTERY => true,
            CartridgeType::MBC3 => false,
            CartridgeType::MBC3_RAM => false,
            CartridgeType::MBC3_RAM_BATTERY => true,
            CartridgeType::MBC4 => false,
            CartridgeType::MBC4_RAM => false,
            CartridgeType::MBC4_RAM_BATTERY => true,
            CartridgeType::MBC5 => false,
            CartridgeType::MBC5_RAM => false,
            CartridgeType::MBC5_RAM_BATTERY => true,
            CartridgeType::MBC5_RUMBLE => false,
            CartridgeType::MBC5_RUMBLE_RAM => false,
            CartridgeType::MBC5_RUMBLE_RAM_BATTERY => true,
            CartridgeType::POCKET_CAMERA => false,
            CartridgeType::BANDAI_TAMA5 => false,
            CartridgeType::HuC3 => false,
            CartridgeType::HuC1_RAM_BATTERY => true,
        }
    }
}

#[derive(TryFromPrimitive, Debug, Copy, Clone)]
#[repr(u8)]
pub enum RomSize {
    RomNoBanking = 0x00,
    Rom4Banks = 0x01,
    Rom8Banks = 0x02,
    Rom16Banks = 0x03,
    Rom32Banks = 0x04,
    Rom64Banks = 0x05,  // only 63 banks used by MBC1
    Rom128Banks = 0x06, // only 125 banks used by MBC1
    Rom256Banks = 0x07,
    Rom72Banks = 0x52,
    Rom80Banks = 0x53,
    Rom96Banks = 0x54,
}

#[derive(TryFromPrimitive, Debug, Copy, Clone)]
#[repr(u8)]
pub enum RamSize {
    RamNone = 0x00,
    Ram2Kb = 0x01,
    Ram8Kb = 0x02,
    Ram32Kb = 0x03, // 4 banks of 8 KBytes each
}
