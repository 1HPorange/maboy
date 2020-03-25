use std::pin::Pin;
/// Contains both the working RAM (WRAM) and high ram (HRAM) sectors of
/// internal Game Boy memory in a continuous array in memory.
pub struct InternalMem {
    pub(super) wram: &'static mut [u8],
    pub(super) hram: &'static mut [u8],
    #[allow(dead_code)] // This thing is not dead, but Rust doesn't understand that
    backing: Pin<Box<[u8]>>,
}

const WRAM_LEN: usize = 0xE000 - 0xC000;
const HRAM_LEN: usize = 0xFFFF - 0xFF80;

impl InternalMem {
    pub fn new() -> InternalMem {
        use std::mem::transmute as forget_lifetime;

        let mut backing = Pin::new(vec![0; WRAM_LEN + HRAM_LEN].into_boxed_slice());

        unsafe {
            InternalMem {
                wram: forget_lifetime(&mut backing[..WRAM_LEN]),
                hram: forget_lifetime(&mut backing[WRAM_LEN..WRAM_LEN + HRAM_LEN]),
                backing,
            }
        }
    }
}
