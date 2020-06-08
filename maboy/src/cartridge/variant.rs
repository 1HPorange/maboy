use super::cram::*;
use super::desc::*;
use super::mbc::*;
use super::Cartridge;
use std::fs;

pub enum CartridgeVariant {
    Rom(Cartridge<NoMBC<NoCRam>>),
    RomRam(Cartridge<NoMBC<CRamUnbanked>>),
    RomRamBat(Cartridge<NoMBC<CRamUnbanked>>),

    MBC1(Cartridge<MBC1<NoCRam>>),
    MBC1Ram(Cartridge<MBC1<CRamUnbanked>>),
    MBC1RamBat(Cartridge<MBC1<CRamUnbanked>>),

    MBC2(Cartridge<MBC2>),
    MBC2Bat(Cartridge<MBC2>),
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

        let header = CartridgeDesc::from_header(&rom[0x100..=0x14F]);

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

        let err_unsupported = Err(CartridgeParseError::Unsupported(ctype, rom_size, ram_size));

        // We have to be very lenient here because cartridges might report incorrect values in the header.
        use CRamUnbanked as URam;
        use Cartridge as C;
        use CartridgeType as CT;
        use CartridgeVariant as CV;

        Ok(match ctype {
            // No MBC
            CT::ROM_ONLY | CT::ROM_RAM | CT::ROM_RAM_BATTERY => match ram_size {
                RamSize::RamNone => CV::Rom(C::new(path, NoMBC::new(rom, NoCRam))),
                RamSize::Ram2Kb | RamSize::Ram8Kb => {
                    if ctype.has_battery() {
                        CV::RomRamBat(C::new(path, NoMBC::new(rom, URam::new(ram_size))))
                    } else {
                        CV::RomRam(C::new(path, NoMBC::new(rom, URam::new(ram_size))))
                    }
                }
                RamSize::Ram32Kb => return err_unsupported,
            },
            // MBC1
            CT::MBC1 | CT::MBC1_RAM | CT::MBC1_RAM_BATTERY => match ram_size {
                RamSize::RamNone => CV::MBC1(C::new(path, MBC1::new(rom, NoCRam))),
                RamSize::Ram2Kb | RamSize::Ram8Kb => {
                    if ctype.has_battery() {
                        CV::MBC1RamBat(C::new(path, MBC1::new(rom, URam::new(ram_size))))
                    } else {
                        CV::MBC1Ram(C::new(path, MBC1::new(rom, URam::new(ram_size))))
                    }
                }
                RamSize::Ram32Kb => return err_unsupported,
            },
            CT::MBC2 => CV::MBC2(C::new(path, MBC2::new(rom))),
            CT::MBC2_BATTERY => CV::MBC2Bat(C::new(path, MBC2::new(rom))),
            _ => return err_unsupported,
        })
    }
}
