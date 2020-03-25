//! Implementation of (almost) every instruction on the Game Boy CPU.
//! This lives inside its own module to keep the root CPU module for being
//! too cluttered.

use super::operands::{Dst8, Src8};
use super::registers::*;
use super::CPU;
use crate::board::Board;
use crate::{debug::CpuEvt, util::BitOps};

pub fn ld8<B: Board, D: Dst8, S: Src8>(cpu: &mut CPU, board: &mut B, dst: D, src: S) {
    let val = src.read(cpu, board);
    dst.write(cpu, board, val);
}

pub fn ld_rr_d16<B: Board>(cpu: &mut CPU, board: &mut B, rr: R16) {
    *cpu.reg.r16_mut(rr) = cpu.read16i(board);
}

pub fn ld_a16_sp<B: Board>(cpu: &mut CPU, board: &mut B) {
    let addr = cpu.read16i(board);
    board.write16(addr, cpu.reg.sp());
}

pub fn ld_sp_hl<B: Board>(cpu: &mut CPU, board: &mut B) {
    *cpu.reg.sp_mut() = cpu.reg.hl();

    board.advance_mcycle();
}

pub fn ld_hl_sp_r8<B: Board>(cpu: &mut CPU, board: &mut B) {
    let offset = unsafe { std::mem::transmute::<u8, i8>(cpu.read8i(board)) } as i32;
    let sp = cpu.reg.sp() as i32;

    *cpu.reg.hl_mut() = (sp + offset) as u16;

    cpu.reg.flags_mut().remove(Flags::Z | Flags::N);
    cpu.reg
        .flags_mut()
        .set(Flags::H, (sp & 0xF) + (offset & 0xF) > 0xF);
    cpu.reg
        .flags_mut()
        .set(Flags::C, (sp & 0xFF) + (offset & 0xFF) > 0xFF);

    board.advance_mcycle();
}

pub fn rlca(cpu: &mut CPU) {
    let old = cpu.reg.r8(R8::A);

    *cpu.reg.r8_mut(R8::A) = old.rotate_left(1);

    cpu.reg.flags_mut().remove(Flags::Z | Flags::N | Flags::H);
    cpu.reg.flags_mut().set(Flags::C, old.bit(7));
}

pub fn rrca(cpu: &mut CPU) {
    let old = cpu.reg.r8(R8::A);

    *cpu.reg.r8_mut(R8::A) = old.rotate_right(1);

    cpu.reg.flags_mut().remove(Flags::Z | Flags::N | Flags::H);
    cpu.reg.flags_mut().set(Flags::C, old.bit(0));
}

pub fn rla(cpu: &mut CPU) {
    let old = cpu.reg.r8(R8::A);
    let new = (old << 1)
        + if cpu.reg.flags().contains(Flags::C) {
            1
        } else {
            0
        };

    *cpu.reg.r8_mut(R8::A) = new;

    cpu.reg.flags_mut().remove(Flags::Z | Flags::N | Flags::H);
    cpu.reg.flags_mut().set(Flags::C, old.bit(7));
}

pub fn rra(cpu: &mut CPU) {
    let old = cpu.reg.r8(R8::A);
    let new = (old >> 1)
        + if cpu.reg.flags().contains(Flags::C) {
            0b_1000_0000
        } else {
            0
        };

    *cpu.reg.r8_mut(R8::A) = new;

    cpu.reg.flags_mut().remove(Flags::Z | Flags::N | Flags::H);
    cpu.reg.flags_mut().set(Flags::C, old.bit(0));
}

pub fn cpl(cpu: &mut CPU) {
    *cpu.reg.r8_mut(R8::A) = !cpu.reg.r8(R8::A);
    cpu.reg.flags_mut().insert(Flags::N | Flags::H);
}

pub fn scf(cpu: &mut CPU) {
    cpu.reg.flags_mut().remove(Flags::N | Flags::H);
    cpu.reg.flags_mut().insert(Flags::C);
}

