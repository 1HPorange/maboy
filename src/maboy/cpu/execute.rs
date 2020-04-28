use super::operands::{Dst8, Src8};
use super::registers::*;
use super::CPU;
use crate::maboy::board::Board;
use crate::maboy::memory::cartridge_mem::CartridgeRam;
use crate::maboy::util::BitOps;

pub fn ld8<CRAM: CartridgeRam, D: Dst8, S: Src8>(
    cpu: &mut CPU,
    board: &mut Board<CRAM>,
    dst: D,
    src: S,
) {
    let val = src.read(cpu, board);
    dst.write(cpu, board, val);
}

pub fn ld_rr_d16<CRAM: CartridgeRam>(cpu: &mut CPU, board: &mut Board<CRAM>, rr: R16) {
    *cpu.reg.r16_mut(rr) = cpu.read16i(board);
}

pub fn ld_a16_sp<CRAM: CartridgeRam>(cpu: &mut CPU, board: &mut Board<CRAM>) {
    let addr = cpu.read16i(board);
    board.write16(addr, cpu.reg.sp());
}

pub fn ld_sp_hl<CRAM: CartridgeRam>(cpu: &mut CPU, board: &mut Board<CRAM>) {
    *cpu.reg.sp_mut() = cpu.reg.hl();

    board.advance_mcycle();
}

pub fn ld_hl_sp_r8<CRAM: CartridgeRam>(cpu: &mut CPU, board: &mut Board<CRAM>) {
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
}

pub fn scf(cpu: &mut CPU) {
    cpu.reg.flags_mut().remove(Flags::N | Flags::H);
    cpu.reg.flags_mut().insert(Flags::C);
}

pub fn ccf(cpu: &mut CPU) {
    cpu.reg.flags_mut().toggle(Flags::C);
}

pub fn jr_cond<CRAM: CartridgeRam>(cpu: &mut CPU, board: &mut Board<CRAM>, cond: bool) {
    let offset: i8 = unsafe { std::mem::transmute(cpu.read8i(board)) };

    if cond {
        // TODO: Figure out why the heck this cast works
        *cpu.reg.pc_mut() = cpu.reg.pc().wrapping_add(offset as u16);

        board.advance_mcycle();
    }
}

pub fn jp_cond<CRAM: CartridgeRam>(cpu: &mut CPU, board: &mut Board<CRAM>, cond: bool) {
    let target = cpu.read16i(board);

    if cond {
        *cpu.reg.pc_mut() = target;

        board.advance_mcycle();
    }
}

pub fn jp_hl(cpu: &mut CPU) {
    *cpu.reg.pc_mut() = cpu.reg.hl();
}

pub fn pop<CRAM: CartridgeRam>(cpu: &mut CPU, board: &mut Board<CRAM>, rr: R16) {
    *cpu.reg.r16_mut(rr) = board.read16(cpu.reg.sp());
    *cpu.reg.sp_mut() = cpu.reg.sp().wrapping_add(2);
}

pub fn push<CRAM: CartridgeRam>(cpu: &mut CPU, board: &mut Board<CRAM>, rr: R16) {
    *cpu.reg.sp_mut() = cpu.reg.sp().wrapping_sub(2);
    board.advance_mcycle();
    board.write16(cpu.reg.r16(R16::SP), cpu.reg.r16(rr));
}

pub fn rst<CRAM: CartridgeRam>(cpu: &mut CPU, board: &mut Board<CRAM>, target: u16) {
    push(cpu, board, R16::PC);
    *cpu.reg.pc_mut() = target;
}

/// Due to timing differences, this function CANNOT be expressed as ret_cond(..., true)!!!
pub fn ret<CRAM: CartridgeRam>(cpu: &mut CPU, board: &mut Board<CRAM>, enable_ime: bool) {
    pop(cpu, board, R16::PC);

    cpu.ime |= enable_ime;

    board.advance_mcycle();
}

pub fn ret_cond<CRAM: CartridgeRam>(cpu: &mut CPU, board: &mut Board<CRAM>, cond: bool) {
    board.advance_mcycle();

    if cond {
        ret(cpu, board, false);
    }
}

pub fn call_cond<CRAM: CartridgeRam>(cpu: &mut CPU, board: &mut Board<CRAM>, cond: bool) {
    let target = cpu.read16i(board);

    if cond {
        push(cpu, board, R16::PC);
        *cpu.reg.sp_mut() = target;
    }
}

pub fn add_hl_rr<CRAM: CartridgeRam>(cpu: &mut CPU, board: &mut Board<CRAM>, rr: R16) {
    let addend = cpu.reg.r16(rr);

    let (new, carry) = cpu.reg.hl().overflowing_add(addend);

    *cpu.reg.hl_mut() = new;

    cpu.reg.flags_mut().remove(Flags::N);
    cpu.reg
        .flags_mut()
        .set(Flags::H, ((new & 0xFFF) + (addend & 0xFFF)).bit(12));
    cpu.reg.flags_mut().set(Flags::C, carry);

    board.advance_mcycle();
}

