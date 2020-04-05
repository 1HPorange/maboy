#![deny(unused_must_use)]

use super::clock;
use super::mmu::MMU;
use static_assertions as sa;
use std::ops::{Index, IndexMut};

// https://stackoverflow.com/questions/41353869/length-of-instruction-ld-a-c-in-gameboy-z80-processor
// > And the other thing that bothers me is the fact that STOP length is 2. It is actually just one byte long.
// > There is a hardware bug on Gameboy Classic that causes the instruction following a STOP to be skipped.
// > So Nintendo started to tell developers to add a NOP always after a STOP.
const SKIP_INSTR_AFTER_STOP: bool = true;

#[derive(Debug)]
pub struct CPU {
    reg: Registers,
    pc: u16,
    flags: Flags,
    /// In literature, this is sometimes called the IME flag (Interrupt Master Enable)
    interrupts_enabled: bool,
}

impl CPU {
    // TODO: Research these values!
    pub fn new() -> CPU {
        CPU {
            reg: Registers::new(),
            pc: 0,
            flags: Flags::new(),
            interrupts_enabled: true,
        }
    }

    pub async fn run(&mut self, mmu: &mut MMU<'_>) {
        loop {
            // If we need to handle an interrupt, we skip normal instruction decoding
            let interrupts_requested = mmu.read8(0xFF0F) & mmu.read8(0xFFFF) & 0x1F;
            if self.interrupts_enabled && interrupts_requested != 0 {
                self.decode_interrupt(mmu, interrupts_requested);
                clock::ticks(4).await; // TODO: Research this timing
            } else {
                // Safe transmute since we have u8::MAX instructions
                sa::const_assert_eq!(Instruction::RST_38H as u8, std::u8::MAX);
                let instruction: Instruction = unsafe { std::mem::transmute(self.read8(mmu)) };

                // println!(
                //     "Executing {:?} with PC now at {:#06X}",
                //     instruction, self.pc
                // );
                // dbg!(&self);

                self.execute(instruction, mmu).await;
            }
        }
    }

    fn read8(&mut self, mmu: &MMU<'_>) -> u8 {
        let val = mmu.read8(self.pc);
        self.pc = self.pc.wrapping_add(1);
        val
    }

    fn read16(&mut self, mmu: &MMU<'_>) -> u16 {
        let val = mmu.read16(self.pc);
        self.pc = self.pc.wrapping_add(2);
        val
    }

    async fn add_hl(&mut self, addend: R16) {
        let addend = self.reg.r16(addend);
        let hl = self.reg.r16_mut(R16::HL);

        let (sum, c) = hl.overflowing_add(addend);

        // Contains result of addition if each overflow is thrown away
        let xor = *hl ^ addend;

        // The difference to the actual results are bits that were overflowed into
        let overflow = sum ^ xor;

        self.flags[Flag::N] = false;
        self.flags[Flag::H] = (overflow & 0b10000) != 0;
        self.flags[Flag::C] = c;

        *hl = sum;
    }

    // num_traits could avoid some code duplication here, but we just avoid any
    // potential overhead

    fn inc8<O: Into<Operand>>(&mut self, mmu: &mut MMU, target: O) {
        let mut target = target.into();
        let old = target.read(self, mmu);

        let h = (old & 0b1111) == 0b1111;

        let new = old.wrapping_add(1);

        target.write(self, mmu, new);

        self.flags[Flag::Z] = new == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = h;
    }

    fn dec8<O: Into<Operand>>(&mut self, mmu: &mut MMU, target: O) {
        let mut target = target.into();
        let old = target.read(self, mmu);

        let h = old == 0;

        let new = old.wrapping_sub(1);

        target.write(self, mmu, new);

        self.flags[Flag::Z] = new == 0;
        self.flags[Flag::N] = true;
        self.flags[Flag::H] = h;
    }

    fn add(&mut self, n: u8) {
        let target = self.reg.r8_mut(R8::A);

        let (sum, c) = target.overflowing_add(n);

        // Contains result of addition if each overflow is thrown away
        let xor = *target ^ n;

        // The difference to the actual results are bits that were overflowed into
        let overflow = sum ^ xor;

        *target = sum;

        self.flags[Flag::Z] = *target == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = (overflow & 0b10000) != 0;
        self.flags[Flag::C] = c;
    }

    fn adc(&mut self, n: u8) {
        let target = self.reg.r8_mut(R8::A);

        let (mut sum, mut c) = target.overflowing_add(n);

        if self.flags[Flag::C] {
            let (s_new, c_new) = sum.overflowing_add(1);
            sum = s_new;
            c |= c_new;
        }

        // Contains result of addition if each overflow is thrown away
        let xor = *target ^ n;

        // The difference to the actual results are bits that were overflowed into
        let overflow = sum ^ xor;

        *target = sum;

        self.flags[Flag::Z] = *target == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = (overflow & 0b10000) != 0;
        self.flags[Flag::C] = c;
    }

    fn sub(&mut self, n: u8) {
        *self.reg.r8_mut(R8::A) = self.cp(n);
    }

    fn sbc(&mut self, mut n: u8) {
        let target = self.reg.r8_mut(R8::A);

        if self.flags[Flag::C] {
            n = n.wrapping_add(1);
        }

        let h = n > *target;

        let (diff, c) = target.overflowing_sub(n);
        *target = diff;

        self.flags[Flag::Z] = diff == 0;
        self.flags[Flag::N] = true;
        self.flags[Flag::H] = h;
        self.flags[Flag::C] = c;
    }

    /// Returns A-n, which can be used to implement SUB_n
    fn cp(&mut self, n: u8) -> u8 {
        let reference = self.reg.r8(R8::A);

        let h = n > reference;

        let (diff, c) = reference.overflowing_sub(n);

        self.flags[Flag::Z] = diff == 0;
        self.flags[Flag::N] = true;
        self.flags[Flag::H] = h;
        self.flags[Flag::C] = c;

        diff
    }

    fn and(&mut self, n: u8) {
        let target = self.reg.r8_mut(R8::A);

        *target &= n;

        self.flags[Flag::Z] = *target == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = true;
        self.flags[Flag::C] = false;
    }

    fn xor(&mut self, n: u8) {
        let target = self.reg.r8_mut(R8::A);

        *target ^= n;

        self.flags[Flag::Z] = *target == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = false;
        self.flags[Flag::C] = false;
    }

    fn or(&mut self, n: u8) {
        let target = self.reg.r8_mut(R8::A);

        *target |= n;

        self.flags[Flag::Z] = *target == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = false;
        self.flags[Flag::C] = false;
    }

    fn jmpr(&mut self, offset: u8) {
        let offset: i8 = unsafe { std::mem::transmute(offset) };
        // This cast works as offset is in 2s complement
        self.pc = self.pc.wrapping_add(offset as u16);
    }

    fn pop(&mut self, mmu: &MMU) -> u16 {
        let sp = self.reg.r16_mut(R16::SP);
        let val = mmu.read16(*sp);
        *sp = sp.wrapping_add(2);
        val
    }

    fn push(&mut self, mmu: &mut MMU, val: u16) {
        let sp = self.reg.r16_mut(R16::SP);
        mmu.write16(*sp, val);
        *sp = sp.wrapping_sub(2);
    }

