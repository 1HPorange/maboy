//! Contains code for storing and accessing CPU registers.
//! See [`Registers`] for more info.

use bitflags::*;

#[repr(C)]
#[derive(Default)]
pub struct Registers {
    pub a: u8,
    pub flags: Flags,
    pub bc: u16,
    pub de: u16,
    pub hl: u16,
    pub sp: u16,
    pub pc: u16,
}

bitflags! {
    #[derive(Default)]
    pub struct Flags: u8 {
        const Z = 0b_1000_0000;
        const N = 0b_0100_0000;
        const H = 0b_0010_0000;
        const C = 0b_0001_0000;
    }
}

#[derive(Copy, Clone)]
pub enum R8 {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
}

/// All 16-bit registers of the Game Boy CPU. The enum values represent the index
/// in the backing array of [`Registers`]
#[derive(Copy, Clone)]
pub enum R16 {
    AF,
    BC,
    DE,
    HL,
    SP,
    PC,
}

impl Registers {
    pub fn new() -> Registers {
        Default::default()
    }

    pub fn get_r8(&self, r: R8) -> u8 {
        match r {
            R8::A => self.a,
            R8::B => self.bc.to_le_bytes()[1],
            R8::C => self.bc.to_le_bytes()[0],
            R8::D => self.de.to_le_bytes()[1],
            R8::E => self.de.to_le_bytes()[0],
            R8::H => self.hl.to_le_bytes()[1],
            R8::L => self.hl.to_le_bytes()[0],
        }
    }

    pub fn set_r8(&mut self, r: R8, val: u8) {
        let r16 = match r {
            R8::A => {
                self.a = val;
                return;
            }
            R8::B => &mut self.bc,
            R8::C => &mut self.bc,
            R8::D => &mut self.de,
            R8::E => &mut self.de,
            R8::H => &mut self.hl,
            R8::L => &mut self.hl,
        };

        let mut bytes = r16.to_le_bytes();

        match r {
            R8::B => bytes[1] = val,
            R8::C => bytes[0] = val,
            R8::D => bytes[1] = val,
            R8::E => bytes[0] = val,
            R8::H => bytes[1] = val,
            R8::L => bytes[0] = val,
            _ => unreachable!(),
        }

        *r16 = u16::from_le_bytes(bytes);
    }

    pub fn get_r16(&self, rr: R16) -> u16 {
        match rr {
            R16::AF => u16::from_le_bytes([self.flags.bits(), self.a]),
            R16::BC => self.bc,
            R16::DE => self.de,
            R16::HL => self.hl,
            R16::SP => self.sp,
            R16::PC => self.pc,
        }
    }

    pub fn set_r16(&mut self, rr: R16, val: u16) {
        match rr {
            R16::AF => {
                let bytes = val.to_le_bytes();
                self.flags = Flags::from_bits_truncate(bytes[0]);
                self.a = bytes[1];
            }
            R16::BC => self.bc = val,
            R16::DE => self.de = val,
            R16::HL => self.hl = val,
            R16::SP => self.sp = val,
            R16::PC => self.pc = val,
        }
    }
}