pub fn add_sp_r8<CRAM: CartridgeRam>(cpu: &mut CPU, board: &mut Board<CRAM>) {
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
}

pub fn inc_rr<CRAM: CartridgeRam>(cpu: &mut CPU, board: &mut Board<CRAM>, rr: R16) {
    *cpu.reg.r16_mut(rr) = cpu.reg.r16(rr).wrapping_add(1);
    board.advance_mcycle();
}

pub fn dec_rr<CRAM: CartridgeRam>(cpu: &mut CPU, board: &mut Board<CRAM>, rr: R16) {
    *cpu.reg.r16_mut(rr) = cpu.reg.r16(rr).wrapping_sub(1);
    board.advance_mcycle();
}

pub fn inc8<CRAM: CartridgeRam, T: Src8 + Dst8 + Copy>(
    cpu: &mut CPU,
    board: &mut Board<CRAM>,
    target: T,
) {
    let old = target.read(cpu, board);
    let new = old.wrapping_add(1);

    target.write(cpu, board, new);

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().remove(Flags::N);
    cpu.reg.flags_mut().set(Flags::H, (old & 0x0f) == 0x0f);
}

pub fn dec8<CRAM: CartridgeRam, T: Src8 + Dst8 + Copy>(
    cpu: &mut CPU,
    board: &mut Board<CRAM>,
    target: T,
) {
    let old = target.read(cpu, board);
    let new = old.wrapping_sub(1);

    target.write(cpu, board, new);

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().insert(Flags::N);
    cpu.reg.flags_mut().set(Flags::H, (new & 0x0f) == 0x0f);
}

pub fn add8<CRAM: CartridgeRam, S: Src8>(cpu: &mut CPU, board: &mut Board<CRAM>, src: S) {
    let old = src.read(cpu, board);
    let (new, carry) = cpu.reg.r8(R8::A).overflowing_add(old);

    *cpu.reg.r8_mut(R8::A) = new;

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().remove(Flags::N);
    cpu.reg.flags_mut().set(Flags::H, (old & 0x0f) == 0x0f);
    cpu.reg.flags_mut().set(Flags::C, carry);
}

pub fn adc8<CRAM: CartridgeRam, S: Src8>(cpu: &mut CPU, board: &mut Board<CRAM>, src: S) {
    let old = cpu.reg.r8(R8::A);
    let addend = src
        .read(cpu, board)
        .wrapping_add(if cpu.reg.flags().contains(Flags::C) {
            1
        } else {
            0
        });

    if addend == 0 {
        cpu.reg.flags_mut().set(Flags::Z, old == 0);
        cpu.reg.flags_mut().remove(Flags::H); // TODO: Check
        cpu.reg.flags_mut().insert(Flags::C); // TODO: Check
    } else {
        let (new, carry) = old.overflowing_add(addend);

        cpu.reg.flags_mut().set(Flags::Z, new == 0);
        cpu.reg.flags_mut().set(Flags::H, (old & 0x0f) == 0x0f);
        cpu.reg.flags_mut().set(Flags::C, carry);
    }

    cpu.reg.flags_mut().remove(Flags::N);
}

pub fn sub8<CRAM: CartridgeRam, S: Src8>(cpu: &mut CPU, board: &mut Board<CRAM>, src: S) {
    *cpu.reg.r8_mut(R8::A) = cp8(cpu, board, src);
}