    async fn execute(&mut self, instruction: Instruction, mmu: &mut MMU<'_>) {
        use Instruction::*;
        use R16::*;
        use R8::*;

        match instruction {
            NOP => {
                clock::ticks(4).await;
                panic!("You did it! You reached NOP!")
            }
            LD_BC_d16 => {
                clock::ticks(12).await;
                *self.reg.r16_mut(BC) = self.read16(mmu);
            }
            LD_xBCx_A => {
                clock::ticks(8).await;
                mmu.write8(self.reg.r16(BC), self.reg.r8(A));
            }
            INC_BC => {
                clock::ticks(8).await;
                let bc = self.reg.r16_mut(BC);
                *bc = bc.wrapping_add(1);
            }
            INC_B => {
                clock::ticks(4).await;
                self.inc8(mmu, B);
            }
            DEC_B => {
                clock::ticks(4).await;
                self.dec8(mmu, B);
            }
            LD_B_d8 => {
                clock::ticks(8).await;
                *self.reg.r8_mut(B) = self.read8(mmu);
            }
            RLCA => {
                clock::ticks(4).await;
                let target = self.reg.r8_mut(A);
                self.flags[Flag::C] = (*target & 0b1000_0000) != 0;
                *target = target.rotate_left(1);
            }
            LD_xa16x_SP => {
                clock::ticks(20).await;
                mmu.write16(self.read16(mmu), self.reg.r16(SP));
            }
            ADD_HL_BC => {
                clock::ticks(8).await;
                self.add_hl(BC).await;
            }
            LD_A_xBCx => {
                clock::ticks(8).await;
                *self.reg.r8_mut(A) = mmu.read8(self.reg.r16(BC));
            }
            DEC_BC => {
                clock::ticks(8).await;
                let bc = self.reg.r16_mut(BC);
                *bc = bc.wrapping_sub(1);
            }
            INC_C => {
                clock::ticks(4).await;
                self.inc8(mmu, C);
            }
            DEC_C => {
                clock::ticks(4).await;
                self.dec8(mmu, C);
            }
            LD_C_d8 => {
                clock::ticks(8).await;
                *self.reg.r8_mut(C) = self.read8(mmu);
            }
            RRCA => {
                clock::ticks(4).await;
                let target = self.reg.r8_mut(A);
                self.flags[Flag::C] = (*target & 1) != 0;
                *target = target.rotate_right(1);
            }
            STOP => {
                if SKIP_INSTR_AFTER_STOP {
                    self.read8(mmu);
                }
                panic!("Reached STOP instruction");
            }
            LD_DE_d16 => {
                clock::ticks(12).await;
                *self.reg.r16_mut(DE) = self.read16(mmu);
            }
            LD_xDEx_A => {
                clock::ticks(8).await;
                mmu.write8(self.reg.r16(DE), self.reg.r8(A));
            }
            INC_DE => {
                clock::ticks(8).await;
                let de = self.reg.r16_mut(DE);
                *de = de.wrapping_add(1);
            }
            INC_D => {
                clock::ticks(4).await;
                self.inc8(mmu, D);
            }
            DEC_D => {
                clock::ticks(4).await;
                self.dec8(mmu, D);
            }
            LD_D_d8 => {
                clock::ticks(8).await;
                *self.reg.r8_mut(D) = self.read8(mmu);
            }
            RLA => {
                // Can't us self.rl because it sets the zero flag
                clock::ticks(4).await;
                let target = self.reg.r8_mut(A);
                let c = self.flags[Flag::C];
                self.flags[Flag::C] = (*target & 0b1000_0000) != 0;
                *target <<= 1;
                if c {
                    *target += 1;
                }
            }
            JR_r8 => {
                clock::ticks(12).await;
                let offset = self.read8(mmu);
                self.jmpr(offset);
            }
            ADD_HL_DE => {
                clock::ticks(8).await;
                self.add_hl(DE).await;
            }
            LD_A_xDEx => {
                clock::ticks(8).await;
                *self.reg.r8_mut(A) = mmu.read8(self.reg.r16(DE));
            }
            DEC_DE => {
                clock::ticks(8).await;
                let de = self.reg.r16_mut(DE);
                *de = de.wrapping_sub(1);
            }
            INC_E => {
                clock::ticks(4).await;
                self.inc8(mmu, E);
            }
            DEC_E => {
                clock::ticks(4).await;
                self.dec8(mmu, E);
            }
            LD_E_d8 => {
                clock::ticks(8).await;
                *self.reg.r8_mut(E) = self.read8(mmu);
            }
            RRA => {
                // Can't us self.rr because it sets the zero flag
                clock::ticks(4).await;
                let target = self.reg.r8_mut(A);
                let c = self.flags[Flag::C];
                self.flags[Flag::C] = (*target & 0b1) != 0;
                *target >>= 1;
                if c {
                    *target += 0b1000_0000;
                }
            }
            JR_NZ_r8 => {
                clock::ticks(8).await;
                let offset = self.read8(mmu);
                if !self.flags[Flag::Z] {
                    clock::ticks(4).await;
                    self.jmpr(offset);
                }
            }
            LD_HL_d16 => {
                clock::ticks(12).await;
                *self.reg.r16_mut(HL) = self.read16(mmu);
            }
            LD_xHLix_A => {
                clock::ticks(8).await;
                mmu.write8(self.reg.r16(HL), self.reg.r8(A));
                let hl = self.reg.r16_mut(HL);
                *hl = hl.wrapping_add(1);
            }
            INC_HL => {
                clock::ticks(8).await;
                *self.reg.r16_mut(HL) += 1;
            }
            INC_H => {
                clock::ticks(4).await;
                self.inc8(mmu, H);
            }
            DEC_H => {
                clock::ticks(4).await;
                self.dec8(mmu, H);
            }
            LD_H_d8 => {
                clock::ticks(8).await;
                *self.reg.r8_mut(H) = self.read8(mmu);
            }
            DAA => {
                // DAA is kind of infamous for having complicated behaviour
                // This is why I took the source code from https://forums.nesdev.com/viewtopic.phpt=15944

                clock::ticks(4).await;

                let a = self.reg.r8_mut(A);

                // note: assumes a is a uint8_t and wraps from 0xff to 0
                if !self.flags[Flag::N] {
                    // after an addition, adjust if (half-)carry occurred or if result is out of bounds
                    if self.flags[Flag::C] || *a > 0x99 {
                        *a = a.wrapping_add(0x60);
                        self.flags[Flag::C] = true;
                    }
                    if self.flags[Flag::H] || (*a & 0x0f) > 0x09 {
                        *a = a.wrapping_add(0x6);
                    }
                } else {
                    // after a subtraction, only adjust if (half-)carry occurred
                    if self.flags[Flag::C] {
                        *a = a.wrapping_sub(0x60);
                    }
                    if self.flags[Flag::H] {
                        *a = a.wrapping_sub(0x6);
                    }
                }
                // these flags are always updated
                self.flags[Flag::Z] = *a == 0; // the usual z flag
                self.flags[Flag::H] = false; // h flag is always cleared
            }
            JR_Z_r8 => {
                clock::ticks(8).await;
                let offset = self.read8(mmu);
                if self.flags[Flag::Z] {
                    clock::ticks(4).await;
                    self.jmpr(offset);
                }
            }
            ADD_HL_HL => {
                clock::ticks(8).await;
                self.add_hl(HL).await;
            }
            LD_A_xHLix => {
                clock::ticks(8).await;
                *self.reg.r8_mut(A) = mmu.read8(self.reg.r16(HL));
                let hl = self.reg.r16_mut(HL);
                *hl = hl.wrapping_add(1);
            }
            DEC_HL => {
                clock::ticks(8).await;
                let hl = self.reg.r16_mut(HL);
                *hl = hl.wrapping_sub(1);
            }
            INC_L => {
                clock::ticks(4).await;
                self.inc8(mmu, L);
            }
            DEC_L => {
                clock::ticks(4).await;
                self.dec8(mmu, L);
            }
            LD_L_d8 => {
                clock::ticks(8).await;
                *self.reg.r8_mut(L) = self.read8(mmu);
            }
            CPL => {
                clock::ticks(4).await;
                let a = self.reg.r8_mut(A);
                *a = !*a;
                self.flags[Flag::N] = true;
                self.flags[Flag::H] = true;
            }
            JR_NC_r8 => {
                clock::ticks(8).await;
                let offset = self.read8(mmu);
                if !self.flags[Flag::C] {
                    clock::ticks(4).await;
                    self.jmpr(offset);
                }
            }
            LD_SP_d16 => {
                clock::ticks(12).await;
                *self.reg.r16_mut(SP) = self.read16(mmu);
            }
            LD_xHLdx_A => {
                clock::ticks(8).await;
                mmu.write8(self.reg.r16(HL), self.reg.r8(A));
                let hl = self.reg.r16_mut(HL);
                *hl = hl.wrapping_sub(1);
            }
            INC_SP => {
                clock::ticks(8).await;
                let sp = self.reg.r16_mut(SP);
                *sp = sp.wrapping_add(1);
            }
            INC_xHLx => {
                clock::ticks(12).await;
                self.inc8(mmu, Operand::HLAddr);
            }
            DEC_xHLx => {
                clock::ticks(12).await;
                self.dec8(mmu, Operand::HLAddr);
            }
            LD_xHLx_d8 => {
                clock::ticks(12).await;
                mmu.write8(self.reg.r16(HL), self.read8(mmu));
            }
            SCF => {
                clock::ticks(4).await;
                self.flags[Flag::N] = false;
                self.flags[Flag::H] = false;
                self.flags[Flag::C] = true;
            }
            JR_C_r8 => {
                clock::ticks(8).await;
                let offset = self.read8(mmu);
                if self.flags[Flag::C] {
                    clock::ticks(4).await;
                    self.jmpr(offset);
                }
            }
            ADD_HL_SP => {
                clock::ticks(8).await;
                self.add_hl(SP).await;
            }
            LD_A_xHLdx => {
                clock::ticks(8).await;
                *self.reg.r8_mut(A) = mmu.read8(self.reg.r16(HL));
                let hl = self.reg.r16_mut(HL);
                *hl = hl.wrapping_sub(1);
            }
            DEC_SP => {
                clock::ticks(8).await;
                let sp = self.reg.r16_mut(SP);
                *sp = sp.wrapping_sub(1);
            }
            INC_A => {
                clock::ticks(4).await;
                self.inc8(mmu, A);
            }
            DEC_A => {
                clock::ticks(4).await;
                self.dec8(mmu, A);
            }
            LD_A_d8 => {
                clock::ticks(8).await;
                *self.reg.r8_mut(A) = self.read8(mmu);
            }
            CCF => {
                clock::ticks(4).await;
                self.flags[Flag::N] = false;
                self.flags[Flag::H] = false;
                self.flags[Flag::C] = !self.flags[Flag::C];
            }
            LD_B_B => {
                clock::ticks(4).await;
                // *self.reg.r8_mut(B) = self.reg.r8(B);
            }
            LD_B_C => {
                clock::ticks(4).await;
                *self.reg.r8_mut(B) = self.reg.r8(C);
            }
            LD_B_D => {
                clock::ticks(4).await;
                *self.reg.r8_mut(B) = self.reg.r8(D);
            }
            LD_B_E => {
                clock::ticks(4).await;
                *self.reg.r8_mut(B) = self.reg.r8(E);
            }
            LD_B_H => {
                clock::ticks(4).await;
                *self.reg.r8_mut(B) = self.reg.r8(H);
            }
            LD_B_L => {
                clock::ticks(4).await;
                *self.reg.r8_mut(B) = self.reg.r8(L);
            }
            LD_B_xHLx => {
                clock::ticks(8).await;
                *self.reg.r8_mut(B) = mmu.read8(self.reg.r16(HL));
            }
            LD_B_A => {
                clock::ticks(4).await;
                *self.reg.r8_mut(B) = self.reg.r8(A);
            }
            LD_C_B => {
                clock::ticks(4).await;
                *self.reg.r8_mut(C) = self.reg.r8(B);
            }
            LD_C_C => {
                clock::ticks(4).await;
                //*self.reg.r8_mut(C) = self.reg.r8(C);
            }
            LD_C_D => {
                clock::ticks(4).await;
                *self.reg.r8_mut(C) = self.reg.r8(D);
            }
            LD_C_E => {
                clock::ticks(4).await;
                *self.reg.r8_mut(C) = self.reg.r8(E);
            }
            LD_C_H => {
                clock::ticks(4).await;
                *self.reg.r8_mut(C) = self.reg.r8(H);
            }
            LD_C_L => {
                clock::ticks(4).await;
                *self.reg.r8_mut(C) = self.reg.r8(L);
            }
            LD_C_xHLx => {
                clock::ticks(8).await;
                *self.reg.r8_mut(C) = mmu.read8(self.reg.r16(HL));
            }
            LD_C_A => {
                clock::ticks(4).await;
                *self.reg.r8_mut(C) = self.reg.r8(A);
            }
            LD_D_B => {
                clock::ticks(4).await;
                *self.reg.r8_mut(D) = self.reg.r8(B);
            }
            LD_D_C => {
                clock::ticks(4).await;
                *self.reg.r8_mut(D) = self.reg.r8(C);
            }
            LD_D_D => {
                clock::ticks(4).await;
                // *self.reg.r8_mut(D) = self.reg.r8(D);
            }
            LD_D_E => {
                clock::ticks(4).await;
                *self.reg.r8_mut(D) = self.reg.r8(E);
            }
            LD_D_H => {
                clock::ticks(4).await;
                *self.reg.r8_mut(D) = self.reg.r8(H);
            }
            LD_D_L => {
                clock::ticks(4).await;
                *self.reg.r8_mut(D) = self.reg.r8(L);
            }
            LD_D_xHLx => {
                clock::ticks(8).await;
                *self.reg.r8_mut(D) = mmu.read8(self.reg.r16(HL));
            }
            LD_D_A => {
                clock::ticks(4).await;
                *self.reg.r8_mut(D) = self.reg.r8(A);
            }
            LD_E_B => {
                clock::ticks(4).await;
                *self.reg.r8_mut(E) = self.reg.r8(B);
            }
            LD_E_C => {
                clock::ticks(4).await;
                *self.reg.r8_mut(E) = self.reg.r8(C);
            }
            LD_E_D => {
                clock::ticks(4).await;
                *self.reg.r8_mut(E) = self.reg.r8(D);
            }
            LD_E_E => {
                clock::ticks(4).await;
                // *self.reg.r8_mut(E) = self.reg.r8(E);
            }
            LD_E_H => {
                clock::ticks(4).await;
                *self.reg.r8_mut(E) = self.reg.r8(H);
            }
            LD_E_L => {
                clock::ticks(4).await;
                *self.reg.r8_mut(E) = self.reg.r8(L);
            }
            LD_E_xHLx => {
                clock::ticks(8).await;
                *self.reg.r8_mut(E) = mmu.read8(self.reg.r16(HL));
            }
            LD_E_A => {
                clock::ticks(4).await;
                *self.reg.r8_mut(E) = self.reg.r8(A);
            }
            LD_H_B => {
                clock::ticks(4).await;
                *self.reg.r8_mut(H) = self.reg.r8(B);
            }
            LD_H_C => {
                clock::ticks(4).await;
                *self.reg.r8_mut(H) = self.reg.r8(C);
            }
            LD_H_D => {
                clock::ticks(4).await;
                *self.reg.r8_mut(H) = self.reg.r8(D);
            }
            LD_H_E => {
                clock::ticks(4).await;
                *self.reg.r8_mut(H) = self.reg.r8(E);
            }
            LD_H_H => {
                clock::ticks(4).await;
                // *self.reg.r8_mut(H) = self.reg.r8(H);
            }
            LD_H_L => {
                clock::ticks(4).await;
                *self.reg.r8_mut(H) = self.reg.r8(L);
            }
            LD_H_xHLx => {
                clock::ticks(8).await;
                *self.reg.r8_mut(H) = mmu.read8(self.reg.r16(HL));
            }
            LD_H_A => {
                clock::ticks(4).await;
                *self.reg.r8_mut(H) = self.reg.r8(A);
            }
            LD_L_B => {
                clock::ticks(4).await;
                *self.reg.r8_mut(L) = self.reg.r8(B);
            }
            LD_L_C => {
                clock::ticks(4).await;
                *self.reg.r8_mut(L) = self.reg.r8(C);
            }
            LD_L_D => {
                clock::ticks(4).await;
                *self.reg.r8_mut(L) = self.reg.r8(D);
            }
            LD_L_E => {
                clock::ticks(4).await;
                *self.reg.r8_mut(L) = self.reg.r8(E);
            }
            LD_L_H => {
                clock::ticks(4).await;
                *self.reg.r8_mut(L) = self.reg.r8(H);
            }
            LD_L_L => {
                clock::ticks(4).await;
                // *self.reg.r8_mut(L) = self.reg.r8(L);
            }
            LD_L_xHLx => {
                clock::ticks(8).await;
                *self.reg.r8_mut(L) = mmu.read8(self.reg.r16(HL));
            }
            LD_L_A => {
                clock::ticks(4).await;
                *self.reg.r8_mut(L) = self.reg.r8(A);
            }
            LD_xHLx_B => {
                clock::ticks(8).await;
                mmu.write8(self.reg.r16(HL), self.reg.r8(B));
            }
            LD_xHLx_C => {
                clock::ticks(8).await;
                mmu.write8(self.reg.r16(HL), self.reg.r8(C));
            }
            LD_xHLx_D => {
                clock::ticks(8).await;
                mmu.write8(self.reg.r16(HL), self.reg.r8(D));
            }
            LD_xHLx_E => {
                clock::ticks(8).await;
                mmu.write8(self.reg.r16(HL), self.reg.r8(E));
            }
            LD_xHLx_H => {
                clock::ticks(8).await;
                mmu.write8(self.reg.r16(HL), self.reg.r8(H));
            }
            LD_xHLx_L => {
                clock::ticks(8).await;
                mmu.write8(self.reg.r16(HL), self.reg.r8(L));
            }
            HALT => {
                panic!("Reached HALT instruction");
            }
            LD_xHLx_A => {
                clock::ticks(8).await;
                mmu.write8(self.reg.r16(HL), self.reg.r8(A));
            }
            LD_A_B => {
                clock::ticks(4).await;
                *self.reg.r8_mut(A) = self.reg.r8(B);
            }
            LD_A_C => {
                clock::ticks(4).await;
                *self.reg.r8_mut(A) = self.reg.r8(C);
            }
            LD_A_D => {
                clock::ticks(4).await;
                *self.reg.r8_mut(A) = self.reg.r8(D);
            }
            LD_A_E => {
                clock::ticks(4).await;
                *self.reg.r8_mut(A) = self.reg.r8(E);
            }
            LD_A_H => {
                clock::ticks(4).await;
                *self.reg.r8_mut(A) = self.reg.r8(H);
            }
            LD_A_L => {
                clock::ticks(4).await;
                *self.reg.r8_mut(A) = self.reg.r8(L);
            }
            LD_A_xHLx => {
                clock::ticks(8).await;
                *self.reg.r8_mut(A) = mmu.read8(self.reg.r16(HL));
            }
            LD_A_A => {
                clock::ticks(4).await;
                // self.reg.r8_mut(A) = self.reg.r8(A);
            }
            ADD_A_B => {
                clock::ticks(4).await;
                self.add(self.reg.r8(B));
            }
            ADD_A_C => {
                clock::ticks(4).await;
                self.add(self.reg.r8(C));
            }
            ADD_A_D => {
                clock::ticks(4).await;
                self.add(self.reg.r8(D));
            }
            ADD_A_E => {
                clock::ticks(4).await;
                self.add(self.reg.r8(E));
            }
            ADD_A_H => {
                clock::ticks(4).await;
                self.add(self.reg.r8(H));
            }
            ADD_A_L => {
                clock::ticks(4).await;
                self.add(self.reg.r8(L));
            }
            ADD_A_xHLx => {
                clock::ticks(8).await;
                self.add(mmu.read8(self.reg.r16(HL)));
            }
            ADD_A_A => {
                clock::ticks(4).await;
                self.add(self.reg.r8(A));
            }
            ADC_A_B => {
                clock::ticks(4).await;
                self.adc(self.reg.r8(B));
            }
            ADC_A_C => {
                clock::ticks(4).await;
                self.adc(self.reg.r8(C));
            }
            ADC_A_D => {
                clock::ticks(4).await;
                self.adc(self.reg.r8(D));
            }
            ADC_A_E => {
                clock::ticks(4).await;
                self.adc(self.reg.r8(E));
            }
            ADC_A_H => {
                clock::ticks(4).await;
                self.adc(self.reg.r8(H));
            }
            ADC_A_L => {
                clock::ticks(4).await;
                self.adc(self.reg.r8(L));
            }
            ADC_A_xHLx => {
                clock::ticks(8).await;
                self.adc(mmu.read8(self.reg.r16(HL)));
            }
            ADC_A_A => {
                clock::ticks(4).await;
                self.adc(self.reg.r8(A));
            }
            SUB_B => {
                clock::ticks(4).await;
                self.sub(self.reg.r8(B));
            }
            SUB_C => {
                clock::ticks(4).await;
                self.sub(self.reg.r8(C));
            }
            SUB_D => {
                clock::ticks(4).await;
                self.sub(self.reg.r8(D));
            }
            SUB_E => {
                clock::ticks(4).await;
                self.sub(self.reg.r8(E));
            }
            SUB_H => {
                clock::ticks(4).await;
                self.sub(self.reg.r8(H));
            }
            SUB_L => {
                clock::ticks(4).await;
                self.sub(self.reg.r8(L));
            }
            SUB_xHLx => {
                clock::ticks(8).await;
                self.sub(mmu.read8(self.reg.r16(HL)));
            }
            SUB_A => {
                clock::ticks(4).await;
                self.sub(self.reg.r8(A));
            }
            SBC_A_B => {
                clock::ticks(4).await;
                self.sbc(self.reg.r8(B));
            }
            SBC_A_C => {
                clock::ticks(4).await;
                self.sbc(self.reg.r8(C));
            }
            SBC_A_D => {
                clock::ticks(4).await;
                self.sbc(self.reg.r8(D));
            }
            SBC_A_E => {
                clock::ticks(4).await;
                self.sbc(self.reg.r8(E));
            }
            SBC_A_H => {
                clock::ticks(4).await;
                self.sbc(self.reg.r8(H));
            }
            SBC_A_L => {
                clock::ticks(4).await;
                self.sbc(self.reg.r8(L));
            }
            SBC_A_xHLx => {
                clock::ticks(8).await;
                self.sbc(mmu.read8(self.reg.r16(HL)));
            }
            SBC_A_A => {
                clock::ticks(4).await;
                self.sbc(self.reg.r8(A));
            }
            AND_B => {
                clock::ticks(4).await;
                self.and(self.reg.r8(B));
            }
            AND_C => {
                clock::ticks(4).await;
                self.and(self.reg.r8(C));
            }
            AND_D => {
                clock::ticks(4).await;
                self.and(self.reg.r8(D));
            }
            AND_E => {
                clock::ticks(4).await;
                self.and(self.reg.r8(E));
            }
            AND_H => {
                clock::ticks(4).await;
                self.and(self.reg.r8(H));
            }
            AND_L => {
                clock::ticks(4).await;
                self.and(self.reg.r8(L));
            }
            AND_xHLx => {
                clock::ticks(8).await;
                self.and(mmu.read8(self.reg.r16(HL)));
            }
            AND_A => {
                clock::ticks(4).await;
                self.and(self.reg.r8(A));
            }
            XOR_B => {
                clock::ticks(4).await;
                self.xor(self.reg.r8(B));
            }
            XOR_C => {
                clock::ticks(4).await;
                self.xor(self.reg.r8(C));
            }
            XOR_D => {
                clock::ticks(4).await;
                self.xor(self.reg.r8(D));
            }
            XOR_E => {
                clock::ticks(4).await;
                self.xor(self.reg.r8(E));
            }
            XOR_H => {
                clock::ticks(4).await;
                self.xor(self.reg.r8(H));
            }
            XOR_L => {
                clock::ticks(4).await;
                self.xor(self.reg.r8(L));
            }
            XOR_xHLx => {
                clock::ticks(8).await;
                self.xor(mmu.read8(self.reg.r16(HL)));
            }
            XOR_A => {
                clock::ticks(4).await;
                self.xor(self.reg.r8(A));
            }
            OR_B => {
                clock::ticks(4).await;
                self.or(self.reg.r8(B));
            }
            OR_C => {
                clock::ticks(4).await;
                self.or(self.reg.r8(C));
            }
            OR_D => {
                clock::ticks(4).await;
                self.or(self.reg.r8(D));
            }
            OR_E => {
                clock::ticks(4).await;
                self.or(self.reg.r8(E));
            }
            OR_H => {
                clock::ticks(4).await;
                self.or(self.reg.r8(H));
            }
            OR_L => {
                clock::ticks(4).await;
                self.or(self.reg.r8(L));
            }
            OR_xHLx => {
                clock::ticks(4).await;
                self.or(mmu.read8(self.reg.r16(HL)));
            }
            OR_A => {
                clock::ticks(4).await;
                self.or(self.reg.r8(A));
            }
            CP_B => {
                clock::ticks(4).await;
                self.cp(self.reg.r8(B));
            }
            CP_C => {
                clock::ticks(4).await;
                self.cp(self.reg.r8(C));
            }
            CP_D => {
                clock::ticks(4).await;
                self.cp(self.reg.r8(D));
            }
            CP_E => {
                clock::ticks(4).await;
                self.cp(self.reg.r8(E));
            }
            CP_H => {
                clock::ticks(4).await;
                self.cp(self.reg.r8(H));
            }
            CP_L => {
                clock::ticks(4).await;
                self.cp(self.reg.r8(L));
            }
            CP_xHLx => {
                clock::ticks(8).await;
                self.cp(mmu.read8(self.reg.r16(HL)));
            }
            CP_A => {
                clock::ticks(4).await;
                self.cp(self.reg.r8(A));
            }
            RET_NZ => {
                clock::ticks(8).await;
                if !self.flags[Flag::Z] {
                    clock::ticks(12).await;
                    self.pc = self.pop(mmu);
                }
            }
            POP_BC => {
                clock::ticks(12).await;
                *self.reg.r16_mut(BC) = self.pop(mmu);
            }
            JP_NZ_a16 => {
                clock::ticks(12).await;
                let target = self.read16(mmu);
                if !self.flags[Flag::Z] {
                    clock::ticks(4).await;
                    self.pc = target;
                }
            }
            JP_a16 => {
                clock::ticks(16).await;
                self.pc = self.read16(mmu);
            }
            CALL_NZ_a16 => {
                clock::ticks(12).await;
                let target = self.read16(mmu);

                if !self.flags[Flag::Z] {
                    clock::ticks(12).await;
                    self.push(mmu, self.pc);
                    self.pc = target;
                }
            }
            PUSH_BC => {
                clock::ticks(16).await;
                self.push(mmu, self.reg.r16(BC));
            }
            ADD_A_d8 => {
                clock::ticks(8).await;
                let d8 = self.read8(mmu);
                self.add(d8);
            }
            RST_00H => {
                clock::ticks(16).await;
                self.push(mmu, self.pc);
                self.pc = 0x00;
            }
            RET_Z => {
                clock::ticks(8).await;
                if self.flags[Flag::Z] {
                    clock::ticks(12).await;
                    self.pc = self.pop(mmu);
                }
            }
            RET => {
                clock::ticks(16).await;
                self.pc = self.pop(mmu);
            }
            JP_Z_a16 => {
                clock::ticks(12).await;
                let target = self.read16(mmu);
                if self.flags[Flag::Z] {
                    clock::ticks(4).await;
                    self.pc = target;
                }
            }
            PREFIX_CB => {
                // Clock ticks are consumed by the prefixed commands to avoid confusion

                sa::const_assert_eq!(CBInstruction::SET_7_A as u8, std::u8::MAX);
                let cb_instruction: CBInstruction = unsafe { std::mem::transmute(self.read8(mmu)) };

                self.execute_cb(cb_instruction, mmu).await;
            }
            CALL_Z_a16 => {
                clock::ticks(12).await;
                let target = self.read16(mmu);

                if self.flags[Flag::Z] {
                    clock::ticks(12).await;
                    self.push(mmu, self.pc);
                    self.pc = target;
                }
            }
            CALL_a16 => {
                clock::ticks(24).await;
                self.push(mmu, self.pc);
                self.pc = self.read16(mmu);
            }
            ADC_A_d8 => {
                clock::ticks(8).await;
                let d8 = self.read8(mmu);
                self.adc(d8);
            }
            RST_08H => {
                clock::ticks(16).await;
                self.push(mmu, self.pc);
                self.pc = 0x08;
            }
            RET_NC => {
                clock::ticks(8).await;
                if !self.flags[Flag::C] {
                    clock::ticks(12).await;
                    self.pc = self.pop(mmu);
                }
            }
            POP_DE => {
                clock::ticks(12).await;
                *self.reg.r16_mut(DE) = self.pop(mmu);
            }
            JP_NC_a16 => {
                clock::ticks(12).await;
                let target = self.read16(mmu);
                if !self.flags[Flag::C] {
                    clock::ticks(4).await;
                    self.pc = target;
                }
            }
            NOT_USED => {
                panic!("Attempted to execute unused instruction");
            }
            CALL_NC_a16 => {
                clock::ticks(12).await;
                let target = self.read16(mmu);

                if !self.flags[Flag::C] {
                    clock::ticks(12).await;
                    self.push(mmu, self.pc);
                    self.pc = target;
                }
            }
            PUSH_DE => {
                clock::ticks(16).await;
                self.push(mmu, self.reg.r16(DE));
            }
            SUB_d8 => {
                clock::ticks(8).await;
                let d8 = self.read8(mmu);
                self.sub(d8);
            }
            RST_10H => {
                clock::ticks(16).await;
                self.push(mmu, self.pc);
                self.pc = 0x10;
            }
            RET_C => {
                clock::ticks(8).await;
                if self.flags[Flag::C] {
                    clock::ticks(12).await;
                    self.pc = self.pop(mmu);
                }
            }
            RETI => {
                clock::ticks(16).await;
                self.pc = self.pop(mmu);
                self.interrupts_enabled = true;
            }
            JP_C_a16 => {
                clock::ticks(12).await;
                let target = self.read16(mmu);
                if self.flags[Flag::C] {
                    clock::ticks(4).await;
                    self.pc = target;
                }
            }
            NOT_USED_0 => {
                panic!("Attempted to execute unused instruction");
            }
            CALL_C_a16 => {
                clock::ticks(12).await;
                let target = self.read16(mmu);

                if self.flags[Flag::C] {
                    clock::ticks(12).await;
                    self.push(mmu, self.pc);
                    self.pc = target;
                }
            }
            NOT_USED_1 => {
                panic!("Attempted to execute unused instruction");
            }
            SBC_A_d8 => {
                clock::ticks(8).await;
                let d8 = self.read8(mmu);
                self.sbc(d8);
            }
            RST_18H => {
                clock::ticks(16).await;
                self.push(mmu, self.pc);
                self.pc = 0x18;
            }
            LDH_xa8x_A => {
                clock::ticks(12).await;
                let offset = self.read8(mmu);
                mmu.write8(0xFF00 + offset as u16, self.reg.r8(A));
            }
            POP_HL => {
                clock::ticks(12).await;
                *self.reg.r16_mut(HL) = self.pop(mmu);
            }
            LD_xCx_A => {
                clock::ticks(8).await;
                mmu.write8(0xFF00 + self.reg.r8(C) as u16, self.reg.r8(A));
            }
            NOT_USED_2 => {
                panic!("Attempted to execute unused instruction");
            }
            NOT_USED_3 => {
                panic!("Attempted to execute unused instruction");
            }
            PUSH_HL => {
                clock::ticks(16).await;
                self.push(mmu, self.reg.r16(HL));
            }
            AND_d8 => {
                clock::ticks(8).await;
                let d8 = self.read8(mmu);
                self.and(d8);
            }
            RST_20H => {
                clock::ticks(16).await;
                self.push(mmu, self.pc);
                self.pc = 0x20;
            }
            ADD_SP_r8 => {
                clock::ticks(16).await;
                let offset = self.read8(mmu);
                let target = self.reg.r16_mut(SP);
                *target = (*target as i16 + offset as i16) as u16;
            }
            JP_xHLx => {
                clock::ticks(4).await;
                self.pc = self.reg.r16(HL);
            }
            LD_xa16x_A => {
                clock::ticks(16).await;
                let addr = self.read16(mmu);
                mmu.write8(addr, self.reg.r8(A));
            }
            NOT_USED_4 => {
                panic!("Attempted to execute unused instruction");
            }
            NOT_USED_5 => {
                panic!("Attempted to execute unused instruction");
            }
            NOT_USED_6 => {
                panic!("Attempted to execute unused instruction");
            }
            XOR_d8 => {
                clock::ticks(8).await;
                let d8 = self.read8(mmu);
                self.xor(d8);
            }
            RST_28H => {
                clock::ticks(16).await;
                self.push(mmu, self.pc);
                self.pc = 0x28;
            }
            LDH_A_xa8x => {
                clock::ticks(12).await;
                let offset = self.read8(mmu);
                *self.reg.r8_mut(A) = mmu.read8(0xFF00 + offset as u16);
            }
            POP_AF => {
                clock::ticks(12).await;
                let af = self.pop(mmu);
                *self.reg.r8_mut(A) = (af >> 4) as u8;
                self.flags.set_from_u8(af as u8);
            }
            LD_A_xCx => {
                clock::ticks(8).await;
                *self.reg.r8_mut(A) = mmu.read8(0xFF00 + self.reg.r8(C) as u16);
            }
            DI => {
                clock::ticks(4).await;
                self.interrupts_enabled = false;
            }
            NOT_USED_7 => {
                panic!("Attempted to execute unused instruction");
            }
            PUSH_AF => {
                clock::ticks(16).await;
                let af = ((self.reg.r8(A) as u16) << 4) + self.flags.as_u8() as u16;
                self.push(mmu, af);
            }
            OR_d8 => {
                clock::ticks(8).await;
                let d8 = self.read8(mmu);
                self.or(d8);
            }
            RST_30H => {
                clock::ticks(16).await;
                self.push(mmu, self.pc);
                self.pc = 0x30;
            }
            LD_HL_SPpr8 => {
                clock::ticks(12).await;
                *self.reg.r16_mut(HL) = (self.reg.r16(SP) as i16 + self.read8(mmu) as i16) as u16;
            }
            LD_SP_HL => {
                clock::ticks(8).await;
                *self.reg.r16_mut(SP) = self.reg.r16(HL);
            }
            LD_A_xa16x => {
                clock::ticks(16).await;
                let addr = self.read16(mmu);
                *self.reg.r8_mut(A) = mmu.read8(addr);
            }
            EI => {
                clock::ticks(4).await;
                // TODO: According to https://www.reddit.com/r/EmuDev/comments/7rm8l2/game_boy_vblank_interrupt_confusion/
                // Interrupts are enabled on after the instruction AFTER this one, not immediately
                // TODO: Check if the same is true for DI
                self.interrupts_enabled = true;
            }
            NOT_USED_8 => {
                panic!("Attempted to execute unused instruction");
            }
            NOT_USED_9 => {
                panic!("Attempted to execute unused instruction");
            }
            CP_d8 => {
                clock::ticks(8).await;
                let d8 = self.read8(mmu);
                self.cp(d8);
            }
            RST_38H => {
                clock::ticks(16).await;
                self.push(mmu, self.pc);
                self.pc = 0x38;
            }
        }
    }

