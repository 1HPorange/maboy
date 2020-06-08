use super::color::{Color, ColorVal};
use super::mem_frame::MemPixel;
use super::oam::OAM;
use super::ppu_registers::PPURegisters;
use super::sprite::Sprite;
use super::tile_data::{SpriteTileRow, TileData, TileRow};
use super::tile_maps::{TileMaps, TileRowAddr};
use super::Palette;

pub struct PixelQueue {
    quads: [PixelQuad; 40],
}

/// The source of a pixel can be precomputed at the beginning of a line,
/// which can save some cycles later. Window and sprite pixels can actually be
/// calculated instantly, while background pixel calculation has to be
/// deferred, since it can change mid-scanline.
#[derive(Copy, Clone)]
struct PixelQuad {
    /// Contains the actual *paletted* pixel colors with the leftmost pixel color
    /// being at the least significant 2 bits. If color is unknown (for BG sprites),
    /// pixel color is set as 0b00. Sprite colors are also contained,
    /// even if the color might later get overwritten.
    pixel_col: u8,

    /// In the same order as `pixel_types`, each two bits describe a pixel source:
    /// 0b00 - Background (needs to be calculated later)
    /// 0b01 - Window or blended Sprite [Priority 1] - This pixel is final and paletted
    /// 0b10 - Sprite with priority 1 over BG - Paletted, even though it might be overwritten
    /// 0b11 - Sprite [Priority 0] (over BG or Window) - This pixel is final and paletted
    /// Note that in this representation, the lower bit indicates if a pixel is final.
    pixel_src: u8,
}

impl PixelQuad {
    fn zero() -> PixelQuad {
        PixelQuad {
            pixel_col: 0,
            pixel_src: 0,
        }
    }
}

impl PixelQueue {
    pub fn new() -> PixelQueue {
        PixelQueue {
            quads: [PixelQuad::zero(); 40],
        }
    }

    pub fn push_scanline(
        &mut self,
        ppu_reg: &PPURegisters,
        tile_maps: &TileMaps,
        tile_data: &TileData,
        oam: &OAM,
    ) -> u8 {
        // TODO: See if BG, Window and Sprites can be enabled mid scanline
        // If yes, we cannot really mark any pixel as final and might just
        // get rid of the `pixel_src` field

        // Forget about the last line (PERF: This is only necessary if LCD was switched off mid-line,
        // otherwise every pixel was already shifted out)
        self.quads = [PixelQuad::zero(); 40];

        let mut num_sprites = 0;

        if ppu_reg.lcdc.sprites_enabled() {
            for sprite in oam.sprites_in_line(ppu_reg.ly) {
                self.draw_sprite(tile_data, ppu_reg, sprite, (ppu_reg.ly + 16) - sprite.y);
                num_sprites += 1;
            }
        }

        // `ppu.wx_reg >= 7` is not a requirement on hardware, so this boolean is technically incorrect.
        // Anyway, stuff gets much easier to write if we do it this way for now. The PPU
        // currently outputs a warning if any value < 7 is written to WX. TODO: Implement correctly
        let window_in_line = ppu_reg.lcdc.window_enabled()
            && ppu_reg.ly >= ppu_reg.wy
            && ppu_reg.wx <= 166
            && ppu_reg.wx >= 7;

        if window_in_line {
            self.draw_window(
                tile_data,
                tile_maps,
                ppu_reg.bgp,
                ppu_reg.wx,
                ppu_reg.ly - ppu_reg.wy,
            );
        }

        // Optimization: If BG is disabled, we can also mark those pixels as final
        if !ppu_reg.lcdc.bg_enabled() {
            // Just set all unknown pixel sources to known
            self.draw_empty_bg();
        }

        num_sprites
    }

    pub fn pop_pixel_quad(
        &self,
        tile_data: &TileData,
        tile_maps: &TileMaps,
        ppu_reg: &PPURegisters,
        line: &mut [MemPixel],
        quad_id: u8,
    ) {
        let mut quad = self.quads[quad_id as usize];

        // PERF: Do once and pass in
        let bg_y = ppu_reg.ly.wrapping_add(ppu_reg.scy);

        for pidx in (quad_id * 4)..(quad_id * 4 + 4) {
            let pix = &mut line[pidx as usize];

            *pix = match quad.pixel_src & 0b11 {
                0b00 => {
                    let col = self.fetch_bg_pix(
                        tile_data,
                        tile_maps,
                        pidx.wrapping_add(ppu_reg.scx),
                        bg_y,
                    );
                    MemPixel::from(ppu_reg.bgp.apply(col))
                }
                0b10 => {
                    let bg_col = self.fetch_bg_pix(
                        tile_data,
                        tile_maps,
                        pidx.wrapping_add(ppu_reg.scx),
                        bg_y,
                    );

                    let sprite_col = Color::from_u8_lsb(quad.pixel_col);

                    MemPixel::from(blend_sprite_col(sprite_col, bg_col, ppu_reg.bgp))
                }
                _ => MemPixel::from(Color::from_u8_lsb(quad.pixel_col)),
            };

            quad.pixel_col >>= 2;
            quad.pixel_src >>= 2;
        }
    }

    fn fetch_bg_pix(
        &self,
        tile_data: &TileData,
        tile_maps: &TileMaps,
        bg_x: u8,
        bg_y: u8,
    ) -> Color {
        let row_addr = tile_maps.bg_tile_row_at(bg_x, bg_y);
        let mut row = tile_data.get_row(row_addr);

        row.discard_leftmost(bg_x % 8);
        row.pop_leftmost()
    }

