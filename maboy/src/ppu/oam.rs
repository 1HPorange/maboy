//! See [`OAM`]

use super::lcdc::{SpriteSize, LCDC};
use super::sprite::Sprite;
use std::ops::{Index, IndexMut};

/// OAM memory (0xFE00 - 0xFEA0) with an internal cache structure to
/// provide faster access to releavent sprites.
pub struct OAM {
    /// The raw, unaltered OAM memory
    mem: Box<[u8]>,
    /// Contains the indexes of a *potentially visible* sprites
    /// sorted by their x coordinate (ascending). This allows for
    /// very efficient search for visible sprites on a given scanline.
    visible_sorted: Vec<u8>,
    /// True if [`self.visible_sorted`] *might* not represent the current
    /// contents of [`mem`] correctly. This is set by the IndexMut impl.
    is_dirty: bool,
    /// Sprite size for which [`self.visible_sorted`] was built. If the global
    /// sprite size is changed, this cache needs to be rebuilt.
    sprite_size: SpriteSize,
}

const SPRITE_BYTE_WIDTH: usize = 4;

impl OAM {
    pub fn new() -> OAM {
        OAM {
            mem: vec![0; 0xFEA0 - 0xFE00].into_boxed_slice(),
            visible_sorted: Vec::with_capacity(40),
            is_dirty: true,
            sprite_size: SpriteSize::W8H8,
        }
    }

    /// Must be called after the LCDC register was written to
    pub fn notify_lcdc_changed(&mut self, lcdc: LCDC) {
        // If sprite size was changed, we have to rebuild our visible sprite cache
        if self.sprite_size != lcdc.sprite_size() {
            self.sprite_size = lcdc.sprite_size();
            self.is_dirty = true;
        }
    }

    /// Returns an iterator of up to 10 sprites in a given scanline, since this is the
    /// maximum amount of sprites that the Game Boy can draw.
    pub fn sprites_in_line<'a>(&'a self, ly: u8) -> impl 'a + Iterator<Item = Sprite> {
        debug_assert!(!self.is_dirty);

        // TODO: Check if this is true... Sprites with higher priority, but color 0b00,
        // might actually STILL draw over other sprites at the same X. In that case, the `dedup_by` call
        // below is incorrect, and we need more complicated handling of the situation.
        // For now, we just let all sprites live.

        self.visible_sorted
            .iter()
            .copied()
            .map(move |id| (id, self.mem[id as usize * SPRITE_BYTE_WIDTH] as i16 - 16))
            .filter(move |(_, sprite_y)| {
                (ly as i16) >= *sprite_y
                    && (ly as i16) < *sprite_y + self.sprite_size.height() as i16
            })
            .take(10)
            .map(move |(id, _sprite_y)| {
                Sprite::from_slice(
                    &self.mem[id as usize * SPRITE_BYTE_WIDTH..id as usize * SPRITE_BYTE_WIDTH + 4],
                )
            })
        // .dedup_by(|s1, s2| s1.x == s2.x)
    }

    /// Rebuilds the internal cache; It is necessary to call this each scanline, after OAM
    /// becomes inaccesible for the CPU but before [`self.sprites_in_line`] is called.
    pub fn rebuild(&mut self) {
        if !self.is_dirty {
            return;
        }

        self.visible_sorted.clear();

        for sprite_id in 0..40 {
            let sprite_y = self.mem[sprite_id as usize * SPRITE_BYTE_WIDTH];
            let sprite_x = self.mem[sprite_id as usize * SPRITE_BYTE_WIDTH + 1];

            // TODO: Investigate if sprites with x == 0 count towards the sprite limit.
            // if no, we can already fltier them out here
            if sprite_y > self.sprite_size.height() && sprite_y < 160 && sprite_x < 168 {
                self.visible_sorted.push(sprite_id);
            }
        }

        // We take this ref to get around a borrowing conflict on self
        let mem = &self.mem;
        self.visible_sorted
            .sort_unstable_by_key(|id| mem[*id as usize * SPRITE_BYTE_WIDTH + 1]);
        self.is_dirty = false;
    }
}

impl Index<u16> for OAM {
    type Output = u8;

    fn index(&self, index: u16) -> &Self::Output {
        &self.mem[index as usize]
    }
}

impl IndexMut<u16> for OAM {
    fn index_mut(&mut self, index: u16) -> &mut Self::Output {
        // TODO: See if it is worth not implementing this and making an explicit write method;
        // That way, we could check if the new data is actually different from the old one,
        // and only set the dirty flag in case it is.
        self.is_dirty = true;

        &mut self.mem[index as usize]
    }
}