    fn rlc<O: Into<Operand>>(&mut self, mmu: &mut MMU, target: O) {
        let mut target = target.into();
        let old = target.read(self, mmu);

        target.write(self, mmu, old.rotate_left(1));

        self.flags[Flag::Z] = old == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = false;
        self.flags[Flag::C] = old & 0b1000_0000 != 0;
    }

    fn rrc<O: Into<Operand>>(&mut self, mmu: &mut MMU, target: O) {
        let mut target = target.into();
        let old = target.read(self, mmu);

        target.write(self, mmu, old.rotate_right(1));

        self.flags[Flag::Z] = old == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = false;
        self.flags[Flag::C] = old & 0b1 != 0;
    }

    fn rl<O: Into<Operand>>(&mut self, mmu: &mut MMU, target: O) {
        let c = self.flags[Flag::C];
        let mut target = target.into();
        let old = target.read(self, mmu);

        let mut new = old << 1;
        if c {
            new += 1;
        }

        target.write(self, mmu, new);

        self.flags[Flag::Z] = new == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = false;
        self.flags[Flag::C] = old & 0b1000_0000 != 0;
    }

    fn rr<O: Into<Operand>>(&mut self, mmu: &mut MMU, target: O) {
        let c = self.flags[Flag::C];
        let mut target = target.into();
        let old = target.read(self, mmu);

        let mut new = old >> 1;
        if c {
            new += 0b1000_0000;
        }

        target.write(self, mmu, new);

        self.flags[Flag::Z] = new == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = false;
        self.flags[Flag::C] = old & 0b1 != 0;
    }

