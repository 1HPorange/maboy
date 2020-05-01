/// A color value guaranteed to be in the range 0..4
pub struct Color(u8);

#[repr(u8)]
pub enum ColorVal {
    C00 = 0b00,
    C01 = 0b01,
    C10 = 0b10,
    C11 = 0b11,
}

impl Color {
    pub unsafe fn from_u8_unsafe(col_raw: u8) -> Color {
        Color(col_raw)
    }

    pub fn into_val(self) -> ColorVal {
        unsafe { std::mem::transmute(self) }
    }

    pub fn into_raw(self) -> u8 {
        self.0
    }
}
