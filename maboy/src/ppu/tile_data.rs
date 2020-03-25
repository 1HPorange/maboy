//! See documentation of [`TileData`]

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
    /// Tile data as the Game Boy CPU reads and writes
    raw_mem: Box<[u8]>,
    /// Tile data where pixel colors are not split across two bytes,
    /// but where pixel 0 (leftmost) is at the two least significant
    /// bits of a byte, pixel 1 is at the next higher two bits, etc.
    pretty_mem: Box<[u8]>,
    /// Bitset where a 1 signals that the tile at that index was mutably
    /// accessed since the last [`rebuild`] call.
    dirty_tiles: FixedBitSet,
    /// Set to true if *any* tile was mutable accessed. Used to avoid
    /// unneccesary queries of [`dirty_tiles`]
    is_dirty: bool,
}

/// A single row of pixels within a tile. Modifiying instances of this
/// struct does *not* modify the backing tile array
pub trait TileRow {
    /// Returns the leftmost pixel of the tile and removes it
    fn pop_leftmost(&mut self) -> Color;

    /// Removes the n leftmost pixels from the tile row
    fn discard_leftmost(&mut self, n: u8);
}

pub enum SpriteTileRow {
    InOrder(InOrderTileRow),
    Reverse(ReverseTileRow),
}

pub struct InOrderTileRow(u16);

pub struct ReverseTileRow(u16);

const TILE_BYTE_WIDTH: usize = 16;

// TODO: Remove all the unneccesary repr transparents for all files

impl TileData {
    pub fn new() -> TileData {
        TileData {
            raw_mem: vec![0; 0x9800 - 0x8000].into_boxed_slice(),
            pretty_mem: vec![0; 0x9800 - 0x8000].into_boxed_slice(),
            dirty_tiles: FixedBitSet::with_capacity((0x9800 - 0x8000) / TILE_BYTE_WIDTH),
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
            for row_addr in (dirty_id * TILE_BYTE_WIDTH
                ..dirty_id * TILE_BYTE_WIDTH + TILE_BYTE_WIDTH)
                .step_by(2)
            {
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
        self.dirty_tiles.insert(index as usize / TILE_BYTE_WIDTH);
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