pub fn ccf(cpu: &mut CPU) {
    cpu.reg.flags_mut().remove(Flags::N | Flags::H);
    cpu.reg.flags_mut().toggle(Flags::C);
}

pub fn jr_cond<B: Board>(cpu: &mut CPU, board: &mut B, cond: bool) {
    let offset = cpu.read8i(board) as i8;

    if cond {
        *cpu.reg.pc_mut() = cpu.reg.pc().wrapping_add(offset as u16);

        board.push_cpu_evt(CpuEvt::TakeJmpTo(cpu.reg.pc()));

        board.advance_mcycle();
    } else {
        board.push_cpu_evt(CpuEvt::SkipJmpTo(cpu.reg.pc().wrapping_add(offset as u16)));
    }
}

pub fn jp_cond<B: Board>(cpu: &mut CPU, board: &mut B, cond: bool) {
    let target = cpu.read16i(board);

    if cond {
        *cpu.reg.pc_mut() = target;

        board.push_cpu_evt(CpuEvt::TakeJmpTo(target));

        board.advance_mcycle();
    } else {
        board.push_cpu_evt(CpuEvt::SkipJmpTo(target));
    }
}

pub fn jp_hl<B: Board>(cpu: &mut CPU, board: &mut B) {
    *cpu.reg.pc_mut() = cpu.reg.hl();

    board.push_cpu_evt(CpuEvt::TakeJmpTo(cpu.reg.hl()));
}

pub fn pop<B: Board>(cpu: &mut CPU, board: &mut B, rr: R16) {
    *cpu.reg.r16_mut(rr) = board.read16(cpu.reg.sp());
    *cpu.reg.sp_mut() = cpu.reg.sp().wrapping_add(2);
}

pub fn pop_af<B: Board>(cpu: &mut CPU, board: &mut B) {
    // The lower four bits of the flag register will always be 0, no matter
    // what you pop into them
    *cpu.reg.r16_mut(R16::AF) = board.read16(cpu.reg.sp()) & 0xFFF0;
    *cpu.reg.sp_mut() = cpu.reg.sp().wrapping_add(2);
}

pub fn push<B: Board>(cpu: &mut CPU, board: &mut B, rr: R16) {
    *cpu.reg.sp_mut() = cpu.reg.sp().wrapping_sub(2);
    board.advance_mcycle();
    board.write16(cpu.reg.r16(R16::SP), cpu.reg.r16(rr));
}

pub fn rst<B: Board>(cpu: &mut CPU, board: &mut B, target: u16) {
    push(cpu, board, R16::PC);
    *cpu.reg.pc_mut() = target;

    board.push_cpu_evt(CpuEvt::TakeJmpTo(target));
}

/// Due to timing differences, this function CANNOT be expressed as ret_cond(..., true)!!!
pub fn ret<B: Board>(cpu: &mut CPU, board: &mut B, enable_ime: bool) {
    pop(cpu, board, R16::PC);

    if enable_ime {
        cpu.set_ime(board, true);
    }

    board.push_cpu_evt(CpuEvt::TakeJmpTo(cpu.reg.pc()));

    board.advance_mcycle();
}

pub fn ret_cond<B: Board>(cpu: &mut CPU, board: &mut B, cond: bool) {
    board.advance_mcycle();

    if cond {
        // This call already pushes the debug event, so no need for us to do that
        ret(cpu, board, false);
    } else {
        // It's really important that this is an *instant* read, since it's only a debug thingy
        board.push_cpu_evt(CpuEvt::SkipJmpTo(board.read16_instant(cpu.reg.sp())));
    }
}

pub fn call_cond<B: Board>(cpu: &mut CPU, board: &mut B, cond: bool) {
    let target = cpu.read16i(board);

    if cond {
        push(cpu, board, R16::PC);
        *cpu.reg.pc_mut() = target;

        board.push_cpu_evt(CpuEvt::TakeJmpTo(target));
    } else {
        board.push_cpu_evt(CpuEvt::SkipJmpTo(target));
    }
}

