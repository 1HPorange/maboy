use super::address::SerialReg;
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
            SerialReg::SC if val == 0x81 => print!("{}", self.sb_reg as char),
            _ => log::warn!("Unimplemented write to SC (Serial Port Control) register"),
        }
    }

    pub fn read_reg(&self, reg: SerialReg) -> u8 {
        match reg {
            SerialReg::SB => self.sb_reg,
            _ => unimplemented!(),
        }
    }
}
