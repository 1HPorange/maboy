/// Contains both the working RAM (WRAM) and high ram (HRAM) sectors of
/// internal Game Boy memory in a continuous array in memory.
pub struct InternalMem {
    pub(super) wram: Box<[u8]>,
    pub(super) hram: Box<[u8]>,
}

const WRAM_LEN: usize = 0xE000 - 0xC000;
const HRAM_LEN: usize = 0xFFFF - 0xFF80;

impl InternalMem {
    pub fn new() -> InternalMem {
        InternalMem {
            wram: vec![0; WRAM_LEN].into_boxed_slice(),
            hram: vec![0; HRAM_LEN].into_boxed_slice(),
        }
    }
}