    fn sla<O: Into<Operand>>(&mut self, mmu: &mut MMU, target: O) {
        let mut target = target.into();
        let old = target.read(self, mmu);

        let new = old << 1;

        target.write(self, mmu, new);

        self.flags[Flag::Z] = new == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = false;
        self.flags[Flag::C] = old & 0b1000_0000 != 0;
    }

    fn sra<O: Into<Operand>>(&mut self, mmu: &mut MMU, target: O) {
        let mut target = target.into();
        let old = target.read(self, mmu);

        let mut new = old >> 1;
        new |= old & 0b1000_0000;

        target.write(self, mmu, new);
        self.flags[Flag::Z] = new == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = false;
        self.flags[Flag::C] = old & 0b1 != 0;
    }

    fn srl<O: Into<Operand>>(&mut self, mmu: &mut MMU, target: O) {
        let mut target = target.into();
        let old = target.read(self, mmu);

        let new = old >> 1;

        self.flags[Flag::Z] = new == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = false;
        self.flags[Flag::C] = old & 0b1 != 0;
    }

    fn swap<O: Into<Operand>>(&mut self, mmu: &mut MMU, target: O) {
        let mut target = target.into();
        let old = target.read(self, mmu);

        let new = (old >> 4) + (old << 4);
        target.write(self, mmu, new);

        self.flags[Flag::Z] = new == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = false;
        self.flags[Flag::C] = false;
    }

