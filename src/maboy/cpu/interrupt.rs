use bitflags::*;
#[repr(u8)]
pub enum Interrupt {
    VBlank = 1 << 0,
    LcdState = 1 << 1,
    Timer = 1 << 2,
    Serial = 1 << 3,
    Joypad = 1 << 4,
}
