pub struct JoyPad {
    /// aka JOYP
    p1_reg: u8,
}

/// The write-mask of the P1 register
const P1_MASK: u8 = 0b_0011_0000;

impl JoyPad {
    pub fn new() -> JoyPad {
        JoyPad { p1_reg: 0xff }
    }

    pub fn read_p1(&self) -> u8 {
        self.p1_reg
    }

    pub fn write_p1(&mut self, val: u8) {
        self.p1_reg = (self.p1_reg & (!P1_MASK)) | (val & P1_MASK);
    }

    // TODO: Actual input
}
