//! To avoid repetitive code in the [`super::execute`] module, this module provides
//! the [`Src8`] and [`Dst8`] traits, which are used to generalize over source and
//! destination locations in memory without requiring the caller to think about
//! how many cycles such a memory access would consume.
//!
//! # Example
//! ```
//! use super::registers::R8;
//!
//! let x = R8::A.read(&mut cpu, &mut board); // Doesn't consume a cycle
//! let y = Imm8.read(&mut cpu, &mut board); // Consumes a single cycle
//! ```

use super::registers::{R16, R8};
use super::CPU;
use crate::board::Board;

/// The HL register offers optional "free" INC/DEC on HL after (HL) is resolved.
/// By providing one of the enum variants as [`Src8`] or [`Dst8`], we can automate this.
pub enum HlOperand {
    /// Increments HL after the lookup (HL++)
    HLi,

    /// Decrements HL after the lookup (HL--)
    HLd,
}

/// Reads an 8 bit value (from memory or a CPU register), consuming
/// the correct amount of cycles in the process.
pub trait Src8 {
    fn read<B: Board>(self, cpu: &mut CPU, board: &mut B) -> u8;
}

/// Writes an 8 bit value (to memory or a CPU register), consuming
/// the correct amount of cycles in the process.
pub trait Dst8 {
    fn write<B: Board>(self, cpu: &mut CPU, board: &mut B, val: u8);
}

/// Passing this as source reads an immediate operand from (PC), then increases PC.
pub struct Imm8;

impl Src8 for Imm8 {
    fn read<B: Board>(self, cpu: &mut CPU, board: &mut B) -> u8 {
        cpu.read8i(board)
    }
}

/// Some operations save a byte by assuming the upper byte of the src/dst address
/// of the operation to be 0xFF (0xFFxx), with the lower byte provided via this operand.
pub enum HighRamOperand {
    Imm8,
    C,
}

impl Src8 for HighRamOperand {
    fn read<B: Board>(self, cpu: &mut CPU, board: &mut B) -> u8 {
        let offset = match self {
            HighRamOperand::Imm8 => cpu.read8i(board) as u16,
            HighRamOperand::C => cpu.reg.get_r8(R8::C) as u16,
        };

        board.read8(offset.wrapping_add(0xFF00))
    }
}

impl Dst8 for HighRamOperand {
    fn write<B: Board>(self, cpu: &mut CPU, board: &mut B, val: u8) {
        let offset = match self {
            HighRamOperand::Imm8 => cpu.read8i(board) as u16,
            HighRamOperand::C => cpu.reg.get_r8(R8::C) as u16,
        };

        board.write8(offset.wrapping_add(0xFF00), val);
    }
}

impl Src8 for HlOperand {
    fn read<B: Board>(self, cpu: &mut CPU, board: &mut B) -> u8 {
        match self {
            HlOperand::HLi => {
                let result = board.read8(cpu.reg.hl);
                cpu.reg.hl = cpu.reg.hl.wrapping_add(1);
                result
            }
            HlOperand::HLd => {
                let result = board.read8(cpu.reg.hl);
                cpu.reg.hl = cpu.reg.hl.wrapping_sub(1);
                result
            }
        }
    }
}

impl Dst8 for HlOperand {
    fn write<B: Board>(self, cpu: &mut CPU, board: &mut B, val: u8) {
        match self {
            HlOperand::HLi => {
                board.write8(cpu.reg.hl, val);
                cpu.reg.hl = cpu.reg.hl.wrapping_add(1);
            }
            HlOperand::HLd => {
                board.write8(cpu.reg.hl, val);
                cpu.reg.hl = cpu.reg.hl.wrapping_sub(1);
            }
        }
    }
}

impl Src8 for R8 {
    fn read<B: Board>(self, cpu: &mut CPU, _board: &mut B) -> u8 {
        cpu.reg.get_r8(self)
    }
}

impl Dst8 for R8 {
    fn write<B: Board>(self, cpu: &mut CPU, _board: &mut B, val: u8) {
        cpu.reg.set_r8(self, val);
    }
}

impl Src8 for R16 {
    fn read<B: Board>(self, cpu: &mut CPU, board: &mut B) -> u8 {
        board.read8(cpu.reg.get_r16(self))
    }
}

impl Dst8 for R16 {
    fn write<B: Board>(self, cpu: &mut CPU, board: &mut B, val: u8) {
        board.write8(cpu.reg.get_r16(self), val);
    }
}

pub struct ImmAddr;

impl Src8 for ImmAddr {
    fn read<B: Board>(self, cpu: &mut CPU, board: &mut B) -> u8 {
        let addr = cpu.read16i(board);
        board.read8(addr)
    }
}

impl Dst8 for ImmAddr {
    fn write<B: Board>(self, cpu: &mut CPU, board: &mut B, val: u8) {
        let addr = cpu.read16i(board);
        board.write8(addr, val);
    }
}
