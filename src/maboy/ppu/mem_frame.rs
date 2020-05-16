use super::color::{Color, ColorVal};

const WIDTH: usize = 160;
const HEIGHT: usize = 144;
pub struct MemFrame {
    data: Box<[MemPixel]>,
}

#[derive(Copy, Clone)]
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

    pub fn data(&self) -> &[MemPixel] {
        &self.data
    }

    pub fn line(&mut self, ly: u8) -> &mut [MemPixel] {
        &mut self.data[WIDTH * ly as usize..WIDTH * ly as usize + WIDTH]
    }
}

impl From<Color> for MemPixel {
    fn from(col: Color) -> Self {
        // MemPixel::from_grayscale(255 - 85 * col.into_raw())
        //     match col.into_val() {
        //         ColorVal::C00 => MemPixel::new(239, 255, 222, 255),
        //         ColorVal::C01 => MemPixel::new(173, 215, 148, 255),
        //         ColorVal::C10 => MemPixel::new(82, 146, 115, 255),
        //         ColorVal::C11 => MemPixel::new(24, 52, 66, 255),
        //     }
        match col.into_val() {
            ColorVal::C00 => MemPixel::new(239, 255, 222, 255),
            ColorVal::C01 => MemPixel::new(173, 215, 148, 255),
            ColorVal::C10 => MemPixel::new(82, 146, 115, 255),
            ColorVal::C11 => MemPixel::new(24, 52, 66, 255),
        }
    }
}

impl MemPixel {
    const CLEAR: MemPixel = MemPixel::new(0, 0, 0, 0);

    pub const fn new(r: u8, b: u8, g: u8, a: u8) -> MemPixel {
        MemPixel { r, g, b, a }
    }

    /// Private to avoid confusion about what kind of color values this takes (it is 0-255)
    const fn from_grayscale(grayscale: u8) -> MemPixel {
        MemPixel::new(grayscale, grayscale, grayscale, 0xff)
    }
}
