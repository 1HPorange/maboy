//! Implementation of the Serial Port of your Game Boy, used for connecting
//! two Game Boys via a link cable. This module is almost completely unfinished;
//! It is only implemented up to a point where it doesn't crash any games.

use super::address::SerialReg;

/// Storage for the SB register
pub struct SerialPort {
    sb_reg: u8,
}

impl SerialPort {
    pub fn new() -> SerialPort {
        SerialPort { sb_reg: 0 }
    }

    pub fn write_reg(&mut self, reg: SerialReg, val: u8) {
        match reg {
            SerialReg::SB => self.sb_reg = val,
            SerialReg::SC => {
                if val == 0x81 {
                    // Blargg's test ROMs use this to output debug info; Uncomment
                    // to print it to the console in addition to the LCD. Useful if
                    // your LCD implementation is really broken.
                    // print!("{}", self.sb_reg as char)
                }

                // This is logged as `info`, not `warn`, because some games tend to spam it massively
                log::info!("Unimplemented write to SC (Serial Port Control) register");
            }
        }
    }

    pub fn read_reg(&self, reg: SerialReg) -> u8 {
        match reg {
            SerialReg::SB => self.sb_reg,
            SerialReg::SC => {
                log::warn!("Unimplemented read of SC register");
                0
            }
        }
    }
}
