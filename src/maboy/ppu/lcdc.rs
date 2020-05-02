use crate::maboy::address::VRAM_START_ADDR;
use crate::maboy::util::BitOps;

pub struct LCDC(pub u8);

pub enum SpriteSize {
    W8H8,
    W8H16,
}

impl LCDC {
    pub fn lcd_enabled(&self) -> bool {
        self.0.bit(7)
    }

    pub fn window_tile_map_addr(&self) -> u16 {
        if self.0.bit(6) {
            0x9C00 - VRAM_START_ADDR
        } else {
            0x9800 - VRAM_START_ADDR
        }
    }

    pub fn window_enabled(&self) -> bool {
        self.0.bit(5)
    }

    pub fn bg_window_tile_data_addr(&self) -> u16 {
        if self.0.bit(4) {
            0x8000 - VRAM_START_ADDR
        } else {
            0x8800 - VRAM_START_ADDR
        }
    }

    // TODO: Explain
    pub fn transform_tile_map_index(&self, index: u8) -> u8 {
        if self.0.bit(4) {
            index
        } else {
            index.wrapping_add(128)
        }
    }

    pub fn bg_tile_map_addr(&self) -> u16 {
        if self.0.bit(3) {
            0x9C00 - VRAM_START_ADDR
        } else {
            0x9800 - VRAM_START_ADDR
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
