use super::lcdc::{SpriteSize, LCDC};
use super::sprite::Sprite;
use std::ops::{Index, IndexMut};

pub struct OAM {
    mem: Box<[u8]>,
    /// Contains the indexes of a *potentially visible* sprites
    /// sorted by their x coordinate (ascending)
    visible_sorted: Vec<u8>,
    is_dirty: bool,
}

const SPRITE_WIDTH: usize = 4;

impl OAM {
    pub fn new() -> OAM {
        OAM {
            mem: vec![0; 0xFEA0 - 0xFE00].into_boxed_slice(),
            visible_sorted: Vec::with_capacity(40),
            is_dirty: true,
        }
    }

    pub fn notify_lcdc_changed(&mut self, lcdc: LCDC) {
        // TODO: Support large sprites
        assert!(matches!(lcdc.sprite_size(), SpriteSize::W8H8))
        // TODO: Set dirty
    }

    pub fn sprites_in_line<'a>(&'a self, ly: u8) -> impl 'a + Iterator<Item = Sprite> {
        debug_assert!(!self.is_dirty);

        // TODO: Check if this is true... Sprites with higher priority, but color 0b00,
        // might actually STILL draw over other sprites at the same X. In that case, the `dedup_by` call
        // below is incorrect, and we need more complicated handling of the situation.
        // use itertools::Itertools;

        self.visible_sorted
            .iter()
            .copied()
            .map(move |id| (id, self.mem[id as usize * SPRITE_WIDTH] as i16 - 16))
            .filter(move |(_, sprite_y)| {
                // TODO: Support large sprites
                ly as i16 >= *sprite_y && (ly as i16) < *sprite_y + 8
            })
            .take(10)
            .map(move |(id, _sprite_y)| {
                Sprite::from_slice(
                    &self.mem[id as usize * SPRITE_WIDTH..id as usize * SPRITE_WIDTH + 4],
                )
            })
        // .dedup_by(|s1, s2| s1.x == s2.x)
    }

    pub fn rebuild(&mut self) {
        if !self.is_dirty {
            return;
        }

        self.visible_sorted.clear();

        for sprite_id in 0..40 {
            let sprite_y = self.mem[sprite_id as usize * SPRITE_WIDTH];
            let sprite_x = self.mem[sprite_id as usize * SPRITE_WIDTH + 1];

            // TODO: Support large sprites
            if sprite_y > 8 && sprite_y < 160 && sprite_x < 166 {
                self.visible_sorted.push(sprite_id);
            }
        }

        // We take this ref to get around a borrowing conflict on self
        let mem = &self.mem;
        self.visible_sorted
            .sort_unstable_by_key(|id| mem[*id as usize * SPRITE_WIDTH + 1]);
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
        self.is_dirty = true;
        &mut self.mem[index as usize]
    }
}
