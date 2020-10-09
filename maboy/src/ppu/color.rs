//! Utilities for safe handling of color values

/// A greyscale color value guaranteed to be in the range 0..4 (exclusive)
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct Color(u8);

/// Colors can be converted into this enum to make matching easier. Otherwise,
/// you would always have to match the whole range of u8.
#[repr(u8)]
// Detected as dead code since we only transmute into it and never manually
// create and instance of this
#[allow(dead_code)]
pub enum ColorVal {
    C00 = 0b00,
    C01 = 0b01,
    C10 = 0b10,
    C11 = 0b11,
}

impl Color {
    /// Can cause undefined behaviour when passed a value above 3
    pub unsafe fn from_u8_unchecked(col_raw: u8) -> Color {
        debug_assert!(col_raw <= 0b11);
        Color(col_raw)
    }

    /// Creates a color from the two least significant bytes of a u8
    pub fn from_u8_lsb(col_raw: u8) -> Color {
        Color(col_raw & 0b11)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    pub fn into_val(self) -> ColorVal {
        // Safe because Self is #[repr(transparent)] and can only contain u8 in the range 0-3,
        // which are all legal enum variants
        unsafe { std::mem::transmute(self) }
    }

    pub fn into_raw(self) -> u8 {
        self.0
    }
}
