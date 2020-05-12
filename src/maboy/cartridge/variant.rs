use super::cram::*;
use super::desc::*;
use super::mbc::*;
use super::Cartridge;
use std::fs;
use std::path::Path;

pub enum CartridgeVariant {
    Unbanked(Cartridge<NoMBC<NoCRAM>>),
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
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<CartridgeVariant, CartridgeParseError> {
        let rom = fs::read(path)
            .map_err(|io_err| CartridgeParseError::IoError(io_err))?
            .into_boxed_slice();

        if rom.len() < 0x4000 || rom.len() % 0x4000 != 0 {
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

        Ok(match (ctype, rom_size, ram_size) {
            (CartridgeType::ROM_ONLY, RomSize::RomNoBanking, RamSize::RamNone) => {
                CartridgeVariant::Unbanked(Cartridge::new(NoMBC::new(rom, NoCRAM)))
            }
            _ => return Err(CartridgeParseError::Unsupported(ctype, rom_size, ram_size)),
        })
    }
}
