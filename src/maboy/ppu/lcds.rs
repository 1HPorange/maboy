use super::Mode;
use crate::maboy::util::BitOps;

#[derive(Copy, Clone)]
pub struct LCDS(u8);

impl LCDS {
    pub fn new() -> LCDS {
        LCDS(0b1000_0000)
    }

    pub fn ly_coincidence_interrupt(&self) -> bool {
        self.0.bit(6)
    }

    pub fn oam_search_interrupt(&self) -> bool {
        self.0.bit(5)
    }

    pub fn v_blank_interrupt(&self) -> bool {
        self.0.bit(4)
    }

    pub fn h_blank_interrupt(&self) -> bool {
        self.0.bit(3)
    }

    pub fn write(&mut self, val: u8) {
        let write_mask = 0b_0111_1000;
        self.0 = (self.0 & (!write_mask)) + (val & write_mask);
    }

    pub fn read(&self) -> u8 {
        self.0
    }

    pub(super) fn set_mode(&mut self, mode: Mode) {
        let mode_mask = 0b_1111_1100;

        match mode {
            Mode::LCDOff => self.0 &= 0b_1111_1000,
            Mode::HBlank(_) => self.0 &= mode_mask,
            Mode::VBlank(_) => self.0 = (self.0 & mode_mask) + 1,
            Mode::OAMSearch(_) => self.0 = (self.0 & mode_mask) + 2,
            Mode::PixelTransfer(_) => self.0 = (self.0 & mode_mask) + 3,
        }
    }

    pub fn set_lyc_equals_ly(&mut self, are_equal: bool) {
        self.0 = self.0.set_bit_to(2, are_equal)
    }
}
