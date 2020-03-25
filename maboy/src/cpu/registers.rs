//! Contains code for storing and accessing CPU registers. This module uses
//! some nasty tricks for performance reasons, so be careful when changing it.
//! See [`Register`] for more info.

// TODO: ASSERT little endian and fail to compile otherwise

use bitflags::*;

/// All 8 and 16 bit registers of the Game Boy CPU implemented using a single
/// 12-byte array. This union uses optimizations that **only work on little-endian
/// architectures** and will silently fail if this requirement is not met. If you
/// are mad about this, open an issue and complain.
///
/// # Safety
/// There are some invariants expected to hold for this union to work correctly:
/// - The architecture of the executing system uses little-endian integers
/// - The lower four bits of the flags register (or the first byte of the backing array)
///   are never anything but 0
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

/// All 8-bit registers of the Game Boy CPU. The enum values represent the index
/// in the backing array of [`Registers`]
#[derive(Copy, Clone)]
pub enum R8 {
    A = 1,
    // F = 0, Should not be accessed via this enum; Use explicit accessor instead
    B = 3,
    C = 2,
    D = 5,
    E = 4,
    H = 7,
    L = 6,
}

/// All 16-bit registers of the Game Boy CPU. The enum values represent the index
/// in the backing array of [`Registers`]
#[derive(Copy, Clone)]
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
