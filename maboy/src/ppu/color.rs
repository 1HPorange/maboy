/// A color value guaranteed to be in the range 0..4
#[derive(Copy, Clone)]
pub struct Color(u8);

#[allow(dead_code)] // Detected as dead code since we only transmute into it
#[repr(u8)]
pub enum ColorVal {
    C00 = 0b00,
    C01 = 0b01,
    C10 = 0b10,
    C11 = 0b11,
}

impl Color {
    pub unsafe fn from_u8_unchecked(col_raw: u8) -> Color {
        debug_assert!(col_raw <= 0b11);
        Color(col_raw)
    }

    // Creates a color from the two least significant bytes of a u8
    pub fn from_u8_lsb(col_raw: u8) -> Color {
        Color(col_raw & 0b11)
    }

    pub fn is_zero(&self) -> bool {
        self.0 == 0
    }

    pub fn into_val(self) -> ColorVal {
        unsafe { std::mem::transmute(self) }
    }

    pub fn into_raw(self) -> u8 {
        self.0
    }
}
