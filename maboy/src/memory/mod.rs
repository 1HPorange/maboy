//! Contains something akin to the Game Boy's memory management unit (MMU),
//! although this implementation is not based on any actual hardware. It
//! just groups functionality that is required for the CPU to access memory
//! correctly.

mod internal_mem;

use super::cartridge::Cartridge;
use crate::address::{CRomAddr, MemAddr};

pub use internal_mem::InternalMem;

/// Contains all memory that is not otherwise explicitly handled by any module
/// (like the PPU).
pub struct Memory<C> {
    internal: InternalMem,
    cartridge: C,
    boot_rom_mapped: bool,
}

impl<C: Cartridge> Memory<C> {
    pub fn new(internal_mem: InternalMem, cartridge: C) -> Memory<C> {
        Memory {
            internal: internal_mem,
            cartridge: cartridge,
            boot_rom_mapped: true,
        }
    }

    pub fn read8(&self, addr: MemAddr) -> u8 {
        use CRomAddr::*;
        use MemAddr::*;

        match addr {
            CROM(CROM0(addr)) if self.boot_rom_mapped && addr < 0x100 => BOOT_ROM[addr as usize],
            CROM(addr) => self.cartridge.read_rom(addr),
            CRAM(addr) => self.cartridge.read_cram(addr),
            WRAM(addr) => self.internal.wram[addr as usize],
            ECHO(addr) => self.internal.wram[addr as usize],
            HRAM(addr) => self.internal.hram[addr as usize],
        }
    }

    pub fn write8(&mut self, addr: MemAddr, val: u8) {
        use MemAddr::*;

        match addr {
            CROM(addr) => self.cartridge.write_rom(addr, val),
            CRAM(addr) => self.cartridge.write_cram(addr, val),
            WRAM(addr) => self.internal.wram[addr as usize] = val,
            ECHO(addr) => self.internal.wram[addr as usize] = val,
            HRAM(addr) => self.internal.hram[addr as usize] = val,
        }
    }

    /// The boot rom writes 1 to 0xff50 to disable itself after completing
    pub fn write_ff50(&mut self, val: u8) {
        if val == 1 {
            self.boot_rom_mapped = false;
        } else {
            unimplemented!("Don't know what happens here")
        }
    }
}

/// When the Game Boy boots up, these 256 bytes are mapped to the lowest 256 addresses instead of
/// the corresponding bytes in the cartridge ROM. This re-mapping is disabled after this boot rom
/// has successfully finished executing (see [`Memory::write_ff50`]).
const BOOT_ROM: [u8; 256] = [
    0x31, 0xFE, 0xFF, 0xAF, 0x21, 0xFF, 0x9F, 0x32, 0xCB, 0x7C, 0x20, 0xFB, 0x21, 0x26, 0xFF, 0x0E,
    0x11, 0x3E, 0x80, 0x32, 0xE2, 0x0C, 0x3E, 0xF3, 0xE2, 0x32, 0x3E, 0x77, 0x77, 0x3E, 0xFC, 0xE0,
    0x47, 0x11, 0x04, 0x01, 0x21, 0x10, 0x80, 0x1A, 0xCD, 0x95, 0x00, 0xCD, 0x96, 0x00, 0x13, 0x7B,
    0xFE, 0x34, 0x20, 0xF3, 0x11, 0xD8, 0x00, 0x06, 0x08, 0x1A, 0x13, 0x22, 0x23, 0x05, 0x20, 0xF9,
    0x3E, 0x19, 0xEA, 0x10, 0x99, 0x21, 0x2F, 0x99, 0x0E, 0x0C, 0x3D, 0x28, 0x08, 0x32, 0x0D, 0x20,
    0xF9, 0x2E, 0x0F, 0x18, 0xF3, 0x67, 0x3E, 0x64, 0x57, 0xE0, 0x42, 0x3E, 0x91, 0xE0, 0x40, 0x04,
    0x1E, 0x02, 0x0E, 0x0C, 0xF0, 0x44, 0xFE, 0x90, 0x20, 0xFA, 0x0D, 0x20, 0xF7, 0x1D, 0x20, 0xF2,
    0x0E, 0x13, 0x24, 0x7C, 0x1E, 0x83, 0xFE, 0x62, 0x28, 0x06, 0x1E, 0xC1, 0xFE, 0x64, 0x20, 0x06,
    0x7B, 0xE2, 0x0C, 0x3E, 0x87, 0xF2, 0xF0, 0x42, 0x90, 0xE0, 0x42, 0x15, 0x20, 0xD2, 0x05, 0x20,
    0x4F, 0x16, 0x20, 0x18, 0xCB, 0x4F, 0x06, 0x04, 0xC5, 0xCB, 0x11, 0x17, 0xC1, 0xCB, 0x11, 0x17,
    0x05, 0x20, 0xF5, 0x22, 0x23, 0x22, 0x23, 0xC9, 0xCE, 0xED, 0x66, 0x66, 0xCC, 0x0D, 0x00, 0x0B,
    0x03, 0x73, 0x00, 0x83, 0x00, 0x0C, 0x00, 0x0D, 0x00, 0x08, 0x11, 0x1F, 0x88, 0x89, 0x00, 0x0E,
    0xDC, 0xCC, 0x6E, 0xE6, 0xDD, 0xDD, 0xD9, 0x99, 0xBB, 0xBB, 0x67, 0x63, 0x6E, 0x0E, 0xEC, 0xCC,
    0xDD, 0xDC, 0x99, 0x9F, 0xBB, 0xB9, 0x33, 0x3E, 0x3c, 0x42, 0xB9, 0xA5, 0xB9, 0xA5, 0x42, 0x4C,
    0x21, 0x04, 0x01, 0x11, 0xA8, 0x00, 0x1A, 0x13, 0xBE, 0x20, 0xFE, 0x23, 0x7D, 0xFE, 0x34, 0x20,
    0xF5, 0x06, 0x19, 0x78, 0x86, 0x23, 0x05, 0x20, 0xFB, 0x86, 0x20, 0xFE, 0x3E, 0x01, 0xE0, 0x50,
];
