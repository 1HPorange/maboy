use num_enum::TryFromPrimitive;
use std::convert::TryFrom;
pub trait CartridgeDesc {
    fn header(&self) -> &[u8];

    fn title(&self) -> String {
        // Title is only null-terminated if less than 16 bytes,
        // so we can't rely on that
        let title_bytes = self.header()[0x34..]
            .iter()
            .copied()
            .take_while(|b| *b != 0)
            .take(16)
            .collect::<Vec<_>>();

        // ASCII can be interpreted as UTF8 (at least to a reasonable degree)
        String::from_utf8_lossy(&title_bytes).into_owned()
    }

    fn cartridge_type(&self) -> Option<CartridgeType> {
        CartridgeType::try_from(self.header()[0x47]).ok()
    }

    fn rom_size(&self) -> Option<RomSize> {
        RomSize::try_from(self.header()[0x48]).ok()
    }

    fn ram_size(&self) -> Option<RamSize> {
        RamSize::try_from(self.header()[0x49]).ok()
    }

    fn has_valid_header(&self) -> bool {
        let header = self.header();

        let mut checksum = 0u8;
        for i in 0x34..=0x4C {
            checksum = checksum.wrapping_sub(header[i]).wrapping_sub(1);
        }

        header[0x4D] == checksum
    }
}

#[allow(non_camel_case_types)]
#[derive(TryFromPrimitive)]
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

#[derive(TryFromPrimitive)]
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

#[derive(TryFromPrimitive)]
#[repr(u8)]
pub enum RamSize {
    RamNone = 0x00,
    Ram2Kb = 0x01,
    Ram8Kb = 0x02,
    Ram32Kb = 0x03, // 4 banks of 8 KBytes each
}
