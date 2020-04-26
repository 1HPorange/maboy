pub trait BitOps {
    fn bit(self, bit: u8) -> bool;
    fn reset_bit(self, bit: u8) -> Self;
    fn set_bit(self, bit: u8) -> Self;
}

macro_rules! impl_bitops {
    ($($type:ty),*) => {
        $(impl BitOps for $type {
            fn bit(self, bit: u8) -> bool {
                (self >> bit) & 1 != 0
            }

            fn reset_bit(self, bit: u8) -> Self {
                self & (!(1 << bit))
            }

            fn set_bit(self, bit: u8) -> Self {
                self | (1 << bit)
            }
        })*
    };
}

impl_bitops!(u8, u16);
