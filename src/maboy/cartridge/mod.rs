use std::fs;
use std::path::Path;

pub struct Cartridge {
    pub(super) bytes: Box<[u8]>,
}

impl Cartridge {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Cartridge {
        // TODO: Do a WHOLE LOT more, and handle errors
        Cartridge {
            bytes: fs::read(path).unwrap().into_boxed_slice(),
        }
    }
}
