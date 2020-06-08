use super::lcdc::{SpriteSize, LCDC};

/// Memory from 0x9800 to 0x9FFF.
/// Contains ids for Window and Background tiles.
pub struct TileMaps {
    pub mem: Box<[u8]>,
    tile_data_starts_at_0x8000: bool,
    bg_tile_map_offset: u16,
    wnd_tile_map_offset: u16,
}

#[repr(transparent)]
pub struct TileRowAddr(u16);

impl TileMaps {
    pub fn new() -> TileMaps {
        TileMaps {
            mem: vec![0; 0xA000 - 0x9800].into_boxed_slice(),
            tile_data_starts_at_0x8000: false,
            bg_tile_map_offset: 0,
            wnd_tile_map_offset: 0,
        }
    }

    pub fn notify_lcdc_changed(&mut self, lcdc: LCDC) {
        self.tile_data_starts_at_0x8000 = lcdc.bg_window_tile_data_start_at_0x8000();
        self.bg_tile_map_offset = lcdc.bg_tile_map_offset();
        self.wnd_tile_map_offset = lcdc.wnd_tile_map_offset();
    }

    pub fn bg_tile_row_at(&self, x: u8, y: u8) -> TileRowAddr {
        self.tile_row_at(self.bg_tile_map_offset, x, y)
    }

    pub fn wnd_tile_row_at(&self, x: u8, y: u8) -> TileRowAddr {
        self.tile_row_at(self.wnd_tile_map_offset, x, y)
    }

    fn tile_row_at(&self, map_offset: u16, x: u8, y: u8) -> TileRowAddr {
        let x = x / 8;
        let tmy = y / 8;
        let subidx_y = y % 8;

        let raw_idx = self.mem[map_offset as usize + (tmy as usize) * 32 + x as usize];

        if self.tile_data_starts_at_0x8000 {
            TileRowAddr(raw_idx as u16 * 16 + subidx_y as u16 * 2)
        } else {
            TileRowAddr(0x800 + raw_idx.wrapping_add(128) as u16 * 16 + subidx_y as u16 * 2)
        }
    }
}

impl TileRowAddr {
    pub fn from_sprite_tile_id(tile_id: u8, subidx_y: u8, sprite_size: SpriteSize) -> TileRowAddr {
        match sprite_size {
            SpriteSize::W8H8 => TileRowAddr(tile_id as u16 * 16 + subidx_y as u16 * 2),
            SpriteSize::W8H16 => {
                if subidx_y < 8 {
                    TileRowAddr((tile_id & 0xFE) as u16 * 16 + subidx_y as u16 * 2)
                } else {
                    TileRowAddr((tile_id | 0x01) as u16 * 16 + (subidx_y - 8) as u16 * 2)
                }
            }
        }
    }

    pub fn into_vram_addr(self) -> u16 {
        self.0
    }
}
