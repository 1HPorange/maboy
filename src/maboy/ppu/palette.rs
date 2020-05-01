use super::color::Color;

pub struct Palette(pub u8);

impl Palette {
    /// In release mode, this will give weird results for illegal color values, but
    /// will not cause undefined behaviour in any case
    pub fn apply(&self, col_raw: u8) -> Color {
        debug_assert!(col_raw <= 3, "Color value outside of allowed range 0..=3");
        unsafe { Color::from_u8_unsafe(self.0.wrapping_shr(2 * col_raw as u32) & 0b11) }
    }
}
