use bitflags::*;

bitflags! {
    pub struct Interrupt: u8 {
        const V_BLANK =     0b_0000_0001;
        const LCD_STAT =    0b_0000_0010;
        const TIMER =       0b_0000_0100;
        const SERIAL =      0b_0000_1000;
        const JOYPAD =      0b_0001_0000;
    }
}
