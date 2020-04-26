use bitflags::*;
use std::ops::{Index, IndexMut};

#[repr(C)]
pub union Registers {
    flags: Flags,
    backing: [u8; 12],
}

bitflags! {
    pub struct Flags: u8 {
        const Z = 0b_1000_0000;
        const N = 0b_0100_0000;
        const H = 0b_0010_0000;
        const C = 0b_0001_0000;
    }
}

// TODO: ASSERT little endian, because sneaky kind of indexing only works there

#[derive(Copy, Clone)]
#[repr(C)]
pub enum R8 {
    A = 1,
    // F = 0,
    B = 3,
    C = 2,
    D = 5,
    E = 4,
    H = 7,
    L = 6,
}

#[derive(Copy, Clone)]
#[repr(C)]
pub enum R16 {
    AF = 0,
    BC = 2,
    DE = 4,
    HL = 6,
    SP = 8,
    PC = 10,
}

impl Registers {
    pub fn new() -> Registers {
        Registers { backing: [0; 12] }
    }

    pub fn r8(&self, r: R8) -> u8 {
        unsafe { self.backing[r as usize] }
    }

    pub fn r8_mut(&mut self, r: R8) -> &mut u8 {
        unsafe { &mut self.backing[r as usize] }
    }

    pub fn r16(&self, rr: R16) -> u16 {
        unsafe { *std::mem::transmute::<&u8, &u16>(&self.backing[rr as usize]) }
    }

    pub fn r16_mut(&mut self, rr: R16) -> &mut u16 {
        unsafe { std::mem::transmute::<&mut u8, &mut u16>(&mut self.backing[rr as usize]) }
    }

    pub fn flags(&self) -> &Flags {
        unsafe { &self.flags }
    }

    pub fn flags_mut(&mut self) -> &mut Flags {
        unsafe { &mut self.flags }
    }

    // Quick Access methods

    pub fn hl(&self) -> u16 {
        self.r16(R16::HL)
    }

    pub fn hl_mut(&mut self) -> &mut u16 {
        self.r16_mut(R16::HL)
    }

    pub fn sp(&self) -> u16 {
        self.r16(R16::SP)
    }

    pub fn sp_mut(&mut self) -> &mut u16 {
        self.r16_mut(R16::SP)
    }

    pub fn pc(&self) -> u16 {
        self.r16(R16::PC)
    }

    pub fn pc_mut(&mut self) -> &mut u16 {
        self.r16_mut(R16::PC)
    }
}

impl Index<Flags> for Registers {
    type Output = bool;

    fn index(&self, flags: Flags) -> &Self::Output {
        &self.flags().contains(flags)
    }
}
