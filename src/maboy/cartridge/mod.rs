pub mod cartridge_desc;

use cartridge_desc::CartridgeDesc;
use std::fs;
use std::io;
use std::path::Path;

pub struct Cartridge {
    pub(super) bytes: Box<[u8]>,
}

impl Cartridge {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Cartridge, io::Error> {
        // TODO: Do a WHOLE LOT more, and handle errors
        Ok(Cartridge {
            bytes: fs::read(path)?.into_boxed_slice(),
        })
    }
}

impl CartridgeDesc for Cartridge {
    fn header(&self) -> &[u8] {
        &self.bytes[0x100..=0x14F]
    }
}