pub fn sbc8<CRAM: CartridgeRam, S: Src8>(cpu: &mut CPU, board: &mut Board<CRAM>, src: S) {
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

pub fn cp8<CRAM: CartridgeRam, S: Src8>(cpu: &mut CPU, board: &mut Board<CRAM>, src: S) -> u8 {
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

pub fn and8<CRAM: CartridgeRam, S: Src8>(cpu: &mut CPU, board: &mut Board<CRAM>, src: S) {
    let new = cpu.reg.r8(R8::A) & src.read(cpu, board);

    *cpu.reg.r8_mut(R8::A) = new;

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().remove(Flags::N | Flags::C);
    cpu.reg.flags_mut().insert(Flags::H);
}

pub fn xor8<CRAM: CartridgeRam, S: Src8>(cpu: &mut CPU, board: &mut Board<CRAM>, src: S) {
    let new = cpu.reg.r8(R8::A) ^ src.read(cpu, board);

    *cpu.reg.r8_mut(R8::A) = new;

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().remove(Flags::N | Flags::H | Flags::C);
}

pub fn or8<CRAM: CartridgeRam, S: Src8>(cpu: &mut CPU, board: &mut Board<CRAM>, src: S) {
    let new = cpu.reg.r8(R8::A) | src.read(cpu, board);

    *cpu.reg.r8_mut(R8::A) = new;

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().remove(Flags::N | Flags::H | Flags::C);
}

// CB prefixed Instructions

pub fn rlc<CRAM: CartridgeRam, T: Src8 + Dst8 + Copy>(
    cpu: &mut CPU,
    board: &mut Board<CRAM>,
    target: T,
) {
    let old = target.read(cpu, board);
    target.write(cpu, board, old.rotate_left(1));

    cpu.reg.flags_mut().set(Flags::Z, old == 0);
    cpu.reg.flags_mut().remove(Flags::N | Flags::H);
    cpu.reg.flags_mut().set(Flags::C, old.bit(7));
}

pub fn rrc<CRAM: CartridgeRam, T: Src8 + Dst8 + Copy>(
    cpu: &mut CPU,
    board: &mut Board<CRAM>,
    target: T,
) {
    let old = target.read(cpu, board);
    target.write(cpu, board, old.rotate_right(1));

    cpu.reg.flags_mut().set(Flags::Z, old == 0);
    cpu.reg.flags_mut().remove(Flags::N | Flags::H);
    cpu.reg.flags_mut().set(Flags::C, old.bit(0))
}

pub fn rl<CRAM: CartridgeRam, T: Src8 + Dst8 + Copy>(
    cpu: &mut CPU,
    board: &mut Board<CRAM>,
    target: T,
) {
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

pub fn rr<CRAM: CartridgeRam, T: Src8 + Dst8 + Copy>(
    cpu: &mut CPU,
    board: &mut Board<CRAM>,
    target: T,
) {
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

pub fn sla<CRAM: CartridgeRam, T: Src8 + Dst8 + Copy>(
    cpu: &mut CPU,
    board: &mut Board<CRAM>,
    target: T,
) {
    let old = target.read(cpu, board);
    let new = old << 1;

    target.write(cpu, board, new);

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().remove(Flags::N | Flags::H);
    cpu.reg.flags_mut().set(Flags::C, old.bit(7));
}

pub fn sra<CRAM: CartridgeRam, T: Src8 + Dst8 + Copy>(
    cpu: &mut CPU,
    board: &mut Board<CRAM>,
    target: T,
) {
    let old = target.read(cpu, board);
    let new = (old >> 1) | (old & 0b_1000_0000);

    target.write(cpu, board, new);

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().remove(Flags::N | Flags::H);
    cpu.reg.flags_mut().set(Flags::C, old.bit(0));
}

pub fn swap<CRAM: CartridgeRam, T: Src8 + Dst8 + Copy>(
    cpu: &mut CPU,
    board: &mut Board<CRAM>,
    target: T,
) {
    let old = target.read(cpu, board);
    target.write(cpu, board, (old >> 4) + (old << 4));

    cpu.reg.flags_mut().set(Flags::Z, old == 0);
    cpu.reg.flags_mut().remove(Flags::N | Flags::H | Flags::C);
}

pub fn srl<CRAM: CartridgeRam, T: Src8 + Dst8 + Copy>(
    cpu: &mut CPU,
    board: &mut Board<CRAM>,
    target: T,
) {
    let old = target.read(cpu, board);
    let new = old >> 1;

    target.write(cpu, board, new);

    cpu.reg.flags_mut().set(Flags::Z, new == 0);
    cpu.reg.flags_mut().remove(Flags::N | Flags::H);
    cpu.reg.flags_mut().set(Flags::C, old.bit(0));
}

pub fn bit<CRAM: CartridgeRam, S: Src8>(cpu: &mut CPU, board: &mut Board<CRAM>, bit: u8, src: S) {
    let bit_set = src.read(cpu, board).bit(bit);
    cpu.reg.flags_mut().set(Flags::Z, !bit_set);
    cpu.reg.flags_mut().remove(Flags::N | Flags::H);
}

pub fn res<CRAM: CartridgeRam, T: Src8 + Dst8 + Copy>(
    cpu: &mut CPU,
    board: &mut Board<CRAM>,
    bit: u8,
    target: T,
) {
    let new = target.read(cpu, board).reset_bit(bit);
    target.write(cpu, board, new);
}

pub fn set<CRAM: CartridgeRam, T: Src8 + Dst8 + Copy>(
    cpu: &mut CPU,
    board: &mut Board<CRAM>,
    bit: u8,
    target: T,
) {
    let new = target.read(cpu, board).set_bit(bit);
    target.write(cpu, board, new);
}

pub fn daa(cpu: &mut CPU) {
    // DAA is kind of infamous for having complicated behaviour
    // This is why I took the source code from https://forums.nesdev.com/viewtopic.phpt=15944

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

    // these flags are always updated
    cpu.reg.flags_mut().set(Flags::Z, new == 0); // the usual z flag
    cpu.reg.flags_mut().remove(Flags::H); // h flag is always cleared
}
