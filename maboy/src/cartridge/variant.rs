//! See documentation of [`CartridgeVariant`] for more info

use super::cram::*;
use super::desc::*;
use super::mbc::*;
use super::CartridgeImpl;
use std::{fs, path::Path};

/// For maximum speed, we want to avoid dynamic dispatch for everything that is called
/// in a hot loop. Cartridge memory access is both very hot and very loopy. This enum
/// holds every combination of MBC and RAM that the emulator supports in a separate
/// variant, which can then be used the statically dispatch the emulation loop. Since
/// all variants implement the [`super::Cartridge`] trait, the emulation loop typically
/// only needs to be written once.
///
/// # Example
/// ```
/// // Boilerplate to dispatch all variants
/// fn dispatch_emulator(rom_path: &str, mut cartridge: CartridgeVariant) {
///     match &mut cartridge {
///         CartridgeVariant::Rom(c) => run_emu(c),
///         CartridgeVariant::RomRam(c) => run_emu(c),
///         CartridgeVariant::RomRamBanked(c) => run_emu(c),
///         CartridgeVariant::MBC1(c) => run_emu(c),
///         _ => panic!("And so on... you get the idea"),
///     }
/// }
/// // Only a single method can handle all variants by using generics
/// fn run_emu<C: Cartridge>(mut cartridge: C) {
///     // Emulation loop goes here
/// }
/// ```
pub enum CartridgeVariant {
    Rom(CartridgeImpl<NoMBC<NoCRam>>),
    RomRam(CartridgeImpl<NoMBC<CRamUnbanked>>),
    RomRamBanked(CartridgeImpl<NoMBC<CRamBanked>>),

    MBC1(CartridgeImpl<MBC1<NoCRam>>),
    MBC1Ram(CartridgeImpl<MBC1<CRamUnbanked>>),
    MBC1RamBanked(CartridgeImpl<MBC1<CRamBanked>>),

    MBC2(CartridgeImpl<MBC2>),

    MBC3(CartridgeImpl<MBC3<NoCRam>>),
    MBC3Rtc(CartridgeImpl<MBC3Rtc<NoCRam>>),
    MBC3Ram(CartridgeImpl<MBC3<CRamUnbanked>>),
    MBC3RamBanked(CartridgeImpl<MBC3<CRamBanked>>),
    MBC3RamRtc(CartridgeImpl<MBC3Rtc<CRamUnbanked>>),
    MBC3RamBankedRtc(CartridgeImpl<MBC3Rtc<CRamBanked>>),
}

#[derive(Debug)]
pub enum CartridgeParseError {
    IoError(std::io::Error),

    // Invalid/Missing cartridge data
    /// Size is not a multiple of 0x4000
    InvalidRomSize,

    // Header size is not 0x50 bytes (or larger)
    InvalidHeaderSize,

    /// Header checksum is incorrect
    InvalidHeaderChecksum,

    /// Header declares unknown cartridge type
    InvalidHeaderCartridgeType,

    /// Header declares unknown ROM size
    InvalidHeaderRomSize,

    /// Header declares unknown RAM size
    InvalidHeaderRamSize,

    /// Cartridge does not support metadata
    MetadataNotSuported,

    /// The RTC module could not deserialize the provided metadata
    InvalidRtcMetadata,

    /// Header MIGHT be valid, but this combination of
    /// cartridge type, ROM size and RAM size is currently
    /// not supported.
    Unsupported(CartridgeType, RomSize, RamSize),
}