pub fn add_hl_rr<B: Board>(cpu: &mut CPU, board: &mut B, rr: R16) {
    let old = cpu.reg.hl();
    let addend = cpu.reg.r16(rr);

    let (new, carry) = old.overflowing_add(addend);

    *cpu.reg.hl_mut() = new;

    cpu.reg.flags_mut().remove(Flags::N);
    cpu.reg
        .flags_mut()
        .set(Flags::H, (old & 0x0FFF) + (addend & 0x0FFF) > 0x0FFF);
    cpu.reg.flags_mut().set(Flags::C, carry);

    board.advance_mcycle();
}

pub fn add_sp_r8<B: Board>(cpu: &mut CPU, board: &mut B) {
    let offset = unsafe { std::mem::transmute::<u8, i8>(cpu.read8i(board)) } as i32;
    let old = cpu.reg.sp() as i32;

    *cpu.reg.sp_mut() = (old + offset) as u16;

    cpu.reg.flags_mut().remove(Flags::Z | Flags::N);
    cpu.reg
        .flags_mut()
        .set(Flags::H, (old & 0xF) + (offset & 0xF) > 0xF);
    cpu.reg
        .flags_mut()
        .set(Flags::C, (old & 0xFF) + (offset & 0xFF) > 0xFF);

    board.advance_mcycle();
    board.advance_mcycle();
}

pub fn inc_rr<B: Board>(cpu: &mut CPU, board: &mut B, rr: R16) {
    *cpu.reg.r16_mut(rr) = cpu.reg.r16(rr).wrapping_add(1);
    board.advance_mcycle();
}

pub fn dec_rr<B: Board>(cpu: &mut CPU, board: &mut B, rr: R16) {
    *cpu.reg.r16_mut(rr) = cpu.reg.r16(rr).wrapping_sub(1);
    board.advance_mcycle();
}

pub fn inc8<B: Board, T: Src8 + Dst8 + Copy>(cpu: &mut CPU, board: &mut B, target: T) {
    let old = target.read(cpu, board);
    let new = old.wrapping_add(1);

    target.write(cpu, board, new);

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().remove(Flags::N);
    cpu.reg.flags_mut().set(Flags::H, (old & 0x0f) == 0x0f);
}

pub fn dec8<B: Board, T: Src8 + Dst8 + Copy>(cpu: &mut CPU, board: &mut B, target: T) {
    let old = target.read(cpu, board);
    let new = old.wrapping_sub(1);

    target.write(cpu, board, new);

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().insert(Flags::N);
    cpu.reg.flags_mut().set(Flags::H, (new & 0x0f) == 0x0f);
}

pub fn add8<B: Board, S: Src8>(cpu: &mut CPU, board: &mut B, src: S) {
    let old = cpu.reg.r8(R8::A);
    let addend = src.read(cpu, board);
    let (new, carry) = old.overflowing_add(addend);

    *cpu.reg.r8_mut(R8::A) = new;

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().remove(Flags::N);
    cpu.reg
        .flags_mut()
        .set(Flags::H, (old & 0x0f) + (addend & 0x0f) > 0x0f);
    cpu.reg.flags_mut().set(Flags::C, carry);
}

pub fn adc8<B: Board, S: Src8>(cpu: &mut CPU, board: &mut B, src: S) {
    let old = cpu.reg.r8(R8::A) as u16;
    let addend = src.read(cpu, board) as u16;
    let carry_val = if cpu.reg.flags().contains(Flags::C) {
        1
    } else {
        0
    };
    let sum = old + addend + carry_val;
    let new = (sum & 0xff) as u8;

    *cpu.reg.r8_mut(R8::A) = new;

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg
        .flags_mut()
        .set(Flags::H, (old & 0x0f) + (addend & 0x0f) + carry_val > 0x0f);
    cpu.reg.flags_mut().set(Flags::C, sum > 0xff);
    cpu.reg.flags_mut().remove(Flags::N);
}

pub fn sub8<B: Board, S: Src8>(cpu: &mut CPU, board: &mut B, src: S) {
    *cpu.reg.r8_mut(R8::A) = cp8(cpu, board, src);
}

