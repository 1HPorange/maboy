#[derive(Copy, Clone)]
pub struct Pixel([u8; 4]);

/// Frame located in main memory, not GPU memory
pub struct MemFrame(Box<[Pixel]>);

impl MemFrame {
    pub fn new() -> MemFrame {
        // Insane in the
        MemFrame(vec![Pixel::WHITE; 256 * 256].into_boxed_slice())
    }
}

impl Pixel {
    const WHITE: Pixel = Pixel([255, 255, 255, 255]);
}
