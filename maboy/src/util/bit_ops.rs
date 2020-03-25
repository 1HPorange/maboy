/// Some common per-bit operations
pub trait BitOps: Copy {
    #[must_use]
    fn bit(self, bit: u8) -> bool;

    #[must_use]
    fn reset_bit(self, bit: u8) -> Self;

    #[must_use]
    fn set_bit(self, bit: u8) -> Self;

    #[must_use]
    fn with_bit(self, bit: u8, is_set: bool) -> Self;
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

            fn with_bit(self, bit: u8, is_set: bool) -> Self {
                if is_set {
                    self.set_bit(bit)
                } else {
                    self.reset_bit(bit)
                }
            }
        })*
    };
}

impl_bitops!(u8, u16);
