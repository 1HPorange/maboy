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
    pub fn from_slice(mem: &[u8]) -> Sprite {
        Sprite {
            y: mem[0],
            x: mem[1],
            id: mem[2],
            flags: SpriteFlags(mem[3]),
        }
    }
}

// TODO: Research if it would be faster to make this struct Copy, then all these method could take self instead of &self
impl SpriteFlags {
    pub fn is_occluded(self) -> bool {
        self.0.bit(7)
    }

    pub fn y_flipped(self) -> bool {
        self.0.bit(6)
    }

    pub fn x_flipped(self) -> bool {
        self.0.bit(5)
    }

    pub fn uses_alternative_pallette(self) -> bool {
        self.0.bit(4)
    }
}