    fn bit(&mut self, bit: u8, target: u8) {
        self.flags[Flag::Z] = (target >> bit) & 0b1 == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = true;
    }

    fn res<O: Into<Operand>>(&mut self, mmu: &mut MMU, bit: u8, target: O) {
        let mut target = target.into();
        let old = target.read(self, mmu);
        target.write(self, mmu, old & !(1 << bit));
    }

    fn set<O: Into<Operand>>(&mut self, mmu: &mut MMU, bit: u8, target: O) {
        let mut target = target.into();
        let old = target.read(self, mmu);
        target.write(self, mmu, old | 1 << bit);
    }

    async fn execute_cb(&mut self, instruction: CBInstruction, mmu: &mut MMU<'_>) {
        use CBInstruction::*;
        use R16::*;
        use R8::*;

        // println!(
        //     "Executing Prefixed {:?} with PC now at {:#06X}",
        //     instruction, self.pc
        // );

        match instruction {
            RLC_B => {
                clock::ticks(8).await;
                self.rlc(mmu, B);
            }
            RLC_C => {
                clock::ticks(8).await;
                self.rlc(mmu, C);
            }
            RLC_D => {
                clock::ticks(8).await;
                self.rlc(mmu, D);
            }
            RLC_E => {
                clock::ticks(8).await;
                self.rlc(mmu, E);
            }
            RLC_H => {
                clock::ticks(8).await;
                self.rlc(mmu, H);
            }
            RLC_L => {
                clock::ticks(8).await;
                self.rlc(mmu, L);
            }
            RLC_xHLx => {
                clock::ticks(16).await;
                self.rlc(mmu, Operand::HLAddr);
            }
            RLC_A => {
                clock::ticks(8).await;
                self.rlc(mmu, A);
            }
            RRC_B => {
                clock::ticks(8).await;
                self.rrc(mmu, B);
            }
            RRC_C => {
                clock::ticks(8).await;
                self.rrc(mmu, C);
            }
            RRC_D => {
                clock::ticks(8).await;
                self.rrc(mmu, D);
            }
            RRC_E => {
                clock::ticks(8).await;
                self.rrc(mmu, E);
            }
            RRC_H => {
                clock::ticks(8).await;
                self.rrc(mmu, H);
            }
            RRC_L => {
                clock::ticks(8).await;
                self.rrc(mmu, L);
            }
            RRC_xHLx => {
                clock::ticks(16).await;
                self.rrc(mmu, Operand::HLAddr);
            }
            RRC_A => {
                clock::ticks(8).await;
                self.rrc(mmu, A);
            }
            RL_B => {
                clock::ticks(8).await;
                self.rl(mmu, B);
            }
            RL_C => {
                clock::ticks(8).await;
                self.rl(mmu, C);
            }
            RL_D => {
                clock::ticks(8).await;
                self.rl(mmu, D);
            }
            RL_E => {
                clock::ticks(8).await;
                self.rl(mmu, E);
            }
            RL_H => {
                clock::ticks(8).await;
                self.rl(mmu, H);
            }
            RL_L => {
                clock::ticks(8).await;
                self.rl(mmu, L);
            }
            RL_xHLx => {
                clock::ticks(16).await;
                self.rl(mmu, Operand::HLAddr);
            }
            RL_A => {
                clock::ticks(8).await;
                self.rl(mmu, A);
            }
            RR_B => {
                clock::ticks(8).await;
                self.rr(mmu, B);
            }
            RR_C => {
                clock::ticks(8).await;
                self.rr(mmu, C);
            }
            RR_D => {
                clock::ticks(8).await;
                self.rr(mmu, D);
            }
            RR_E => {
                clock::ticks(8).await;
                self.rr(mmu, E);
            }
            RR_H => {
                clock::ticks(8).await;
                self.rr(mmu, H);
            }
            RR_L => {
                clock::ticks(8).await;
                self.rr(mmu, L);
            }
            RR_xHLx => {
                clock::ticks(16).await;
                self.rr(mmu, Operand::HLAddr);
            }
            RR_A => {
                clock::ticks(8).await;
                self.rr(mmu, A);
            }
            SLA_B => {
                clock::ticks(8).await;
                self.sla(mmu, B);
            }
            SLA_C => {
                clock::ticks(8).await;
                self.sla(mmu, C);
            }
            SLA_D => {
                clock::ticks(8).await;
                self.sla(mmu, D);
            }
            SLA_E => {
                clock::ticks(8).await;
                self.sla(mmu, E);
            }
            SLA_H => {
                clock::ticks(8).await;
                self.sla(mmu, H);
            }
            SLA_L => {
                clock::ticks(8).await;
                self.sla(mmu, L);
            }
            SLA_xHLx => {
                clock::ticks(16).await;
                self.sla(mmu, Operand::HLAddr);
            }
            SLA_A => {
                clock::ticks(8).await;
                self.sla(mmu, A);
            }
            SRA_B => {
                clock::ticks(8).await;
                self.sra(mmu, B);
            }
            SRA_C => {
                clock::ticks(8).await;
                self.sra(mmu, C);
            }
            SRA_D => {
                clock::ticks(8).await;
                self.sra(mmu, D);
            }
            SRA_E => {
                clock::ticks(8).await;
                self.sra(mmu, E);
            }
            SRA_H => {
                clock::ticks(8).await;
                self.sra(mmu, H);
            }
            SRA_L => {
                clock::ticks(8).await;
                self.sra(mmu, L);
            }
            SRA_xHLx => {
                clock::ticks(16).await;
                self.sra(mmu, Operand::HLAddr);
            }
            SRA_A => {
                clock::ticks(8).await;
                self.sra(mmu, A);
            }
            SWAP_B => {
                clock::ticks(8).await;
                self.swap(mmu, B);
            }
            SWAP_C => {
                clock::ticks(8).await;
                self.swap(mmu, C);
            }
            SWAP_D => {
                clock::ticks(8).await;
                self.swap(mmu, D);
            }
            SWAP_E => {
                clock::ticks(8).await;
                self.swap(mmu, E);
            }
            SWAP_H => {
                clock::ticks(8).await;
                self.swap(mmu, H);
            }
            SWAP_L => {
                clock::ticks(8).await;
                self.swap(mmu, L);
            }
            SWAP_xHLx => {
                clock::ticks(16).await;
                self.swap(mmu, Operand::HLAddr);
            }
            SWAP_A => {
                clock::ticks(8).await;
                self.swap(mmu, A);
            }
            SRL_B => {
                clock::ticks(8).await;
                self.srl(mmu, B);
            }
            SRL_C => {
                clock::ticks(8).await;
                self.srl(mmu, C);
            }
            SRL_D => {
                clock::ticks(8).await;
                self.srl(mmu, D);
            }
            SRL_E => {
                clock::ticks(8).await;
                self.srl(mmu, E);
            }
            SRL_H => {
                clock::ticks(8).await;
                self.srl(mmu, H);
            }
            SRL_L => {
                clock::ticks(8).await;
                self.srl(mmu, L);
            }
            SRL_xHLx => {
                clock::ticks(16).await;
                self.srl(mmu, Operand::HLAddr);
            }
            SRL_A => {
                clock::ticks(8).await;
                self.srl(mmu, A);
            }
            BIT_0_B => {
                clock::ticks(8).await;
                self.bit(0, self.reg.r8(B));
            }
            BIT_0_C => {
                clock::ticks(8).await;
                self.bit(0, self.reg.r8(C));
            }
            BIT_0_D => {
                clock::ticks(8).await;
                self.bit(0, self.reg.r8(D));
            }
            BIT_0_E => {
                clock::ticks(8).await;
                self.bit(0, self.reg.r8(E));
            }
            BIT_0_H => {
                clock::ticks(8).await;
                self.bit(0, self.reg.r8(H));
            }
            BIT_0_L => {
                clock::ticks(8).await;
                self.bit(0, self.reg.r8(L));
            }
            BIT_0_xHLx => {
                clock::ticks(16).await;
                self.bit(0, mmu.read8(self.reg.r16(HL)));
            }
            BIT_0_A => {
                clock::ticks(8).await;
                self.bit(0, self.reg.r8(A));
            }
            BIT_1_B => {
                clock::ticks(8).await;
                self.bit(1, self.reg.r8(B));
            }
            BIT_1_C => {
                clock::ticks(8).await;
                self.bit(1, self.reg.r8(C));
            }
            BIT_1_D => {
                clock::ticks(8).await;
                self.bit(1, self.reg.r8(D));
            }
            BIT_1_E => {
                clock::ticks(8).await;
                self.bit(1, self.reg.r8(E));
            }
            BIT_1_H => {
                clock::ticks(8).await;
                self.bit(1, self.reg.r8(H));
            }
            BIT_1_L => {
                clock::ticks(8).await;
                self.bit(1, self.reg.r8(L));
            }
            BIT_1_xHLx => {
                clock::ticks(16).await;
                self.bit(1, mmu.read8(self.reg.r16(HL)));
            }
            BIT_1_A => {
                clock::ticks(8).await;
                self.bit(1, self.reg.r8(A));
            }
            BIT_2_B => {
                clock::ticks(8).await;
                self.bit(2, self.reg.r8(B));
            }
            BIT_2_C => {
                clock::ticks(8).await;
                self.bit(2, self.reg.r8(C));
            }
            BIT_2_D => {
                clock::ticks(8).await;
                self.bit(2, self.reg.r8(D));
            }
            BIT_2_E => {
                clock::ticks(8).await;
                self.bit(2, self.reg.r8(E));
            }
            BIT_2_H => {
                clock::ticks(8).await;
                self.bit(2, self.reg.r8(H));
            }
            BIT_2_L => {
                clock::ticks(8).await;
                self.bit(2, self.reg.r8(L));
            }
            BIT_2_xHLx => {
                clock::ticks(16).await;
                self.bit(2, mmu.read8(self.reg.r16(HL)));
            }
            BIT_2_A => {
                clock::ticks(8).await;
                self.bit(2, self.reg.r8(A));
            }
            BIT_3_B => {
                clock::ticks(8).await;
                self.bit(3, self.reg.r8(B));
            }
            BIT_3_C => {
                clock::ticks(8).await;
                self.bit(3, self.reg.r8(C));
            }
            BIT_3_D => {
                clock::ticks(8).await;
                self.bit(3, self.reg.r8(D));
            }
            BIT_3_E => {
                clock::ticks(8).await;
                self.bit(3, self.reg.r8(E));
            }
            BIT_3_H => {
                clock::ticks(8).await;
                self.bit(3, self.reg.r8(H));
            }
            BIT_3_L => {
                clock::ticks(8).await;
                self.bit(3, self.reg.r8(L));
            }
            BIT_3_xHLx => {
                clock::ticks(16).await;
                self.bit(3, mmu.read8(self.reg.r16(HL)));
            }
            BIT_3_A => {
                clock::ticks(8).await;
                self.bit(3, self.reg.r8(A));
            }
            BIT_4_B => {
                clock::ticks(8).await;
                self.bit(4, self.reg.r8(B));
            }
            BIT_4_C => {
                clock::ticks(8).await;
                self.bit(4, self.reg.r8(C));
            }
            BIT_4_D => {
                clock::ticks(8).await;
                self.bit(4, self.reg.r8(D));
            }
            BIT_4_E => {
                clock::ticks(8).await;
                self.bit(4, self.reg.r8(E));
            }
            BIT_4_H => {
                clock::ticks(8).await;
                self.bit(4, self.reg.r8(H));
            }
            BIT_4_L => {
                clock::ticks(8).await;
                self.bit(4, self.reg.r8(L));
            }
            BIT_4_xHLx => {
                clock::ticks(16).await;
                self.bit(4, mmu.read8(self.reg.r16(HL)));
            }
            BIT_4_A => {
                clock::ticks(8).await;
                self.bit(4, self.reg.r8(A));
            }
            BIT_5_B => {
                clock::ticks(8).await;
                self.bit(5, self.reg.r8(B));
            }
            BIT_5_C => {
                clock::ticks(8).await;
                self.bit(5, self.reg.r8(C));
            }
            BIT_5_D => {
                clock::ticks(8).await;
                self.bit(5, self.reg.r8(D));
            }
            BIT_5_E => {
                clock::ticks(8).await;
                self.bit(5, self.reg.r8(E));
            }
            BIT_5_H => {
                clock::ticks(8).await;
                self.bit(5, self.reg.r8(H));
            }
            BIT_5_L => {
                clock::ticks(8).await;
                self.bit(5, self.reg.r8(L));
            }
            BIT_5_xHLx => {
                clock::ticks(16).await;
                self.bit(5, mmu.read8(self.reg.r16(HL)));
            }
            BIT_5_A => {
                clock::ticks(8).await;
                self.bit(5, self.reg.r8(A));
            }
            BIT_6_B => {
                clock::ticks(8).await;
                self.bit(6, self.reg.r8(B));
            }
            BIT_6_C => {
                clock::ticks(8).await;
                self.bit(6, self.reg.r8(C));
            }
            BIT_6_D => {
                clock::ticks(8).await;
                self.bit(6, self.reg.r8(D));
            }
            BIT_6_E => {
                clock::ticks(8).await;
                self.bit(6, self.reg.r8(E));
            }
            BIT_6_H => {
                clock::ticks(8).await;
                self.bit(6, self.reg.r8(H));
            }
            BIT_6_L => {
                clock::ticks(8).await;
                self.bit(6, self.reg.r8(L));
            }
            BIT_6_xHLx => {
                clock::ticks(16).await;
                self.bit(6, mmu.read8(self.reg.r16(HL)));
            }
            BIT_6_A => {
                clock::ticks(8).await;
                self.bit(6, self.reg.r8(A));
            }
            BIT_7_B => {
                clock::ticks(8).await;
                self.bit(7, self.reg.r8(B));
            }
            BIT_7_C => {
                clock::ticks(8).await;
                self.bit(7, self.reg.r8(C));
            }
            BIT_7_D => {
                clock::ticks(8).await;
                self.bit(7, self.reg.r8(D));
            }
            BIT_7_E => {
                clock::ticks(8).await;
                self.bit(7, self.reg.r8(E));
            }
            BIT_7_H => {
                clock::ticks(8).await;
                self.bit(7, self.reg.r8(H));
            }
            BIT_7_L => {
                clock::ticks(8).await;
                self.bit(7, self.reg.r8(L));
            }
            BIT_7_xHLx => {
                clock::ticks(16).await;
                self.bit(7, mmu.read8(self.reg.r16(HL)));
            }
            BIT_7_A => {
                clock::ticks(8).await;
                self.bit(7, self.reg.r8(A));
            }
            RES_0_B => {
                clock::ticks(8).await;
                self.res(mmu, 0, B);
            }
            RES_0_C => {
                clock::ticks(8).await;
                self.res(mmu, 0, C);
            }
            RES_0_D => {
                clock::ticks(8).await;
                self.res(mmu, 0, D);
            }
            RES_0_E => {
                clock::ticks(8).await;
                self.res(mmu, 0, E);
            }
            RES_0_H => {
                clock::ticks(8).await;
                self.res(mmu, 0, H);
            }
            RES_0_L => {
                clock::ticks(8).await;
                self.res(mmu, 0, L);
            }
            RES_0_xHLx => {
                clock::ticks(16).await;
                self.res(mmu, 0, Operand::HLAddr);
            }
            RES_0_A => {
                clock::ticks(8).await;
                self.res(mmu, 0, A);
            }
            RES_1_B => {
                clock::ticks(8).await;
                self.res(mmu, 1, B);
            }
            RES_1_C => {
                clock::ticks(8).await;
                self.res(mmu, 1, C);
            }
            RES_1_D => {
                clock::ticks(8).await;
                self.res(mmu, 1, D);
            }
            RES_1_E => {
                clock::ticks(8).await;
                self.res(mmu, 1, E);
            }
            RES_1_H => {
                clock::ticks(8).await;
                self.res(mmu, 1, H);
            }
            RES_1_L => {
                clock::ticks(8).await;
                self.res(mmu, 1, L);
            }
            RES_1_xHLx => {
                clock::ticks(16).await;
                self.res(mmu, 1, Operand::HLAddr);
            }
            RES_1_A => {
                clock::ticks(8).await;
                self.res(mmu, 1, A);
            }
            RES_2_B => {
                clock::ticks(8).await;
                self.res(mmu, 2, B);
            }
            RES_2_C => {
                clock::ticks(8).await;
                self.res(mmu, 2, C);
            }
            RES_2_D => {
                clock::ticks(8).await;
                self.res(mmu, 2, D);
            }
            RES_2_E => {
                clock::ticks(8).await;
                self.res(mmu, 2, E);
            }
            RES_2_H => {
                clock::ticks(8).await;
                self.res(mmu, 2, H);
            }
            RES_2_L => {
                clock::ticks(8).await;
                self.res(mmu, 2, L);
            }
            RES_2_xHLx => {
                clock::ticks(16).await;
                self.res(mmu, 2, Operand::HLAddr);
            }
            RES_2_A => {
                clock::ticks(8).await;
                self.res(mmu, 2, A);
            }
            RES_3_B => {
                clock::ticks(8).await;
                self.res(mmu, 3, B);
            }
            RES_3_C => {
                clock::ticks(8).await;
                self.res(mmu, 3, C);
            }
            RES_3_D => {
                clock::ticks(8).await;
                self.res(mmu, 3, D);
            }
            RES_3_E => {
                clock::ticks(8).await;
                self.res(mmu, 3, E);
            }
            RES_3_H => {
                clock::ticks(8).await;
                self.res(mmu, 3, H);
            }
            RES_3_L => {
                clock::ticks(8).await;
                self.res(mmu, 3, L);
            }
            RES_3_xHLx => {
                clock::ticks(16).await;
                self.res(mmu, 3, Operand::HLAddr);
            }
            RES_3_A => {
                clock::ticks(8).await;
                self.res(mmu, 3, A);
            }
            RES_4_B => {
                clock::ticks(8).await;
                self.res(mmu, 4, B);
            }
            RES_4_C => {
                clock::ticks(8).await;
                self.res(mmu, 4, C);
            }
            RES_4_D => {
                clock::ticks(8).await;
                self.res(mmu, 4, D);
            }
            RES_4_E => {
                clock::ticks(8).await;
                self.res(mmu, 4, E);
            }
            RES_4_H => {
                clock::ticks(8).await;
                self.res(mmu, 4, H);
            }
            RES_4_L => {
                clock::ticks(8).await;
                self.res(mmu, 4, L);
            }
            RES_4_xHLx => {
                clock::ticks(16).await;
                self.res(mmu, 4, Operand::HLAddr);
            }
            RES_4_A => {
                clock::ticks(8).await;
                self.res(mmu, 4, A);
            }
            RES_5_B => {
                clock::ticks(8).await;
                self.res(mmu, 5, B);
            }
            RES_5_C => {
                clock::ticks(8).await;
                self.res(mmu, 5, C);
            }
            RES_5_D => {
                clock::ticks(8).await;
                self.res(mmu, 5, D);
            }
            RES_5_E => {
                clock::ticks(8).await;
                self.res(mmu, 5, E);
            }
            RES_5_H => {
                clock::ticks(8).await;
                self.res(mmu, 5, H);
            }
            RES_5_L => {
                clock::ticks(8).await;
                self.res(mmu, 5, L);
            }
            RES_5_xHLx => {
                clock::ticks(16).await;
                self.res(mmu, 5, Operand::HLAddr);
            }
            RES_5_A => {
                clock::ticks(8).await;
                self.res(mmu, 5, A);
            }
            RES_6_B => {
                clock::ticks(8).await;
                self.res(mmu, 6, B);
            }
            RES_6_C => {
                clock::ticks(8).await;
                self.res(mmu, 6, C);
            }
            RES_6_D => {
                clock::ticks(8).await;
                self.res(mmu, 6, D);
            }
            RES_6_E => {
                clock::ticks(8).await;
                self.res(mmu, 6, E);
            }
            RES_6_H => {
                clock::ticks(8).await;
                self.res(mmu, 6, H);
            }
            RES_6_L => {
                clock::ticks(8).await;
                self.res(mmu, 6, L);
            }
            RES_6_xHLx => {
                clock::ticks(16).await;
                self.res(mmu, 6, Operand::HLAddr);
            }
            RES_6_A => {
                clock::ticks(8).await;
                self.res(mmu, 6, A);
            }
            RES_7_B => {
                clock::ticks(8).await;
                self.res(mmu, 7, B);
            }
            RES_7_C => {
                clock::ticks(8).await;
                self.res(mmu, 7, C);
            }
            RES_7_D => {
                clock::ticks(8).await;
                self.res(mmu, 7, D);
            }
            RES_7_E => {
                clock::ticks(8).await;
                self.res(mmu, 7, E);
            }
            RES_7_H => {
                clock::ticks(8).await;
                self.res(mmu, 7, H);
            }
            RES_7_L => {
                clock::ticks(8).await;
                self.res(mmu, 7, L);
            }
            RES_7_xHLx => {
                clock::ticks(16).await;
                self.res(mmu, 7, Operand::HLAddr);
            }
            RES_7_A => {
                clock::ticks(8).await;
                self.res(mmu, 7, A);
            }
            SET_0_B => {
                clock::ticks(8).await;
                self.set(mmu, 0, B);
            }
            SET_0_C => {
                clock::ticks(8).await;
                self.set(mmu, 0, C);
            }
            SET_0_D => {
                clock::ticks(8).await;
                self.set(mmu, 0, D);
            }
            SET_0_E => {
                clock::ticks(8).await;
                self.set(mmu, 0, E);
            }
            SET_0_H => {
                clock::ticks(8).await;
                self.set(mmu, 0, H);
            }
            SET_0_L => {
                clock::ticks(8).await;
                self.set(mmu, 0, L);
            }
            SET_0_xHLx => {
                clock::ticks(16).await;
                self.set(mmu, 0, Operand::HLAddr);
            }
            SET_0_A => {
                clock::ticks(8).await;
                self.set(mmu, 0, A);
            }
            SET_1_B => {
                clock::ticks(8).await;
                self.set(mmu, 1, B);
            }
            SET_1_C => {
                clock::ticks(8).await;
                self.set(mmu, 1, C);
            }
            SET_1_D => {
                clock::ticks(8).await;
                self.set(mmu, 1, D);
            }
            SET_1_E => {
                clock::ticks(8).await;
                self.set(mmu, 1, E);
            }
            SET_1_H => {
                clock::ticks(8).await;
                self.set(mmu, 1, H);
            }
            SET_1_L => {
                clock::ticks(8).await;
                self.set(mmu, 1, L);
            }
            SET_1_xHLx => {
                clock::ticks(16).await;
                self.set(mmu, 1, Operand::HLAddr);
            }
            SET_1_A => {
                clock::ticks(8).await;
                self.set(mmu, 1, A);
            }
            SET_2_B => {
                clock::ticks(8).await;
                self.set(mmu, 2, B);
            }
            SET_2_C => {
                clock::ticks(8).await;
                self.set(mmu, 2, C);
            }
            SET_2_D => {
                clock::ticks(8).await;
                self.set(mmu, 2, D);
            }
            SET_2_E => {
                clock::ticks(8).await;
                self.set(mmu, 2, E);
            }
            SET_2_H => {
                clock::ticks(8).await;
                self.set(mmu, 2, H);
            }
            SET_2_L => {
                clock::ticks(8).await;
                self.set(mmu, 2, L);
            }
            SET_2_xHLx => {
                clock::ticks(16).await;
                self.set(mmu, 2, Operand::HLAddr);
            }
            SET_2_A => {
                clock::ticks(8).await;
                self.set(mmu, 2, A);
            }
            SET_3_B => {
                clock::ticks(8).await;
                self.set(mmu, 3, B);
            }
            SET_3_C => {
                clock::ticks(8).await;
                self.set(mmu, 3, C);
            }
            SET_3_D => {
                clock::ticks(8).await;
                self.set(mmu, 3, D);
            }
            SET_3_E => {
                clock::ticks(8).await;
                self.set(mmu, 3, E);
            }
            SET_3_H => {
                clock::ticks(8).await;
                self.set(mmu, 3, H);
            }
            SET_3_L => {
                clock::ticks(8).await;
                self.set(mmu, 3, L);
            }
            SET_3_xHLx => {
                clock::ticks(16).await;
                self.set(mmu, 3, Operand::HLAddr);
            }
            SET_3_A => {
                clock::ticks(8).await;
                self.set(mmu, 3, A);
            }
            SET_4_B => {
                clock::ticks(8).await;
                self.set(mmu, 4, B);
            }
            SET_4_C => {
                clock::ticks(8).await;
                self.set(mmu, 4, C);
            }
            SET_4_D => {
                clock::ticks(8).await;
                self.set(mmu, 4, D);
            }
            SET_4_E => {
                clock::ticks(8).await;
                self.set(mmu, 4, E);
            }
            SET_4_H => {
                clock::ticks(8).await;
                self.set(mmu, 4, H);
            }
            SET_4_L => {
                clock::ticks(8).await;
                self.set(mmu, 4, L);
            }
            SET_4_xHLx => {
                clock::ticks(16).await;
                self.set(mmu, 4, Operand::HLAddr);
            }
            SET_4_A => {
                clock::ticks(8).await;
                self.set(mmu, 4, A);
            }
            SET_5_B => {
                clock::ticks(8).await;
                self.set(mmu, 5, B);
            }
            SET_5_C => {
                clock::ticks(8).await;
                self.set(mmu, 5, C);
            }
            SET_5_D => {
                clock::ticks(8).await;
                self.set(mmu, 5, D);
            }
            SET_5_E => {
                clock::ticks(8).await;
                self.set(mmu, 5, E);
            }
            SET_5_H => {
                clock::ticks(8).await;
                self.set(mmu, 5, H);
            }
            SET_5_L => {
                clock::ticks(8).await;
                self.set(mmu, 5, L);
            }
            SET_5_xHLx => {
                clock::ticks(16).await;
                self.set(mmu, 5, Operand::HLAddr);
            }
            SET_5_A => {
                clock::ticks(8).await;
                self.set(mmu, 5, A);
            }
            SET_6_B => {
                clock::ticks(8).await;
                self.set(mmu, 6, B);
            }
            SET_6_C => {
                clock::ticks(8).await;
                self.set(mmu, 6, C);
            }
            SET_6_D => {
                clock::ticks(8).await;
                self.set(mmu, 6, D);
            }
            SET_6_E => {
                clock::ticks(8).await;
                self.set(mmu, 6, E);
            }
            SET_6_H => {
                clock::ticks(8).await;
                self.set(mmu, 6, H);
            }
            SET_6_L => {
                clock::ticks(8).await;
                self.set(mmu, 6, L);
            }
            SET_6_xHLx => {
                clock::ticks(16).await;
                self.set(mmu, 6, Operand::HLAddr);
            }
            SET_6_A => {
                clock::ticks(8).await;
                self.set(mmu, 6, A);
            }
            SET_7_B => {
                clock::ticks(8).await;
                self.set(mmu, 7, B);
            }
            SET_7_C => {
                clock::ticks(8).await;
                self.set(mmu, 7, C);
            }
            SET_7_D => {
                clock::ticks(8).await;
                self.set(mmu, 7, D);
            }
            SET_7_E => {
                clock::ticks(8).await;
                self.set(mmu, 7, E);
            }
            SET_7_H => {
                clock::ticks(8).await;
                self.set(mmu, 7, H);
            }
            SET_7_L => {
                clock::ticks(8).await;
                self.set(mmu, 7, L);
            }
            SET_7_xHLx => {
                clock::ticks(16).await;
                self.set(mmu, 7, Operand::HLAddr);
            }
            SET_7_A => {
                clock::ticks(8).await;
                self.set(mmu, 7, A);
            }
        }
    }

