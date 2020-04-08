#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct Pixel([u8; 4]);

/// Frame located in main memory, not GPU memory
pub struct MemFrame(Box<[Pixel]>);

impl MemFrame {
    pub fn new() -> MemFrame {
        // Insane in the
        MemFrame(vec![Pixel::WHITE; 256 * 256].into_boxed_slice())
    }

    pub fn line(&mut self, ly: u8) -> &mut [Pixel] {
        let start_idx = 256 * ly as usize;
        &mut self.0[start_idx..start_idx + 256]
    }

    pub fn data(&self) -> &[Pixel] {
        &self.0
    }
}

impl Pixel {
    const BLACK: Pixel = Pixel([0, 0, 0, 255]);
    const DGREY: Pixel = Pixel([96, 96, 96, 255]);
    const LGREY: Pixel = Pixel([192, 192, 192, 255]);
    const WHITE: Pixel = Pixel([255, 255, 255, 255]);

    pub unsafe fn from_2bit(val: u8) -> Pixel {
        match val & 0b11 {
            0b00 => Pixel::WHITE,
            0b01 => Pixel::LGREY,
            0b10 => Pixel::DGREY,
            0b11 => Pixel::BLACK,
            _ => unsafe { std::hint::unreachable_unchecked() },
        }
    }
}