impl CartridgeVariant {
    /// Attempts to parse a cartridge from a ROM file on disk
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<CartridgeVariant, CartridgeParseError> {
        let rom = fs::read(&path)
            .map_err(|io_err| CartridgeParseError::IoError(io_err))?
            .into_boxed_slice();

        // This condition sets up an important invariant that a lot of code relies upon,
        // for example the MBC code. Change it only if you are sure about what you're doing.
        if rom.len() < 0x8000 || rom.len() % 0x4000 != 0 {
            return Err(CartridgeParseError::InvalidRomSize);
        }

        let header = CartridgeDesc::from_header(&rom[0x100..=0x14F]);

        if !header.has_valid_checksum() {
            return Err(CartridgeParseError::InvalidHeaderChecksum);
        }

        let ctype = header
            .cartridge_type()
            .ok_or(CartridgeParseError::InvalidHeaderCartridgeType)?;
        let rom_size = header
            .rom_size()
            .ok_or(CartridgeParseError::InvalidHeaderRomSize)?;
        let ram_size = header
            .ram_size()
            .ok_or(CartridgeParseError::InvalidHeaderRamSize)?;

        let err_unsupported = Err(CartridgeParseError::Unsupported(ctype, rom_size, ram_size));

        // We have to be very lenient here because cartridges might report incorrect values in the header.
        use CRamBanked as BRam;
        use CRamUnbanked as URam;
        use CartridgeImpl as C;
        use CartridgeType as CT;
        use CartridgeVariant as CV;

        Ok(match ctype {
            // No MBC
            CT::ROM_ONLY | CT::ROM_RAM | CT::ROM_RAM_BATTERY => match ram_size {
                RamSize::RamNone => CV::Rom(C::new(NoMBC::new(rom, NoCRam))),
                RamSize::Ram2Kb | RamSize::Ram8Kb => CV::RomRam(C::new(NoMBC::new(
                    rom,
                    URam::new(ram_size, ctype.has_battery()),
                ))),
                RamSize::Ram32Kb => {
                    CV::RomRamBanked(C::new(NoMBC::new(rom, BRam::new(ctype.has_battery()))))
                }
            },

            // MBC1
            CT::MBC1 | CT::MBC1_RAM | CT::MBC1_RAM_BATTERY => match ram_size {
                RamSize::RamNone => CV::MBC1(C::new(MBC1::new(rom, NoCRam))),
                RamSize::Ram2Kb | RamSize::Ram8Kb => CV::MBC1Ram(C::new(MBC1::new(
                    rom,
                    URam::new(ram_size, ctype.has_battery()),
                ))),

                RamSize::Ram32Kb => {
                    CV::MBC1RamBanked(C::new(MBC1::new(rom, BRam::new(ctype.has_battery()))))
                }
            },

            // MBC2
            CT::MBC2 | CT::MBC2_BATTERY => CV::MBC2(C::new(MBC2::new(rom, ctype.has_battery()))),

            // MBC3
            CT::MBC3 | CT::MBC3_RAM | CT::MBC3_RAM_BATTERY => match ram_size {
                RamSize::RamNone => CV::MBC3(C::new(MBC3::new(rom, NoCRam))),
                RamSize::Ram2Kb | RamSize::Ram8Kb => CV::MBC3Ram(C::new(MBC3::new(
                    rom,
                    URam::new(ram_size, ctype.has_battery()),
                ))),
                RamSize::Ram32Kb => {
                    CV::MBC3RamBanked(C::new(MBC3::new(rom, BRam::new(ctype.has_battery()))))
                }
            },
            CT::MBC3_TIMER_BATTERY | CT::MBC3_TIMER_RAM_BATTERY => match ram_size {
                RamSize::RamNone => CV::MBC3Rtc(C::new(MBC3Rtc::new(rom, NoCRam))),
                RamSize::Ram2Kb | RamSize::Ram8Kb => CV::MBC3RamRtc(C::new(MBC3Rtc::new(
                    rom,
                    URam::new(ram_size, ctype.has_battery()),
                ))),
                RamSize::Ram32Kb => {
                    CV::MBC3RamBankedRtc(C::new(MBC3Rtc::new(rom, BRam::new(ctype.has_battery()))))
                }
            },

            // Anything else is not supported (yet)
            _ => return err_unsupported,
        })
    }
}