pub fn sbc8<B: Board, S: Src8>(cpu: &mut CPU, board: &mut B, src: S) {
    // The bit magic gets a bit easier when we convert stuff to i16
    let old = cpu.reg.r8(R8::A) as i16;
    let subtrahend = src.read(cpu, board) as i16;
    let carry_val = if cpu.reg.flags().contains(Flags::C) {
        1
    } else {
        0
    };

    let new = old - subtrahend - carry_val;

    *cpu.reg.r8_mut(R8::A) = new as u8;

    cpu.reg.flags_mut().set(Flags::Z, new & 0xff == 0);
    cpu.reg.flags_mut().insert(Flags::N);
    cpu.reg
        .flags_mut()
        .set(Flags::H, (old & 0xf) < (subtrahend & 0xf) + carry_val);
    cpu.reg.flags_mut().set(Flags::C, new < 0);
}

pub fn cp8<B: Board, S: Src8>(cpu: &mut CPU, board: &mut B, src: S) -> u8 {
    let old = cpu.reg.r8(R8::A);
    let subtrahend = src.read(cpu, board);

    let (new, carry) = old.overflowing_sub(subtrahend);

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().insert(Flags::N);
    cpu.reg
        .flags_mut()
        .set(Flags::H, (old & 0x0f) < (subtrahend & 0x0f));
    cpu.reg.flags_mut().set(Flags::C, carry);

    new
}

pub fn and8<B: Board, S: Src8>(cpu: &mut CPU, board: &mut B, src: S) {
    let new = cpu.reg.r8(R8::A) & src.read(cpu, board);

    *cpu.reg.r8_mut(R8::A) = new;

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().remove(Flags::N | Flags::C);
    cpu.reg.flags_mut().insert(Flags::H);
}

pub fn xor8<B: Board, S: Src8>(cpu: &mut CPU, board: &mut B, src: S) {
    let new = cpu.reg.r8(R8::A) ^ src.read(cpu, board);

    *cpu.reg.r8_mut(R8::A) = new;

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().remove(Flags::N | Flags::H | Flags::C);
}

pub fn or8<B: Board, S: Src8>(cpu: &mut CPU, board: &mut B, src: S) {
    let new = cpu.reg.r8(R8::A) | src.read(cpu, board);

    *cpu.reg.r8_mut(R8::A) = new;

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().remove(Flags::N | Flags::H | Flags::C);
}

// CB prefixed Instructions

pub fn rlc<B: Board, T: Src8 + Dst8 + Copy>(cpu: &mut CPU, board: &mut B, target: T) {
    let old = target.read(cpu, board);
    target.write(cpu, board, old.rotate_left(1));

    cpu.reg.flags_mut().set(Flags::Z, old == 0);
    cpu.reg.flags_mut().remove(Flags::N | Flags::H);
    cpu.reg.flags_mut().set(Flags::C, old.bit(7));
}

pub fn rrc<B: Board, T: Src8 + Dst8 + Copy>(cpu: &mut CPU, board: &mut B, target: T) {
    let old = target.read(cpu, board);
    target.write(cpu, board, old.rotate_right(1));

    cpu.reg.flags_mut().set(Flags::Z, old == 0);
    cpu.reg.flags_mut().remove(Flags::N | Flags::H);
    cpu.reg.flags_mut().set(Flags::C, old.bit(0))
}

pub fn rl<B: Board, T: Src8 + Dst8 + Copy>(cpu: &mut CPU, board: &mut B, target: T) {
    let old = target.read(cpu, board);
    let new = (old << 1)
        + if cpu.reg.flags().contains(Flags::C) {
            1
        } else {
            0
        };

    target.write(cpu, board, new);

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().remove(Flags::N | Flags::H);
    cpu.reg.flags_mut().set(Flags::C, old.bit(7));
}

