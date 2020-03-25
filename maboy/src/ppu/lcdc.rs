//! Helper code to deal with register 0xFF40, the LCD Control register (LCDC)

use crate::util::BitOps;

/// Thin wrapper providing some methods to query the current value of LCDC
#[derive(Copy, Clone)]
pub struct LCDC(pub u8);

#[derive(Debug, PartialEq)]
pub enum SpriteSize {
    W8H8,
    W8H16,
}

impl LCDC {
    pub fn lcd_enabled(&self) -> bool {
        self.0.bit(7)
    }

    /// The offset of the window tile map *from the beginning of tilemap VRAM (0x9800)*
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

    /// Whether BG and WND tile data starts at 0x8000 or 0x8800
    pub fn bg_window_tile_data_start_at_0x8000(&self) -> bool {
        self.0.bit(4)
    }

    /// The offset of the background tile map *from the beginning of tilemap VRAM (0x9800)*
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

// TODO: This could be implemented as a generic paramter of OAM and PixelQueue...
// Saves a few branches per sprite, but probably not worth it.
impl SpriteSize {
    pub fn height(&self) -> u8 {
        match self {
            SpriteSize::W8H8 => 8,
            SpriteSize::W8H16 => 16,
        }
    }
}
