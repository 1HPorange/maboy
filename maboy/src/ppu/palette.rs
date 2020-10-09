//! Provides the thin [`Palette`] wrapper around bytes, which
//! provides [`Palette::apply`] method to transform [`Color`].

use super::color::Color;

// TODO: Pallette -> Palette in whole source code

#[derive(Copy, Clone)]
pub struct Palette(pub u8);

impl Palette {
    pub fn apply(&self, col: Color) -> Color {
        Color::from_u8_lsb(self.0.wrapping_shr(2 * col.into_raw() as u32))
    }
}