    fn decode_interrupt(&mut self, mmu: &mut MMU<'_>, ir: u8) {
        use Interrupt::*;

        for bit in (VBlank as u8)..=(Joypad as u8) {
            if ir & (1 << bit) != 0 {
                self.interrupts_enabled = false;

                self.push(mmu, self.pc);

                let interrupt = unsafe { std::mem::transmute::<u8, Interrupt>(bit) };

                self.pc = match interrupt {
                    VBlank => 0x40,
                    LCD_Stat => 0x48,
                    Timer => 0x50,
                    Serial => 0x58,
                    Joypad => 0x60,
                };

                return;
            }
        }

        unreachable!("Incorrect interrupt handling")
    }
}

#[allow(non_camel_case_types)]
#[repr(u8)]
pub enum Interrupt {
    VBlank,
    LCD_Stat,
    Timer,
    Serial,
    Joypad,
}

#[repr(u8)]
#[derive(Copy, Clone)]
enum R8 {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
}

#[repr(u8)]
#[derive(Copy, Clone)]
enum R16 {
    BC = 1,
    DE = 3,
    HL = 5,
    SP = 7,
}

#[derive(Debug)]
struct Registers([u8; 9]);

impl Registers {
    fn new() -> Registers {
        // Initial values according to bgb.bircd.org/pandocs.htm#powerupsequence
        Registers([0x01, 0x00, 0x13, 0x00, 0xD8, 0x01, 0x4D, 0xFF, 0xFE])
    }