    fn draw_sprite(
        &mut self,
        tile_data: &TileData,
        ppu_reg: &PPURegisters,
        sprite: Sprite,
        sprite_line: u8,
    ) {
        let sprite_size = ppu_reg.lcdc.sprite_size();

        let row_addr = if sprite.flags.y_flipped() {
            TileRowAddr::from_sprite_tile_id(
                sprite.id,
                sprite_size.height() - sprite_line,
                sprite_size,
            )
        } else {
            TileRowAddr::from_sprite_tile_id(sprite.id, sprite_line, sprite_size)
        };

        let row = if sprite.flags.x_flipped() {
            SpriteTileRow::Reverse(tile_data.get_row_reverse(row_addr))
        } else {
            SpriteTileRow::InOrder(tile_data.get_row(row_addr))
        };

        let pixel_src = if sprite.flags.is_occluded() {
            0b10 // Pixel might get occluded by BG or WND
        } else {
            0b11 // Pixel is final
        };

        match row {
            SpriteTileRow::InOrder(row) => self.draw_sprite_row(ppu_reg, row, sprite, pixel_src),
            SpriteTileRow::Reverse(row) => self.draw_sprite_row(ppu_reg, row, sprite, pixel_src),
        }
    }

    fn draw_window(
        &mut self,
        tile_data: &TileData,
        tile_maps: &TileMaps,
        bgp: Palette,
        wx: u8,
        window_line: u8,
    ) {
        // TODO: Investigate WX < 7 and handle it correctly

        let window_pix_width = 167 - wx;
        let visible_tiles = (window_pix_width + 7) / 8;
        let mut tile_rows = (0..visible_tiles).map(|tile_idx| {
            let row_addr = tile_maps.wnd_tile_row_at(tile_idx * 8, window_line);
            tile_data.get_row(row_addr)
        });

        let mut pidx = wx.saturating_sub(7);

        // Draw all tiles that are completely on-screen
        for mut row in tile_rows.by_ref().take(visible_tiles as usize - 1) {
            for _ in 0..8 {
                let col = row.pop_leftmost();
                self.draw_window_pix(bgp, pidx, col);
                pidx += 1;
            }
        }

        // Draw the last (possibly incomplete) tile
        let mut last_row = tile_rows.next().unwrap();

        while pidx < 160 {
            let col = last_row.pop_leftmost();
            self.draw_window_pix(bgp, pidx, col);
            pidx += 1;
        }
    }

    fn draw_empty_bg(&mut self) {
        for quad in self.quads.iter_mut() {
            // This basically sets all pixels to final, regardless of their content.
            // This works because nothing is drawn after this function.
            quad.pixel_src = 0xff;
        }
    }

    fn draw_sprite_row<R: TileRow>(
        &mut self,
        ppu_reg: &PPURegisters,
        mut row: R,
        sprite: Sprite,
        pixel_src: u8,
    ) {
        // If the sprite goes over the left edge of the screen, we disacrd some pixels
        row.discard_leftmost(7u8.saturating_sub(sprite.x));

        for pidx in sprite.x.max(8) - 8..sprite.x.min(159) {
            let col = row.pop_leftmost();
            self.draw_sprite_pix(sprite, ppu_reg.obp0, ppu_reg.obp1, pidx, col, pixel_src);
        }
    }

    fn draw_sprite_pix(
        &mut self,
        sprite: Sprite,
        obp0: Palette,
        obp1: Palette,
        pidx: u8,
        col: Color,
        src: u8,
    ) {
        // TODO: Check if this handles the situation of overwriting
        // a higher priority sprite with color value 00 correctly.

        if !col.is_zero() {
            // The sprite color is non-zero, so we actually have to do work

            let quad_idx = pidx / 4;
            let quad_subidx = pidx % 4;
            let quad = &mut self.quads[quad_idx as usize];

            let old_src = quad.pixel_src >> (quad_subidx * 2);

            if old_src != 0 {
                return; // A higher priority sprite was already drawn here
            }

            let col = if sprite.flags.uses_alternative_pallette() {
                obp1.apply(col)
            } else {
                obp0.apply(col)
            };

            quad.pixel_col |= col.into_raw() << (quad_subidx * 2);
            quad.pixel_src |= src << (quad_subidx * 2);
        }
    }

    fn draw_window_pix(&mut self, bgp: Palette, pidx: u8, col: Color) {
        let quad_idx = pidx / 4;
        let quad_subidx = pidx % 4;
        let quad = &mut self.quads[quad_idx as usize];

        let quad_shift = quad_subidx * 2;

        let old_src = quad.pixel_src >> quad_shift;

        if old_src & 1 == 1 {
            return; // The pixel is already final, we are done here
        }

        let col = if old_src & 0b10 == 0b10 {
            // This pixel is a partially occluded sprite... Jesus Christ!

            // Clear pixel color and pixel source
            quad.pixel_col &= (!0b11u8).rotate_left(quad_shift as u32);
            quad.pixel_src &= (!0b11u8).rotate_left(quad_shift as u32);

            // Blend sprite and BG color
            let sprite_col = Color::from_u8_lsb(quad.pixel_col >> quad_shift);
            blend_sprite_col(sprite_col, col, bgp)
        } else {
            // The pixel has col and src 0b00, so we draw over it
            bgp.apply(col)
        };

        quad.pixel_col |= col.into_raw() << quad_shift;
        quad.pixel_src |= 0b01 << quad_shift;
    }
}

/// For sprites with OBJ-to-BG priority 1, this function calculates
/// the resulting color of a blend with a BG/WND color
fn blend_sprite_col(sprite_col: Color, bg_col: Color, bg_palette: Palette) -> Color {
    match bg_col.into_val() {
        ColorVal::C00 => sprite_col,
        _ => bg_palette.apply(bg_col),
    }
}
