use crate::util::BitOps;

#[derive(Copy, Clone)]
pub struct LCDC(pub u8);

#[derive(Debug)]
pub enum SpriteSize {
    W8H8,
    W8H16,
}

impl LCDC {
    pub fn lcd_enabled(&self) -> bool {
        self.0.bit(7)
    }

    // TODO: Explain what this offset shiznit is about

    pub fn wnd_tile_map_offset(&self) -> u16 {
        if self.0.bit(6) {
            0x400
        } else {
            0
        }
    }

    pub fn window_enabled(&self) -> bool {
        self.0.bit(5)
    }

    pub fn bg_window_tile_data_start_at_0x8000(&self) -> bool {
        self.0.bit(4)
    }

    pub fn bg_tile_map_offset(&self) -> u16 {
        if self.0.bit(3) {
            0x400
        } else {
            0
        }
    }

    pub fn sprite_size(&self) -> SpriteSize {
        if self.0.bit(2) {
            SpriteSize::W8H16
        } else {
            SpriteSize::W8H8
        }
    }

    pub fn sprites_enabled(&self) -> bool {
        self.0.bit(1)
    }

    pub fn bg_enabled(&self) -> bool {
        self.0.bit(0)
    }
}
