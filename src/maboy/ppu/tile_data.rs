use super::color::Color;
use super::tile_maps::TileRowAddr;
use fixedbitset::FixedBitSet;
use std::ops::{Index, IndexMut};

/// Memory from 0x8000 - 0x97FF, which is reserved for tile data.
/// That is the CONTENT of each tile, not referencces to tiles.
/// Since tile data is laid out in memory in a really weird way,
/// we calculate a friendlier layout when needed and keep it separate
/// from the raw layout.
pub struct TileData {
    raw_mem: Box<[u8]>,
    pretty_mem: Box<[u8]>,
    dirty_tiles: FixedBitSet,
    is_dirty: bool,
}

pub trait TileRow {
    fn pop_leftmost(&mut self) -> Color;
    fn discard_leftmost(&mut self, n: u8);
}

pub enum SpriteTileRow {
    InOrder(InOrderTileRow),
    Reverse(ReverseTileRow),
}

pub struct InOrderTileRow(u16);

pub struct ReverseTileRow(u16);

// TODO: Remove all the unneccesary repr transparents

impl TileData {
    pub fn new() -> TileData {
        TileData {
            /// Tile rows be like: [u8 color LSB, u8 color MSB] with leftmost pixel at MSB position
            raw_mem: vec![0; 0x9800 - 0x8000].into_boxed_slice(),
            /// Tile rows be like: [u8 with leftmost pixel at [LSB, LSB + 1] and so on]
            pretty_mem: vec![0; 0x9800 - 0x8000].into_boxed_slice(),
            /// Bitset of tiles that were accessed mutably since the last `rebuild()`
            /// Sprite index == Bitset index
            dirty_tiles: FixedBitSet::with_capacity(384), // (0x9800 - 0x8000) / SPRITE_WIDTH
            is_dirty: true,
        }
    }

    pub fn get_row(&self, tile_row_addr: TileRowAddr) -> InOrderTileRow {
        debug_assert!(!self.is_dirty);

        let tile_row_addr = tile_row_addr.into_vram_addr();
        InOrderTileRow(u16::from_le_bytes([
            self.pretty_mem[tile_row_addr as usize],
            self.pretty_mem[tile_row_addr as usize + 1],
        ]))
    }

    pub fn get_row_reverse(&self, tile_row_addr: TileRowAddr) -> ReverseTileRow {
        debug_assert!(!self.is_dirty);

        let tile_row_addr = tile_row_addr.into_vram_addr();
        ReverseTileRow(u16::from_le_bytes([
            self.pretty_mem[tile_row_addr as usize],
            self.pretty_mem[tile_row_addr as usize + 1],
        ]))
    }

    pub fn rebuild(&mut self) {
        if !self.is_dirty {
            return;
        }

        for dirty_id in self.dirty_tiles.ones() {
            for row_addr in (dirty_id * 16..dirty_id * 16 + 16).step_by(2) {
                let row_lower = self.raw_mem[row_addr as usize];
                let row_upper = self.raw_mem[row_addr as usize + 1];

                let mut row_col = 0u16;

                for pix in 0..8 {
                    row_col <<= 2;
                    row_col += ((((row_upper >> pix) & 1) << 1) + ((row_lower >> pix) & 1)) as u16;
                }

                let [row_left, row_right] = row_col.to_le_bytes();
                self.pretty_mem[row_addr as usize] = row_left;
                self.pretty_mem[row_addr as usize + 1] = row_right;
            }
        }
        self.dirty_tiles.clear();

        self.is_dirty = false;
    }
}

impl Index<u16> for TileData {
    type Output = u8;

    fn index(&self, index: u16) -> &Self::Output {
        &self.raw_mem[index as usize]
    }
}

impl IndexMut<u16> for TileData {
    fn index_mut(&mut self, index: u16) -> &mut Self::Output {
        self.is_dirty = true;
        self.dirty_tiles.insert(index as usize / 16);
        &mut self.raw_mem[index as usize]
    }
}

impl TileRow for InOrderTileRow {
    fn pop_leftmost(&mut self) -> Color {
        let col = Color::from_u8_lsb(self.0 as u8);
        self.0 >>= 2;
        col
    }

    fn discard_leftmost(&mut self, n: u8) {
        self.0 >>= n * 2;
    }
}

impl TileRow for ReverseTileRow {
    fn pop_leftmost(&mut self) -> Color {
        self.0 = self.0.rotate_left(2);
        Color::from_u8_lsb(self.0 as u8)
    }

    fn discard_leftmost(&mut self, n: u8) {
        self.0 = self.0.rotate_left(n as u32 * 2);
    }
}

impl TileRow for SpriteTileRow {
    fn pop_leftmost(&mut self) -> Color {
        match self {
            SpriteTileRow::InOrder(row) => row.pop_leftmost(),
            SpriteTileRow::Reverse(row) => row.pop_leftmost(),
        }
    }

    fn discard_leftmost(&mut self, n: u8) {
        match self {
            SpriteTileRow::InOrder(row) => row.discard_leftmost(n),
            SpriteTileRow::Reverse(row) => row.discard_leftmost(n),
        }
    }
}