    fn r8(&self, r: R8) -> u8 {
        self.0[r as usize]
    }

    fn r8_mut(&mut self, r: R8) -> &mut u8 {
        &mut self.0[r as usize]
    }

    fn r16(&self, r: R16) -> u16 {
        unsafe { *std::mem::transmute::<&u8, &u16>(&self.0[r as usize]) }
    }

    fn r16_mut(&mut self, r: R16) -> &mut u16 {
        unsafe { std::mem::transmute::<&mut u8, &mut u16>(&mut self.0[r as usize]) }
    }
}

enum Flag {
    /// Operation had a result of 0
    Z,
    /// Operation was a subtraction
    N,
    /// Operation caused overflow from 3rd to 4th bit, or 11th to 12th bit
    H,
    /// Operation caused overflow from 7th to 8th bit, or 15h to 16th bit
    C,
}

#[derive(Debug)]
struct Flags {
    z: bool,
    n: bool,
    h: bool,
    c: bool,
}

impl Flags {
    fn new() -> Flags {
        // Initial values according to bgb.bircd.org/pandocs.htm#powerupsequence
        Flags {
            z: true,
            n: false,
            h: true,
            c: true,
        }
    }

    fn set_from_u8(&mut self, f: u8) {
        self.z = f & 0b1000_0000 != 0;
        self.n = f & 0b0100_0000 != 0;
        self.h = f & 0b0010_0000 != 0;
        self.c = f & 0b0001_0000 != 0;
    }

    fn as_u8(&self) -> u8 {
        let mut val = 0;

        if self.z {
            val += 0b1000_0000;
        }

        if self.n {
            val += 0b0100_0000;
        }

        if self.h {
            val += 0b0010_0000;
        }

        if self.c {
            val += 0b0001_0000;
        }

        val
    }
}

impl Index<Flag> for Flags {
    type Output = bool;

    fn index(&self, flag: Flag) -> &Self::Output {
        match flag {
            Flag::Z => &self.z,
            Flag::N => &self.n,
            Flag::H => &self.h,
            Flag::C => &self.c,
        }
    }
}

impl IndexMut<Flag> for Flags {
    fn index_mut(&mut self, flag: Flag) -> &mut Self::Output {
        match flag {
            Flag::Z => &mut self.z,
            Flag::N => &mut self.n,
            Flag::H => &mut self.h,
            Flag::C => &mut self.c,
        }
    }
}

// TODO: Think about implementing this as an index op for simplicity...
// TODO: Also, this thing is both very elegant and very shitty. Think about fixing it
enum Operand {
    HLAddr,
    Reg(R8),
}

impl Operand {
    fn read(&self, cpu: &mut CPU, mmu: &MMU) -> u8 {
        match self {
            Operand::Reg(r) => cpu.reg.r8(*r),
            Operand::HLAddr => mmu.read8(cpu.reg.r16(R16::HL)),
        }
    }

    fn write(&mut self, cpu: &mut CPU, mmu: &mut MMU, value: u8) {
        match self {
            Operand::Reg(r) => *cpu.reg.r8_mut(*r) = value,
            Operand::HLAddr => mmu.write8(cpu.reg.r16(R16::HL), value),
        }
    }
}

impl From<R8> for Operand {
    fn from(r: R8) -> Self {
        Operand::Reg(r)
    }
}

