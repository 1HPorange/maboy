use crate::util::BitOps;

#[derive(Copy, Clone)]
pub struct Sprite {
    pub y: u8,
    pub x: u8,
    pub id: u8,
    pub flags: SpriteFlags,
}

#[derive(Copy, Clone)]
pub struct SpriteFlags(u8);

impl Sprite {
    /// Creates a sprite (usually from a slice of OAM RAM) by copying bytes
    pub fn from_slice(mem: &[u8]) -> Sprite {
        debug_assert!(mem.len() >= 4);

        Sprite {
            y: mem[0],
            x: mem[1],
            id: mem[2],
            flags: SpriteFlags(mem[3]),
        }
    }
}

impl SpriteFlags {
    pub fn is_occluded(&self) -> bool {
        self.0.bit(7)
    }

    pub fn y_flipped(&self) -> bool {
        self.0.bit(6)
    }

    pub fn x_flipped(&self) -> bool {
        self.0.bit(5)
    }

    pub fn uses_alternative_pallette(&self) -> bool {
        self.0.bit(4)
    }
}