pub fn rr<B: Board, T: Src8 + Dst8 + Copy>(cpu: &mut CPU, board: &mut B, target: T) {
    let old = target.read(cpu, board);
    let new = (old >> 1)
        + if cpu.reg.flags().contains(Flags::C) {
            0b_1000_0000
        } else {
            0
        };

    target.write(cpu, board, new);

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().remove(Flags::N | Flags::H);
    cpu.reg.flags_mut().set(Flags::C, old.bit(0));
}

pub fn sla<B: Board, T: Src8 + Dst8 + Copy>(cpu: &mut CPU, board: &mut B, target: T) {
    let old = target.read(cpu, board);
    let new = old << 1;

    target.write(cpu, board, new);

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().remove(Flags::N | Flags::H);
    cpu.reg.flags_mut().set(Flags::C, old.bit(7));
}

pub fn sra<B: Board, T: Src8 + Dst8 + Copy>(cpu: &mut CPU, board: &mut B, target: T) {
    let old = target.read(cpu, board);
    let new = (old >> 1) | (old & 0b_1000_0000);

    target.write(cpu, board, new);

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().remove(Flags::N | Flags::H);
    cpu.reg.flags_mut().set(Flags::C, old.bit(0));
}

pub fn swap<B: Board, T: Src8 + Dst8 + Copy>(cpu: &mut CPU, board: &mut B, target: T) {
    let old = target.read(cpu, board);
    target.write(cpu, board, (old >> 4) + (old << 4));

    cpu.reg.flags_mut().set(Flags::Z, old == 0);
    cpu.reg.flags_mut().remove(Flags::N | Flags::H | Flags::C);
}

pub fn srl<B: Board, T: Src8 + Dst8 + Copy>(cpu: &mut CPU, board: &mut B, target: T) {
    let old = target.read(cpu, board);
    let new = old >> 1;

    target.write(cpu, board, new);

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().remove(Flags::N | Flags::H);
    cpu.reg.flags_mut().set(Flags::C, old.bit(0));
}

pub fn bit<B: Board, S: Src8>(cpu: &mut CPU, board: &mut B, bit: u8, src: S) {
    let bit_set = src.read(cpu, board).bit(bit);
    cpu.reg.flags_mut().set(Flags::Z, !bit_set);
    cpu.reg.flags_mut().remove(Flags::N);
    cpu.reg.flags_mut().insert(Flags::H);
}

pub fn res<B: Board, T: Src8 + Dst8 + Copy>(cpu: &mut CPU, board: &mut B, bit: u8, target: T) {
    let new = target.read(cpu, board).reset_bit(bit);
    target.write(cpu, board, new);
}

pub fn set<B: Board, T: Src8 + Dst8 + Copy>(cpu: &mut CPU, board: &mut B, bit: u8, target: T) {
    let new = target.read(cpu, board).set_bit(bit);
    target.write(cpu, board, new);
}

pub fn daa(cpu: &mut CPU) {
    // DAA is kind of infamous for having complicated behaviour
    // This is why I took the source code from https://forums.nesdev.com/viewtopic.php?t=15944

    let mut new = cpu.reg.r8(R8::A);

    // note: assumes a is a uint8_t and wraps from 0xff to 0
    if !cpu.reg.flags().contains(Flags::N) {
        // after an addition, adjust if (half-)carry occurred or if result is out of bounds
        if cpu.reg.flags().contains(Flags::C) || new > 0x99 {
            new = new.wrapping_add(0x60);
            cpu.reg.flags_mut().insert(Flags::C);
        }
        if cpu.reg.flags().contains(Flags::H) || (new & 0x0f) > 0x09 {
            new = new.wrapping_add(0x6);
        }
    } else {
        // after a subtraction, only adjust if (half-)carry occurred
        if cpu.reg.flags().contains(Flags::C) {
            new = new.wrapping_sub(0x60);
        }
        if cpu.reg.flags().contains(Flags::H) {
            new = new.wrapping_sub(0x6);
        }
    };

    *cpu.reg.r8_mut(R8::A) = new;

    // these flags are always updated
    cpu.reg.flags_mut().set(Flags::Z, new == 0); // the usual z flag
    cpu.reg.flags_mut().remove(Flags::H); // h flag is always cleared
}
