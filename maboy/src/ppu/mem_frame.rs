//! See documentation of [`MemFrame`]

use super::color::Color;

const WIDTH: usize = 160;
const HEIGHT: usize = 144;

/// RGBA array representing the Game Boys LCD screen. This is the direct target
/// of all rendering code; Each scanline is written directly into [`MemFrame`],
/// without any buffering.
///
/// In some cases (e.g. when the LCD is turned off), this array will contain old
/// data. This should never be a problem for normal operation of the emulator,
/// since it will only display finished frames, but is important to keep in mind
/// during frame debugging.
pub struct MemFrame {
    data: Box<[MemPixel]>,
}

/// RGBA color values without padding. These should be directly mappable to any
/// decent graphics API.
#[derive(Copy, Clone)]
#[repr(C)]
pub struct MemPixel {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl MemFrame {
    pub fn new() -> MemFrame {
        MemFrame {
            data: vec![MemPixel::CLEAR; WIDTH * HEIGHT].into_boxed_slice(),
        }
    }

    /// Retrieves the entire pixel buffer (read-only). The frontend is responsible
    /// for copying the content into some native texture format.
    ///
    /// This method should only be called when a frame is finished; Otherwise, garbage
    /// might be displayed.

    pub fn data(&self) -> &[MemPixel] {
        &self.data
    }

    /// Retrieves one entire scanline
    pub fn line(&mut self, ly: u8) -> &mut [MemPixel] {
        &mut self.data[WIDTH * ly as usize..WIDTH * ly as usize + WIDTH]
    }
}

// TODO: Make this configurable
/// The conversion from 2-bit color values to RGBA values
impl From<Color> for MemPixel {
    fn from(col: Color) -> Self {
        // These values simulate the original Game Boy's signature green tint...

        use super::color::ColorVal;
        match col.into_val() {
            ColorVal::C00 => MemPixel::new(239, 255, 222, 255),
            ColorVal::C01 => MemPixel::new(173, 215, 148, 255),
            ColorVal::C10 => MemPixel::new(82, 146, 115, 255),
            ColorVal::C11 => MemPixel::new(24, 52, 66, 255),
        }

        // ... and this conversion results in a direct mapping to grayscale values

        // MemPixel::from_grayscale(255 - 85 * col.into_raw())
    }
}

impl MemPixel {
    /// A fully transparent black pixel
    const CLEAR: MemPixel = MemPixel::new(0, 0, 0, 0);

    pub const fn new(r: u8, g: u8, b: u8, a: u8) -> MemPixel {
        MemPixel { r, g, b, a }
    }

    /// Private to avoid confusion about what kind of color values this takes (it is 0-255)
    const fn _from_grayscale(grayscale: u8) -> MemPixel {
        MemPixel::new(grayscale, grayscale, grayscale, 0xff)
    }
}
