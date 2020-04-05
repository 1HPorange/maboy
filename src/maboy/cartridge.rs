use super::mmu::CartridgeMem;
use num_enum::TryFromPrimitive;
use std::convert::TryFrom;
use std::path::Path;

const TITLE_START: usize = 0x134;
const CARTRIDGE_TYPE: usize = 0x147;

pub struct Cartridge {
    mem: CartridgeMem,
}

impl Cartridge {
    pub fn from_rom<P: AsRef<Path>>(path: P) -> Result<Cartridge, CartridgeError> {
        let rom = std::fs::read(path)?;

        let title_len = rom
            .iter()
            .skip(TITLE_START)
            .take_while(|&ch| *ch != 0)
            .take(16)
            .count();

        let title = String::from_utf8_lossy(&rom[TITLE_START..=TITLE_START + title_len]);

        println!("Your game is called: {}", title);

        let ctype = CartridgeType::try_from(rom[CARTRIDGE_TYPE])
            .map_err(|_| CartridgeError::UnknownCartridgeType(rom[CARTRIDGE_TYPE]))?;

        dbg!(ctype);

        //  x=0:FOR i=0134h TO 014Ch:x=x-MEM[i]-1:NEXT
        let mut checksum = 0u8;
        for i in 0x0134..=0x014C {
            checksum = checksum.wrapping_sub(rom[i]).wrapping_sub(1);
        }

        if rom[0x014D] == checksum {
            println!("Checksum correct");
        } else {
            panic!("Checksum incorrect");
        }

        unimplemented!()
        // Ok(Cartridge {
        //     mem: CartridgeMem {

        //     }
        // })
    }
}

#[derive(Debug)]
pub enum CartridgeError {
    FileAccessError(std::io::Error),
    UnknownCartridgeType(u8),
}

impl From<std::io::Error> for CartridgeError {
    fn from(e: std::io::Error) -> Self {
        CartridgeError::FileAccessError(e)
    }
}

#[allow(non_camel_case_types)]
#[derive(Debug, TryFromPrimitive)]
#[repr(u8)]
enum CartridgeType {
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
    MBC5 = 0x19,
    MBC5_RAM = 0x1A,
    MBC5_RAM_BATTERY = 0x1B,
    MBC5_RUMBLE = 0x1C,
    MBC5_RUMBLE_RAM = 0x1D,
    MBC5_RUMBLE_RAM_BATTERY = 0x1E,
    MBC6 = 0x20,
    MBC7_SENSOR_RUMBLE_RAM_BATTERY = 0x22,
    POCKET_CAMERA = 0xFC,
    BANDAI_TAMA5 = 0xFD,
    HuC3 = 0xFE,
    HuC1_RAM_BATTERY = 0xFF,
}