#[allow(non_camel_case_types, dead_code)]
#[repr(u8)]
#[derive(Debug)]
enum Instruction {
    NOP,
    LD_BC_d16,
    LD_xBCx_A,
    INC_BC,
    INC_B,
    DEC_B,
    LD_B_d8,
    RLCA,
    LD_xa16x_SP,
    ADD_HL_BC,
    LD_A_xBCx,
    DEC_BC,
    INC_C,
    DEC_C,
    LD_C_d8,
    RRCA,
    STOP,
    LD_DE_d16,
    LD_xDEx_A,
    INC_DE,
    INC_D,
    DEC_D,
    LD_D_d8,
    RLA,
    JR_r8,
    ADD_HL_DE,
    LD_A_xDEx,
    DEC_DE,
    INC_E,
    DEC_E,
    LD_E_d8,
    RRA,
    JR_NZ_r8,
    LD_HL_d16,
    LD_xHLix_A,
    INC_HL,
    INC_H,
    DEC_H,
    LD_H_d8,
    DAA,
    JR_Z_r8,
    ADD_HL_HL,
    LD_A_xHLix,
    DEC_HL,
    INC_L,
    DEC_L,
    LD_L_d8,
    CPL,
    JR_NC_r8,
    LD_SP_d16,
    LD_xHLdx_A,
    INC_SP,
    INC_xHLx,
    DEC_xHLx,
    LD_xHLx_d8,
    SCF,
    JR_C_r8,
    ADD_HL_SP,
    LD_A_xHLdx,
    DEC_SP,
    INC_A,
    DEC_A,
    LD_A_d8,
    CCF,
    LD_B_B,
    LD_B_C,
    LD_B_D,
    LD_B_E,
    LD_B_H,
    LD_B_L,
    LD_B_xHLx,
    LD_B_A,
    LD_C_B,
    LD_C_C,
    LD_C_D,
    LD_C_E,
    LD_C_H,
    LD_C_L,
    LD_C_xHLx,
    LD_C_A,
    LD_D_B,
    LD_D_C,
    LD_D_D,
    LD_D_E,
    LD_D_H,
    LD_D_L,
    LD_D_xHLx,
    LD_D_A,
    LD_E_B,
    LD_E_C,
    LD_E_D,
    LD_E_E,
    LD_E_H,
    LD_E_L,
    LD_E_xHLx,
    LD_E_A,
    LD_H_B,
    LD_H_C,
    LD_H_D,
    LD_H_E,
    LD_H_H,
    LD_H_L,
    LD_H_xHLx,
    LD_H_A,
    LD_L_B,
    LD_L_C,
    LD_L_D,
    LD_L_E,
    LD_L_H,
    LD_L_L,
    LD_L_xHLx,
    LD_L_A,
    LD_xHLx_B,
    LD_xHLx_C,
    LD_xHLx_D,
    LD_xHLx_E,
    LD_xHLx_H,
    LD_xHLx_L,
    HALT,
    LD_xHLx_A,
    LD_A_B,
    LD_A_C,
    LD_A_D,
    LD_A_E,
    LD_A_H,
    LD_A_L,
    LD_A_xHLx,
    LD_A_A,
    ADD_A_B,
    ADD_A_C,
    ADD_A_D,
    ADD_A_E,
    ADD_A_H,
    ADD_A_L,
    ADD_A_xHLx,
    ADD_A_A,
    ADC_A_B,
    ADC_A_C,
    ADC_A_D,
    ADC_A_E,
    ADC_A_H,
    ADC_A_L,
    ADC_A_xHLx,
    ADC_A_A,
    SUB_B,
    SUB_C,
    SUB_D,
    SUB_E,
    SUB_H,
    SUB_L,
    SUB_xHLx,
    SUB_A,
    SBC_A_B,
    SBC_A_C,
    SBC_A_D,
    SBC_A_E,
    SBC_A_H,
    SBC_A_L,
    SBC_A_xHLx,
    SBC_A_A,
    AND_B,
    AND_C,
    AND_D,
    AND_E,
    AND_H,
    AND_L,
    AND_xHLx,
    AND_A,
    XOR_B,
    XOR_C,
    XOR_D,
    XOR_E,
    XOR_H,
    XOR_L,
    XOR_xHLx,
    XOR_A,
    OR_B,
    OR_C,
    OR_D,
    OR_E,
    OR_H,
    OR_L,
    OR_xHLx,
    OR_A,
    CP_B,
    CP_C,
    CP_D,
    CP_E,
    CP_H,
    CP_L,
    CP_xHLx,
    CP_A,
    RET_NZ,
    POP_BC,
    JP_NZ_a16,
    JP_a16,
    CALL_NZ_a16,
    PUSH_BC,
    ADD_A_d8,
    RST_00H,
    RET_Z,
    RET,
    JP_Z_a16,
    PREFIX_CB,
    CALL_Z_a16,
    CALL_a16,
    ADC_A_d8,
    RST_08H,
    RET_NC,
    POP_DE,
    JP_NC_a16,
    NOT_USED,
    CALL_NC_a16,
    PUSH_DE,
    SUB_d8,
    RST_10H,
    RET_C,
    RETI,
    JP_C_a16,
    NOT_USED_0,
    CALL_C_a16,
    NOT_USED_1,
    SBC_A_d8,
    RST_18H,
    LDH_xa8x_A,
    POP_HL,
    LD_xCx_A,
    NOT_USED_2,
    NOT_USED_3,
    PUSH_HL,
    AND_d8,
    RST_20H,
    ADD_SP_r8,
    JP_xHLx,
    LD_xa16x_A,
    NOT_USED_4,
    NOT_USED_5,
    NOT_USED_6,
    XOR_d8,
    RST_28H,
    LDH_A_xa8x,
    POP_AF,
    LD_A_xCx,
    DI,
    NOT_USED_7,
    PUSH_AF,
    OR_d8,
    RST_30H,
    LD_HL_SPpr8,
    LD_SP_HL,
    LD_A_xa16x,
    EI,
    NOT_USED_8,
    NOT_USED_9,
    CP_d8,
    RST_38H,
}

/// Preceeded by a 0xCB instruction
#[allow(non_camel_case_types, dead_code)]
#[repr(u8)]
#[derive(Debug)]
enum CBInstruction {
    RLC_B,
    RLC_C,
    RLC_D,
    RLC_E,
    RLC_H,
    RLC_L,
    RLC_xHLx,
    RLC_A,
    RRC_B,
    RRC_C,
    RRC_D,
    RRC_E,
    RRC_H,
    RRC_L,
    RRC_xHLx,
    RRC_A,
    RL_B,
    RL_C,
    RL_D,
    RL_E,
    RL_H,
    RL_L,
    RL_xHLx,
    RL_A,
    RR_B,
    RR_C,
    RR_D,
    RR_E,
    RR_H,
    RR_L,
    RR_xHLx,
    RR_A,
    SLA_B,
    SLA_C,
    SLA_D,
    SLA_E,
    SLA_H,
    SLA_L,
    SLA_xHLx,
    SLA_A,
    SRA_B,
    SRA_C,
    SRA_D,
    SRA_E,
    SRA_H,
    SRA_L,
    SRA_xHLx,
    SRA_A,
    SWAP_B,
    SWAP_C,
    SWAP_D,
    SWAP_E,
    SWAP_H,
    SWAP_L,
    SWAP_xHLx,
    SWAP_A,
    SRL_B,
    SRL_C,
    SRL_D,
    SRL_E,
    SRL_H,
    SRL_L,
    SRL_xHLx,
    SRL_A,
    BIT_0_B,
    BIT_0_C,
    BIT_0_D,
    BIT_0_E,
    BIT_0_H,
    BIT_0_L,
    BIT_0_xHLx,
    BIT_0_A,
    BIT_1_B,
    BIT_1_C,
    BIT_1_D,
    BIT_1_E,
    BIT_1_H,
    BIT_1_L,
    BIT_1_xHLx,
    BIT_1_A,
    BIT_2_B,
    BIT_2_C,
    BIT_2_D,
    BIT_2_E,
    BIT_2_H,
    BIT_2_L,
    BIT_2_xHLx,
    BIT_2_A,
    BIT_3_B,
    BIT_3_C,
    BIT_3_D,
    BIT_3_E,
    BIT_3_H,
    BIT_3_L,
    BIT_3_xHLx,
    BIT_3_A,
    BIT_4_B,
    BIT_4_C,
    BIT_4_D,
    BIT_4_E,
    BIT_4_H,
    BIT_4_L,
    BIT_4_xHLx,
    BIT_4_A,
    BIT_5_B,
    BIT_5_C,
    BIT_5_D,
    BIT_5_E,
    BIT_5_H,
    BIT_5_L,
    BIT_5_xHLx,
    BIT_5_A,
    BIT_6_B,
    BIT_6_C,
    BIT_6_D,
    BIT_6_E,
    BIT_6_H,
    BIT_6_L,
    BIT_6_xHLx,
    BIT_6_A,
    BIT_7_B,
    BIT_7_C,
    BIT_7_D,
    BIT_7_E,
    BIT_7_H,
    BIT_7_L,
    BIT_7_xHLx,
    BIT_7_A,
    RES_0_B,
    RES_0_C,
    RES_0_D,
    RES_0_E,
    RES_0_H,
    RES_0_L,
    RES_0_xHLx,
    RES_0_A,
    RES_1_B,
    RES_1_C,
    RES_1_D,
    RES_1_E,
    RES_1_H,
    RES_1_L,
    RES_1_xHLx,
    RES_1_A,
    RES_2_B,
    RES_2_C,
    RES_2_D,
    RES_2_E,
    RES_2_H,
    RES_2_L,
    RES_2_xHLx,
    RES_2_A,
    RES_3_B,
    RES_3_C,
    RES_3_D,
    RES_3_E,
    RES_3_H,
    RES_3_L,
    RES_3_xHLx,
    RES_3_A,
    RES_4_B,
    RES_4_C,
    RES_4_D,
    RES_4_E,
    RES_4_H,
    RES_4_L,
    RES_4_xHLx,
    RES_4_A,
    RES_5_B,
    RES_5_C,
    RES_5_D,
    RES_5_E,
    RES_5_H,
    RES_5_L,
    RES_5_xHLx,
    RES_5_A,
    RES_6_B,
    RES_6_C,
    RES_6_D,
    RES_6_E,
    RES_6_H,
    RES_6_L,
    RES_6_xHLx,
    RES_6_A,
    RES_7_B,
    RES_7_C,
    RES_7_D,
    RES_7_E,
    RES_7_H,
    RES_7_L,
    RES_7_xHLx,
    RES_7_A,
    SET_0_B,
    SET_0_C,
    SET_0_D,
    SET_0_E,
    SET_0_H,
    SET_0_L,
    SET_0_xHLx,
    SET_0_A,
    SET_1_B,
    SET_1_C,
    SET_1_D,
    SET_1_E,
    SET_1_H,
    SET_1_L,
    SET_1_xHLx,
    SET_1_A,
    SET_2_B,
    SET_2_C,
    SET_2_D,
    SET_2_E,
    SET_2_H,
    SET_2_L,
    SET_2_xHLx,
    SET_2_A,
    SET_3_B,
    SET_3_C,
    SET_3_D,
    SET_3_E,
    SET_3_H,
    SET_3_L,
    SET_3_xHLx,
    SET_3_A,
    SET_4_B,
    SET_4_C,
    SET_4_D,
    SET_4_E,
    SET_4_H,
    SET_4_L,
    SET_4_xHLx,
    SET_4_A,
    SET_5_B,
    SET_5_C,
    SET_5_D,
    SET_5_E,
    SET_5_H,
    SET_5_L,
    SET_5_xHLx,
    SET_5_A,
    SET_6_B,
    SET_6_C,
    SET_6_D,
    SET_6_E,
    SET_6_H,
    SET_6_L,
    SET_6_xHLx,
    SET_6_A,
    SET_7_B,
    SET_7_C,
    SET_7_D,
    SET_7_E,
    SET_7_H,
    SET_7_L,
    SET_7_xHLx,
    SET_7_A,
}
