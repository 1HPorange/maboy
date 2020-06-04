use super::cram::*;
use super::desc::*;
use super::mbc::*;
use super::Cartridge;
use std::fs;

pub enum CartridgeVariant {
    RomOnly(Cartridge<NoMBC<NoCRAM>>),
    MBC1NoRam(Cartridge<MBC1<NoCRAM>>),
    MBC1UnbankedRamNoBat(Cartridge<MBC1<CRAMUnbanked>>),
    MBC1UnbankedRamBat(Cartridge<MBC1<CRAMUnbanked>>),
}

#[derive(Debug)]
pub enum CartridgeParseError {
    IoError(std::io::Error),

    // Invalid/Missing cartridge data
    /// Size is not a multiple of 0x4000
    InvalidSize,

    /// Header checksum is incorrect
    InvalidChecksum,

    /// Header declares unknown cartridge type
    InvalidCartridgeType,

    /// Header declares unknown ROM size
    InvalidRomSize,

    /// Header declares unknown RAM size
    InvalidRamSize,

    /// Header MIGHT be valid, but this combination of
    /// cartridge type, ROM size and RAM size is currently
    /// not supported.
    Unsupported(CartridgeType, RomSize, RamSize),
}

impl CartridgeVariant {
    pub fn from_file(path: String) -> Result<CartridgeVariant, CartridgeParseError> {
        let rom = fs::read(&path)
            .map_err(|io_err| CartridgeParseError::IoError(io_err))?
            .into_boxed_slice();

        // This condition sets up an important invariant that a lot of code relies upon,
        // for example the MBC code. Change it only if you are sure about what you're doing.
        if rom.len() < 0x8000 || rom.len() % 0x4000 != 0 {
            return Err(CartridgeParseError::InvalidSize);
        }

        let header = CartridgeDesc(&rom[0x100..=0x14F]);

        if !header.has_valid_checksum() {
            return Err(CartridgeParseError::InvalidChecksum);
        }

        let ctype = header
            .cartridge_type()
            .ok_or(CartridgeParseError::InvalidCartridgeType)?;
        let rom_size = header
            .rom_size()
            .ok_or(CartridgeParseError::InvalidRomSize)?;
        let ram_size = header
            .ram_size()
            .ok_or(CartridgeParseError::InvalidRamSize)?;

        let err = Err(CartridgeParseError::Unsupported(ctype, rom_size, ram_size));

        Ok(match ctype {
            CartridgeType::ROM_ONLY | CartridgeType::ROM_RAM | CartridgeType::ROM_RAM_BATTERY => {
                match ram_size {
                    RamSize::RamNone => {
                        CartridgeVariant::RomOnly(Cartridge::new(path, NoMBC::new(rom, NoCRAM)))
                    }
                    _ => unimplemented!(),
                }
            }
            CartridgeType::MBC1 => match ram_size {
                RamSize::RamNone => {
                    CartridgeVariant::MBC1NoRam(Cartridge::new(path, MBC1::new(rom, NoCRAM)))
                }
                _ => return err,
            },
            CartridgeType::MBC1_RAM => match ram_size {
                RamSize::RamNone => return err,
                RamSize::Ram32Kb => unimplemented!(),
                _ => CartridgeVariant::MBC1UnbankedRamNoBat(Cartridge::new(
                    path,
                    MBC1::new(rom, CRAMUnbanked::new(ram_size)),
                )),
            },
            CartridgeType::MBC1_RAM_BATTERY => match ram_size {
                RamSize::RamNone => return err,
                RamSize::Ram32Kb => unimplemented!(),
                _ => CartridgeVariant::MBC1UnbankedRamBat(Cartridge::new(
                    path,
                    MBC1::new(rom, CRAMUnbanked::new(ram_size)),
                )),
            },
            _ => return err,
        })
    }
}
