//! Helper code to deal with register 0xFF41, the LCD Status register (LCDS)

use super::Mode;
use crate::util::BitOps;

/// Wrapper around the LCDS register with some utility methods
#[derive(Clone)]
pub struct LCDS(u8);

impl LCDS {
    pub fn new() -> LCDS {
        LCDS(0b1000_0000)
    }

    pub fn from_raw(reg: u8) -> LCDS {
        LCDS(0b1000_0000 | reg)
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

    pub fn lyc_equals_ly(&self) -> bool {
        self.0.bit(2)
    }

    pub fn set_lyc_equals_ly(&mut self, are_equal: bool) {
        self.0 = self.0.with_bit(2, are_equal)
    }

    /// Mode that the CPU reads, but not necessarily representing
    /// the current internal mode of the PPU
    pub fn mode(&self) -> Mode {
        unsafe { Mode::from_unchecked(self.0 & 0b11) }
    }

    pub fn write(&mut self, val: u8) {
        let write_mask = 0b_0111_1000;
        self.0 = (self.0 & (!write_mask)) + (val & write_mask);
    }

    pub fn read(&self) -> u8 {
        self.0
    }

    pub(super) fn set_mode(&mut self, ppu_mode: Mode) {
        let mode_mask = 0b_1111_1100;

        match ppu_mode {
            Mode::LCDOff => self.0 &= 0b_1111_1000,
            other => self.0 = (self.0 & mode_mask) + other as u8,
        }
    }

    /// True if the current mode and any of the requested LCD Stat interrupts
    /// coincide. This is used to implement LCD STAT blocking (a hardware bug).
    pub fn any_conditions_met(&self) -> bool {
        (self.ly_coincidence_interrupt() && self.0.bit(2))
            || match self.mode() {
                Mode::OAMSearch => self.oam_search_interrupt(),
                Mode::VBlank => self.v_blank_interrupt(),
                Mode::HBlank => self.h_blank_interrupt(),
                _ => false,
            }
    }
}
