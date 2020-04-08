pub trait Bit {
    fn bit(self, n: u8) -> bool;
}

impl Bit for u8 {
    fn bit(self, n: u8) -> bool {
        (self >> n) & 0b1 != 0
    }
}
