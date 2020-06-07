use super::color::Color;

// TODO: Pallette -> Palette in whole source code

#[derive(Copy, Clone)]
pub struct Palette(pub u8);

impl Palette {
    pub fn apply(&self, col: Color) -> Color {
        unsafe { Color::from_u8_unsafe(self.0.wrapping_shr(2 * col.into_raw() as u32) & 0b11) }
    }
}