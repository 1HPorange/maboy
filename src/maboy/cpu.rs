#![deny(unused_must_use)]

use super::clock::Clock;
use super::error::Error;
use super::memory::{Memory, MemoryAccessError};
use static_assertions as sa;
use std::ops::{Index, IndexMut};

// https://stackoverflow.com/questions/41353869/length-of-instruction-ld-a-c-in-gameboy-z80-processor
// > And the other thing that bothers me is the fact that STOP length is 2. It is actually just one byte long.
// > There is a hardware bug on Gameboy Classic that causes the instruction following a STOP to be skipped.
// > So Nintendo started to tell developers to add a NOP always after a STOP.
const SKIP_INSTR_AFTER_STOP: bool = true;

pub struct CPU {
    reg: Registers,
    pc: u16,
    flags: Flags,
    mem: Memory, // TODO: Think about if this should sit here
}

impl CPU {
    // TODO: Research these values!
    pub fn new() -> CPU {
        CPU {
            reg: Registers,
            pc: 0,
            flags: Flags {
                z: false,
                n: false,
                h: false,
                c: false,
            },
            mem: Memory::TEMP_NEW(),
        }
    }

    pub async fn run(&mut self, clock: &Clock) -> Result<(), Error> {
        loop {
            let mem = Memory::TEMP_NEW();

            // Safe transmute since we have u8::MAX instructions
            sa::const_assert_eq!(Instruction::RST_38H as u8, std::u8::MAX);
            let instruction: Instruction = unsafe { std::mem::transmute(self.read8()?) };

            self.execute(clock, instruction).await?;
        }
    }

    fn read8(&mut self) -> Result<u8, Error> {
        let mem = Memory::TEMP_NEW();
        let res = mem.get8(self.pc)?;
        self.pc += 1;
        Ok(res)
    }

    fn read16(&mut self) -> Result<u16, Error> {
        let mem = Memory::TEMP_NEW();
        let res = mem.get16(self.pc)?;
        self.pc += 2;
        Ok(res)
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

    fn inc8<T: Into<Operand>>(&mut self, target: T) -> Result<(), MemoryAccessError> {
        let target = target.into().into_ref(self)?;

        let h = (*target & 0b1111) == 0b1111;

        *target = target.wrapping_add(1);

        self.flags[Flag::Z] = *target == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = h;

        Ok(())
    }

    fn dec8<T: Into<Operand>>(&mut self, target: T) -> Result<(), MemoryAccessError> {
        let target = target.into().into_ref(self)?;

        let h = *target == 0;

        *target = target.wrapping_sub(1);

        self.flags[Flag::Z] = *target == 0;
        self.flags[Flag::N] = true;
        self.flags[Flag::H] = h;

        Ok(())
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
        // TODO: Investigate if we should convert pc to i16
        self.pc = (self.pc as i16 + offset as i16) as u16;
    }

    async fn execute(&mut self, clock: &Clock, instruction: Instruction) -> Result<(), Error> {
        use Instruction::*;
        use R16::*;
        use R8::*;

        let mem = Memory::TEMP_NEW();

        return Ok(match instruction {
            NOP => {
                clock.cycles(4).await;
            }
            LD_BC_d16 => {
                clock.cycles(12).await;
                *self.reg.r16_mut(BC) = self.read16()?;
            }
            LD_xBCx_A => {
                clock.cycles(8).await;
                *mem.get8_mut(self.reg.r16(BC))? = self.reg.r8(A);
            }
            INC_BC => {
                clock.cycles(8).await;
                *self.reg.r16_mut(BC) += 1;
            }
            INC_B => {
                clock.cycles(4).await;
                self.inc8(B)?;
            }
            DEC_B => {
                clock.cycles(4).await;
                self.dec8(B)?;
            }
            LD_B_d8 => {
                clock.cycles(8).await;
                *self.reg.r8_mut(B) = self.read8()?;
            }
            RLCA => {
                clock.cycles(4).await;
                let target = self.reg.r8_mut(A);
                self.flags[Flag::C] = (*target & 0b1000_0000) != 0;
                *target = target.rotate_left(1);
            }
            LD_xa16x_SP => {
                clock.cycles(20).await;
                *mem.get16_mut(self.read16()?)? = self.reg.r16(SP);
            }
            ADD_HL_BC => {
                clock.cycles(8).await;
                self.add_hl(BC).await;
            }
            LD_A_xBCx => {
                clock.cycles(8).await;
                *self.reg.r8_mut(A) = mem.get8(self.reg.r16(BC))?;
            }
            DEC_BC => {
                clock.cycles(8).await;
                *self.reg.r16_mut(BC) -= 1;
            }
            INC_C => {
                clock.cycles(4).await;
                self.inc8(C)?;
            }
            DEC_C => {
                clock.cycles(4).await;
                self.dec8(C)?;
            }
            LD_C_d8 => {
                clock.cycles(8).await;
                *self.reg.r8_mut(C) = self.read8()?;
            }
            RRCA => {
                clock.cycles(4).await;
                let target = self.reg.r8_mut(A);
                self.flags[Flag::C] = (*target & 1) != 0;
                *target = target.rotate_right(1);
            }
            STOP => {
                if SKIP_INSTR_AFTER_STOP {
                    self.read8()?;
                }
                panic!("Reached STOP instruction");
            }
            LD_DE_d16 => {
                clock.cycles(12).await;
                *self.reg.r16_mut(DE) = self.read16()?;
            }
            LD_xDEx_A => {
                clock.cycles(8).await;
                *mem.get8_mut(self.reg.r16(DE))? = self.reg.r8(A);
            }
            INC_DE => {
                clock.cycles(8).await;
                *self.reg.r16_mut(DE) += 1;
            }
            INC_D => {
                clock.cycles(4).await;
                self.inc8(D)?;
            }
            DEC_D => {
                clock.cycles(4).await;
                self.dec8(D)?;
            }
            LD_D_d8 => {
                clock.cycles(8).await;
                *self.reg.r8_mut(D) = self.read8()?;
            }
            RLA => {
                clock.cycles(4).await;
                let target = self.reg.r8_mut(A);
                self.flags[Flag::C] = (*target & 0b1000_0000) != 0;
                *target <<= 1;
            }
            JR_r8 => {
                clock.cycles(12).await;
                let offset = self.read8()?;
                self.jmpr(offset);
            }
            ADD_HL_DE => {
                clock.cycles(8).await;
                self.add_hl(DE).await;
            }
            LD_A_xDEx => {
                clock.cycles(8).await;
                *self.reg.r8_mut(A) = mem.get8(self.reg.r16(DE))?;
            }
            DEC_DE => {
                clock.cycles(8).await;
                *self.reg.r16_mut(DE) -= 1;
            }
            INC_E => {
                clock.cycles(4).await;
                self.inc8(E)?;
            }
            DEC_E => {
                clock.cycles(4).await;
                self.dec8(E)?;
            }
            LD_E_d8 => {
                clock.cycles(8).await;
                *self.reg.r8_mut(E) = self.read8()?;
            }
            RRA => {
                clock.cycles(4).await;
                let target = self.reg.r8_mut(A);
                self.flags[Flag::C] = (*target & 0b1000_0000) != 0;
                *target >>= 1;
            }
            JR_NZ_r8 => {
                clock.cycles(8).await;
                if !self.flags[Flag::Z] {
                    clock.cycles(4).await;
                    let offset = self.read8()?;
                    self.jmpr(offset);
                }
            }
            LD_HL_d16 => {
                clock.cycles(12).await;
                *self.reg.r16_mut(HL) = self.read16()?;
            }
            LD_xHLix_A => {
                clock.cycles(8).await;
                *mem.get8_mut(self.reg.r16(HL))? = self.reg.r8(A);
                *self.reg.r16_mut(HL) += 1;
            }
            INC_HL => {
                clock.cycles(8).await;
                *self.reg.r16_mut(HL) += 1;
            }
            INC_H => {
                clock.cycles(4).await;
                self.inc8(H)?;
            }
            DEC_H => {
                clock.cycles(4).await;
                self.dec8(H)?;
            }
            LD_H_d8 => {
                clock.cycles(8).await;
                *self.reg.r8_mut(H) = self.read8()?;
            }
            // DAA,
            JR_Z_r8 => {
                clock.cycles(8).await;
                if self.flags[Flag::Z] {
                    clock.cycles(4).await;
                    let offset = self.read8()?;
                    self.jmpr(offset);
                }
            }
            ADD_HL_HL => {
                clock.cycles(8).await;
                self.add_hl(HL).await;
            }
            // LD_A_xHLix,
            DEC_HL => {
                clock.cycles(8).await;
                *self.reg.r16_mut(HL) -= 1;
            }
            INC_L => {
                clock.cycles(4).await;
                self.inc8(L)?;
            }
            DEC_L => {
                clock.cycles(4).await;
                self.dec8(L)?;
            }
            LD_L_d8 => {
                clock.cycles(8).await;
                *self.reg.r8_mut(L) = self.read8()?;
            }
            // CPL,
            JR_NC_r8 => {
                clock.cycles(8).await;
                if !self.flags[Flag::C] {
                    clock.cycles(4).await;
                    let offset = self.read8()?;
                    self.jmpr(offset);
                }
            }
            LD_SP_d16 => {
                clock.cycles(12).await;
                *self.reg.r16_mut(SP) = self.read16()?;
            }
            LD_xHLdx_A => {
                clock.cycles(8).await;
                *mem.get8_mut(self.reg.r16(HL))? = self.reg.r8(A);
                *self.reg.r16_mut(HL) -= 1;
            }
            INC_SP => {
                clock.cycles(8).await;
                *self.reg.r16_mut(SP) += 1;
            }
            INC_xHLx => {
                clock.cycles(12).await;
                self.inc8(Operand::HLAddr)?;
            }
            DEC_xHLx => {
                clock.cycles(12).await;
                self.dec8(Operand::HLAddr)?;
            }
            LD_xHLx_d8 => {
                clock.cycles(12).await;
                *mem.get8_mut(self.reg.r16(HL))? = self.read8()?;
            }
            // SCF,
            JR_C_r8 => {
                clock.cycles(8).await;
                if self.flags[Flag::C] {
                    clock.cycles(4).await;
                    let offset = self.read8()?;
                    self.jmpr(offset);
                }
            }
            ADD_HL_SP => {
                clock.cycles(8).await;
                self.add_hl(SP).await;
            }
            // LD_A_xHLdx,
            DEC_SP => {
                clock.cycles(8).await;
                *self.reg.r16_mut(SP) -= 1;
            }
            INC_A => {
                clock.cycles(4).await;
                self.inc8(A)?;
            }
            DEC_A => {
                clock.cycles(4).await;
                self.dec8(A)?;
            }
            LD_A_d8 => {
                clock.cycles(8).await;
                *self.reg.r8_mut(A) = self.read8()?;
            }
            // CCF,
            LD_B_B => {
                clock.cycles(4).await;
                // *self.reg.r8_mut(B) = self.reg.r8(B);
            }
            LD_B_C => {
                clock.cycles(4).await;
                *self.reg.r8_mut(B) = self.reg.r8(C);
            }
            LD_B_D => {
                clock.cycles(4).await;
                *self.reg.r8_mut(B) = self.reg.r8(D);
            }
            LD_B_E => {
                clock.cycles(4).await;
                *self.reg.r8_mut(B) = self.reg.r8(E);
            }
            LD_B_H => {
                clock.cycles(4).await;
                *self.reg.r8_mut(B) = self.reg.r8(H);
            }
            LD_B_L => {
                clock.cycles(4).await;
                *self.reg.r8_mut(B) = self.reg.r8(L);
            }
            LD_B_xHLx => {
                clock.cycles(8).await;
                *self.reg.r8_mut(B) = mem.get8(self.reg.r16(HL))?;
            }
            LD_B_A => {
                clock.cycles(4).await;
                *self.reg.r8_mut(B) = self.reg.r8(A);
            }
            LD_C_B => {
                clock.cycles(4).await;
                *self.reg.r8_mut(C) = self.reg.r8(B);
            }
            LD_C_C => {
                clock.cycles(4).await;
                //*self.reg.r8_mut(C) = self.reg.r8(C);
            }
            LD_C_D => {
                clock.cycles(4).await;
                *self.reg.r8_mut(C) = self.reg.r8(D);
            }
            LD_C_E => {
                clock.cycles(4).await;
                *self.reg.r8_mut(C) = self.reg.r8(E);
            }
            LD_C_H => {
                clock.cycles(4).await;
                *self.reg.r8_mut(C) = self.reg.r8(H);
            }
            LD_C_L => {
                clock.cycles(4).await;
                *self.reg.r8_mut(C) = self.reg.r8(L);
            }
            LD_C_xHLx => {
                clock.cycles(8).await;
                *self.reg.r8_mut(C) = mem.get8(self.reg.r16(HL))?;
            }
            LD_C_A => {
                clock.cycles(4).await;
                *self.reg.r8_mut(C) = self.reg.r8(A);
            }
            LD_D_B => {
                clock.cycles(4).await;
                *self.reg.r8_mut(D) = self.reg.r8(B);
            }
            LD_D_C => {
                clock.cycles(4).await;
                *self.reg.r8_mut(D) = self.reg.r8(C);
            }
            LD_D_D => {
                clock.cycles(4).await;
                // *self.reg.r8_mut(D) = self.reg.r8(D);
            }
            LD_D_E => {
                clock.cycles(4).await;
                *self.reg.r8_mut(D) = self.reg.r8(E);
            }
            LD_D_H => {
                clock.cycles(4).await;
                *self.reg.r8_mut(D) = self.reg.r8(H);
            }
            LD_D_L => {
                clock.cycles(4).await;
                *self.reg.r8_mut(D) = self.reg.r8(L);
            }
            LD_D_xHLx => {
                clock.cycles(8).await;
                *self.reg.r8_mut(D) = mem.get8(self.reg.r16(HL))?;
            }
            LD_D_A => {
                clock.cycles(4).await;
                *self.reg.r8_mut(D) = self.reg.r8(A);
            }
            LD_E_B => {
                clock.cycles(4).await;
                *self.reg.r8_mut(E) = self.reg.r8(B);
            }
            LD_E_C => {
                clock.cycles(4).await;
                *self.reg.r8_mut(E) = self.reg.r8(C);
            }
            LD_E_D => {
                clock.cycles(4).await;
                *self.reg.r8_mut(E) = self.reg.r8(D);
            }
            LD_E_E => {
                clock.cycles(4).await;
                // *self.reg.r8_mut(E) = self.reg.r8(E);
            }
            LD_E_H => {
                clock.cycles(4).await;
                *self.reg.r8_mut(E) = self.reg.r8(H);
            }
            LD_E_L => {
                clock.cycles(4).await;
                *self.reg.r8_mut(E) = self.reg.r8(L);
            }
            LD_E_xHLx => {
                clock.cycles(8).await;
                *self.reg.r8_mut(E) = mem.get8(self.reg.r16(HL))?;
            }
            LD_E_A => {
                clock.cycles(4).await;
                *self.reg.r8_mut(E) = self.reg.r8(A);
            }
            LD_H_B => {
                clock.cycles(4).await;
                *self.reg.r8_mut(H) = self.reg.r8(B);
            }
            LD_H_C => {
                clock.cycles(4).await;
                *self.reg.r8_mut(H) = self.reg.r8(C);
            }
            LD_H_D => {
                clock.cycles(4).await;
                *self.reg.r8_mut(H) = self.reg.r8(D);
            }
            LD_H_E => {
                clock.cycles(4).await;
                *self.reg.r8_mut(H) = self.reg.r8(E);
            }
            LD_H_H => {
                clock.cycles(4).await;
                // *self.reg.r8_mut(H) = self.reg.r8(H);
            }
            LD_H_L => {
                clock.cycles(4).await;
                *self.reg.r8_mut(H) = self.reg.r8(L);
            }
            LD_H_xHLx => {
                clock.cycles(8).await;
                *self.reg.r8_mut(H) = mem.get8(self.reg.r16(HL))?;
            }
            LD_H_A => {
                clock.cycles(4).await;
                *self.reg.r8_mut(H) = self.reg.r8(A);
            }
            LD_L_B => {
                clock.cycles(4).await;
                *self.reg.r8_mut(L) = self.reg.r8(B);
            }
            LD_L_C => {
                clock.cycles(4).await;
                *self.reg.r8_mut(L) = self.reg.r8(C);
            }
            LD_L_D => {
                clock.cycles(4).await;
                *self.reg.r8_mut(L) = self.reg.r8(D);
            }
            LD_L_E => {
                clock.cycles(4).await;
                *self.reg.r8_mut(L) = self.reg.r8(E);
            }
            LD_L_H => {
                clock.cycles(4).await;
                *self.reg.r8_mut(L) = self.reg.r8(H);
            }
            LD_L_L => {
                clock.cycles(4).await;
                // *self.reg.r8_mut(L) = self.reg.r8(L);
            }
            LD_L_xHLx => {
                clock.cycles(8).await;
                *self.reg.r8_mut(L) = mem.get8(self.reg.r16(HL))?;
            }
            LD_L_A => {
                clock.cycles(4).await;
                *self.reg.r8_mut(L) = self.reg.r8(A);
            }
            LD_xHLx_B => {
                clock.cycles(8).await;
                *mem.get8_mut(self.reg.r16(HL))? = self.reg.r8(B);
            }
            LD_xHLx_C => {
                clock.cycles(8).await;
                *mem.get8_mut(self.reg.r16(HL))? = self.reg.r8(C);
            }
            LD_xHLx_D => {
                clock.cycles(8).await;
                *mem.get8_mut(self.reg.r16(HL))? = self.reg.r8(D);
            }
            LD_xHLx_E => {
                clock.cycles(8).await;
                *mem.get8_mut(self.reg.r16(HL))? = self.reg.r8(E);
            }
            LD_xHLx_H => {
                clock.cycles(8).await;
                *mem.get8_mut(self.reg.r16(HL))? = self.reg.r8(H);
            }
            LD_xHLx_L => {
                clock.cycles(8).await;
                *mem.get8_mut(self.reg.r16(HL))? = self.reg.r8(L);
            }
            HALT => {
                panic!("Reached HALT instruction");
            }
            LD_xHLx_A => {
                clock.cycles(8).await;
                *mem.get8_mut(self.reg.r16(HL))? = self.reg.r8(A);
            }
            LD_A_B => {
                clock.cycles(4).await;
                *self.reg.r8_mut(A) = self.reg.r8(B);
            }
            LD_A_C => {
                clock.cycles(4).await;
                *self.reg.r8_mut(A) = self.reg.r8(C);
            }
            LD_A_D => {
                clock.cycles(4).await;
                *self.reg.r8_mut(A) = self.reg.r8(D);
            }
            LD_A_E => {
                clock.cycles(4).await;
                *self.reg.r8_mut(A) = self.reg.r8(E);
            }
            LD_A_H => {
                clock.cycles(4).await;
                *self.reg.r8_mut(A) = self.reg.r8(H);
            }
            LD_A_L => {
                clock.cycles(4).await;
                *self.reg.r8_mut(A) = self.reg.r8(L);
            }
            LD_A_xHLx => {
                clock.cycles(8).await;
                *self.reg.r8_mut(A) = mem.get8(self.reg.r16(HL))?;
            }
            LD_A_A => {
                clock.cycles(4).await;
                // self.reg.r8_mut(A) = self.reg.r8(A);
            }
            ADD_A_B => {
                clock.cycles(4).await;
                self.add(self.reg.r8(B));
            }
            ADD_A_C => {
                clock.cycles(4).await;
                self.add(self.reg.r8(C));
            }
            ADD_A_D => {
                clock.cycles(4).await;
                self.add(self.reg.r8(D));
            }
            ADD_A_E => {
                clock.cycles(4).await;
                self.add(self.reg.r8(E));
            }
            ADD_A_H => {
                clock.cycles(4).await;
                self.add(self.reg.r8(H));
            }
            ADD_A_L => {
                clock.cycles(4).await;
                self.add(self.reg.r8(L));
            }
            ADD_A_xHLx => {
                clock.cycles(8).await;
                self.add(mem.get8(self.reg.r16(HL))?);
            }
            ADD_A_A => {
                clock.cycles(4).await;
                self.add(self.reg.r8(A));
            }
            ADC_A_B => {
                clock.cycles(4).await;
                self.adc(self.reg.r8(B));
            }
            ADC_A_C => {
                clock.cycles(4).await;
                self.adc(self.reg.r8(C));
            }
            ADC_A_D => {
                clock.cycles(4).await;
                self.adc(self.reg.r8(D));
            }
            ADC_A_E => {
                clock.cycles(4).await;
                self.adc(self.reg.r8(E));
            }
            ADC_A_H => {
                clock.cycles(4).await;
                self.adc(self.reg.r8(H));
            }
            ADC_A_L => {
                clock.cycles(4).await;
                self.adc(self.reg.r8(L));
            }
            ADC_A_xHLx => {
                clock.cycles(8).await;
                self.adc(mem.get8(self.reg.r16(HL))?);
            }
            ADC_A_A => {
                clock.cycles(4).await;
                self.adc(self.reg.r8(A));
            }
            SUB_B => {
                clock.cycles(4).await;
                self.sub(self.reg.r8(B));
            }
            SUB_C => {
                clock.cycles(4).await;
                self.sub(self.reg.r8(C));
            }
            SUB_D => {
                clock.cycles(4).await;
                self.sub(self.reg.r8(D));
            }
            SUB_E => {
                clock.cycles(4).await;
                self.sub(self.reg.r8(E));
            }
            SUB_H => {
                clock.cycles(4).await;
                self.sub(self.reg.r8(H));
            }
            SUB_L => {
                clock.cycles(4).await;
                self.sub(self.reg.r8(L));
            }
            SUB_xHLx => {
                clock.cycles(8).await;
                self.sub(mem.get8(self.reg.r16(HL))?);
            }
            SUB_A => {
                clock.cycles(4).await;
                self.sub(self.reg.r8(A));
            }
            SBC_B => {
                clock.cycles(4).await;
                self.sbc(self.reg.r8(B));
            }
            SBC_C => {
                clock.cycles(4).await;
                self.sbc(self.reg.r8(C));
            }
            SBC_D => {
                clock.cycles(4).await;
                self.sbc(self.reg.r8(D));
            }
            SBC_E => {
                clock.cycles(4).await;
                self.sbc(self.reg.r8(E));
            }
            SBC_H => {
                clock.cycles(4).await;
                self.sbc(self.reg.r8(H));
            }
            SBC_L => {
                clock.cycles(4).await;
                self.sbc(self.reg.r8(L));
            }
            SBC_xHLx => {
                clock.cycles(8).await;
                self.sbc(mem.get8(self.reg.r16(HL))?);
            }
            SBC_A => {
                clock.cycles(4).await;
                self.sbc(self.reg.r8(A));
            }
            AND_B => {
                clock.cycles(4).await;
                self.and(self.reg.r8(B));
            }
            AND_C => {
                clock.cycles(4).await;
                self.and(self.reg.r8(C));
            }
            AND_D => {
                clock.cycles(4).await;
                self.and(self.reg.r8(D));
            }
            AND_E => {
                clock.cycles(4).await;
                self.and(self.reg.r8(E));
            }
            AND_H => {
                clock.cycles(4).await;
                self.and(self.reg.r8(H));
            }
            AND_L => {
                clock.cycles(4).await;
                self.and(self.reg.r8(L));
            }
            AND_xHLx => {
                clock.cycles(8).await;
                self.and(mem.get8(self.reg.r16(HL))?);
            }
            AND_A => {
                clock.cycles(4).await;
                self.and(self.reg.r8(A));
            }
            XOR_B => {
                clock.cycles(4).await;
                self.xor(self.reg.r8(B));
            }
            XOR_C => {
                clock.cycles(4).await;
                self.xor(self.reg.r8(C));
            }
            XOR_D => {
                clock.cycles(4).await;
                self.xor(self.reg.r8(D));
            }
            XOR_E => {
                clock.cycles(4).await;
                self.xor(self.reg.r8(E));
            }
            XOR_H => {
                clock.cycles(4).await;
                self.xor(self.reg.r8(H));
            }
            XOR_L => {
                clock.cycles(4).await;
                self.xor(self.reg.r8(L));
            }
            XOR_xHLx => {
                clock.cycles(8).await;
                self.xor(mem.get8(self.reg.r16(HL))?);
            }
            XOR_A => {
                clock.cycles(4).await;
                self.xor(self.reg.r8(A));
            }
            OR_B => {
                clock.cycles(4).await;
                self.or(self.reg.r8(B));
            }
            OR_C => {
                clock.cycles(4).await;
                self.or(self.reg.r8(C));
            }
            OR_D => {
                clock.cycles(4).await;
                self.or(self.reg.r8(D));
            }
            OR_E => {
                clock.cycles(4).await;
                self.or(self.reg.r8(E));
            }
            OR_H => {
                clock.cycles(4).await;
                self.or(self.reg.r8(H));
            }
            OR_L => {
                clock.cycles(4).await;
                self.or(self.reg.r8(L));
            }
            OR_xHLx => {
                clock.cycles(4).await;
                self.or(mem.get8(self.reg.r16(HL))?);
            }
            OR_A => {
                clock.cycles(4).await;
                self.or(self.reg.r8(A));
            }
            CP_B => {
                clock.cycles(4).await;
                self.cp(self.reg.r8(B));
            }
            CP_C => {
                clock.cycles(4).await;
                self.cp(self.reg.r8(C));
            }
            CP_D => {
                clock.cycles(4).await;
                self.cp(self.reg.r8(D));
            }
            CP_E => {
                clock.cycles(4).await;
                self.cp(self.reg.r8(E));
            }
            CP_H => {
                clock.cycles(4).await;
                self.cp(self.reg.r8(H));
            }
            CP_L => {
                clock.cycles(4).await;
                self.cp(self.reg.r8(L));
            }
            CP_xHLx => {
                clock.cycles(8).await;
                self.cp(mem.get8(self.reg.r16(HL))?);
            }
            CP_A => {
                clock.cycles(4).await;
                self.cp(self.reg.r8(A));
            }
            // RET_NZ,
            // POP_BC,
            JP_NZ_a16 => {
                clock.cycles(12).await;
                if !self.flags[Flag::Z] {
                    clock.cycles(4).await;
                    self.pc = self.read16()?;
                }
            }
            JP_a16 => {
                clock.cycles(16).await;
                self.pc = self.read16()?;
            }
            // CALL_NZ_a16,
            // PUSH_BC,
            ADD_A_d8 => {
                clock.cycles(8).await;
                let n = self.read8()?;
                self.add(n);
            }
            // RST_00H,
            // RET_Z,
            // RET,
            JP_Z_a16 => {
                clock.cycles(12).await;
                if self.flags[Flag::Z] {
                    clock.cycles(4).await;
                    self.pc = self.read16()?;
                }
            }
            PREFIX_CB => {
                // Clock cycles are consumed by the prefixed commands to avoid confusion

                sa::const_assert_eq!(CBInstruction::SET_7_A as u8, std::u8::MAX);
                let cb_instruction: CBInstruction = unsafe { std::mem::transmute(self.read8()?) };

                self.execute_cb(clock, cb_instruction).await?;
            }
            // CALL_Z_a16,
            // CALL_a16,
            ADC_A_d8 => {
                clock.cycles(8).await;
                let n = self.read8()?;
                self.adc(n);
            }
            // RST_08H,
            // RET_NC,
            // POP_DE,
            JP_NC_a16 => {
                clock.cycles(12).await;
                if !self.flags[Flag::C] {
                    clock.cycles(4).await;
                    self.pc = self.read16()?;
                }
            }
            NOT_USED => {
                panic!("Attempted to execute unused instruction");
            }
            // CALL_NC_a16,
            // PUSH_DE,
            SUB_d8 => {
                clock.cycles(8).await;
                let n = self.read8()?;
                self.sub(n);
            }
            // RST_10H,
            // RET_C,
            // RETI,
            JP_C_a16 => {
                clock.cycles(12).await;
                if self.flags[Flag::C] {
                    clock.cycles(4).await;
                    self.pc = self.read16()?;
                }
            }
            NOT_USED_0 => {
                panic!("Attempted to execute unused instruction");
            }
            // CALL_C_a16,
            NOT_USED_1 => {
                panic!("Attempted to execute unused instruction");
            }
            SBC_A_d8 => {
                clock.cycles(8).await;
                let n = self.read8()?;
                self.sbc(n);
            }
            // RST_18H,
            // LDH_xa8x_A,
            // POP_HL,
            LD_xCx_A => {
                clock.cycles(8).await;
                *mem.get8_mut(self.reg.r8(C) as u16)? = self.reg.r8(A);
            }
            NOT_USED_2 => {
                panic!("Attempted to execute unused instruction");
            }
            NOT_USED_3 => {
                panic!("Attempted to execute unused instruction");
            }
            // PUSH_HL,
            AND_d8 => {
                clock.cycles(8).await;
                let n = self.read8()?;
                self.and(n);
            }
            // RST_20H,
            ADD_SP_r8 => {
                clock.cycles(16).await;
                let offset = self.read8()?;
                let target = self.reg.r16_mut(SP);
                *target = (*target as i16 + offset as i16) as u16;
            }
            JP_xHLx => {
                clock.cycles(4).await;
                self.pc = self.reg.r16(HL);
            }
            LD_xa16x_A => {
                clock.cycles(16).await;
                *mem.get8_mut(self.read16()?)? = self.reg.r8(A);
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
                clock.cycles(8).await;
                let n = self.read8()?;
                self.xor(n);
            }
            // RST_28H,
            // LDH_A_xa8x,
            // POP_AF,
            LD_A_xCx => {
                clock.cycles(8).await;
                *self.reg.r8_mut(A) = mem.get8(self.reg.r8(C) as u16)?;
            }
            // DI,
            NOT_USED_7 => {
                panic!("Attempted to execute unused instruction");
            }
            // PUSH_AF,
            OR_d8 => {
                clock.cycles(8).await;
                let n = self.read8()?;
                self.or(n);
            }
            // RST_30H,
            // LD_HL_SPpr8,
            LD_SP_HL => {
                clock.cycles(8).await;
                *self.reg.r16_mut(SP) = self.reg.r16(HL);
            }
            LD_A_xa16x => {
                clock.cycles(16).await;
                *self.reg.r8_mut(A) = mem.get8(self.read16()?)?;
            }
            // EI,
            NOT_USED_8 => {
                panic!("Attempted to execute unused instruction");
            }
            NOT_USED_9 => {
                panic!("Attempted to execute unused instruction");
            }
            CP_d8 => {
                clock.cycles(8).await;
                let n = self.read8()?;
                self.cp(n);
            }
            // RST_38H,
            _ => unimplemented!(), // TODO: Removeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
        });
    }

    async fn execute_cb(&mut self, clock: &Clock, instruction: CBInstruction) -> Result<(), Error> {
        match instruction {
            // RLC_B,
            // RLC_C,
            // RLC_D,
            // RLC_E,
            // RLC_H,
            // RLC_L,
            // RLC_xHLx,
            // RLC_A,
            // RRC_B,
            // RRC_C,
            // RRC_D,
            // RRC_E,
            // RRC_H,
            // RRC_L,
            // RRC_xHLx,
            // RRC_A,
            // RL_B,
            // RL_C,
            // RL_D,
            // RL_E,
            // RL_H,
            // RL_L,
            // RL_xHLx,
            // RL_A,
            // RR_B,
            // RR_C,
            // RR_D,
            // RR_E,
            // RR_H,
            // RR_L,
            // RR_xHLx,
            // RR_A,
            // SLA_B,
            // SLA_C,
            // SLA_D,
            // SLA_E,
            // SLA_H,
            // SLA_L,
            // SLA_xHLx,
            // SLA_A,
            // SRA_B,
            // SRA_C,
            // SRA_D,
            // SRA_E,
            // SRA_H,
            // SRA_L,
            // SRA_xHLx,
            // SRA_A,
            // SWAP_B,
            // SWAP_C,
            // SWAP_D,
            // SWAP_E,
            // SWAP_H,
            // SWAP_L,
            // SWAP_xHLx,
            // SWAP_A,
            // SRL_B,
            // SRL_C,
            // SRL_D,
            // SRL_E,
            // SRL_H,
            // SRL_L,
            // SRL_xHLx,
            // SRL_A,
            // BIT_0_B,
            // BIT_0_C,
            // BIT_0_D,
            // BIT_0_E,
            // BIT_0_H,
            // BIT_0_L,
            // BIT_0_xHLx,
            // BIT_0_A,
            // BIT_1_B,
            // BIT_1_C,
            // BIT_1_D,
            // BIT_1_E,
            // BIT_1_H,
            // BIT_1_L,
            // BIT_1_xHLx,
            // BIT_1_A,
            // BIT_2_B,
            // BIT_2_C,
            // BIT_2_D,
            // BIT_2_E,
            // BIT_2_H,
            // BIT_2_L,
            // BIT_2_xHLx,
            // BIT_2_A,
            // BIT_3_B,
            // BIT_3_C,
            // BIT_3_D,
            // BIT_3_E,
            // BIT_3_H,
            // BIT_3_L,
            // BIT_3_xHLx,
            // BIT_3_A,
            // BIT_4_B,
            // BIT_4_C,
            // BIT_4_D,
            // BIT_4_E,
            // BIT_4_H,
            // BIT_4_L,
            // BIT_4_xHLx,
            // BIT_4_A,
            // BIT_5_B,
            // BIT_5_C,
            // BIT_5_D,
            // BIT_5_E,
            // BIT_5_H,
            // BIT_5_L,
            // BIT_5_xHLx,
            // BIT_5_A,
            // BIT_6_B,
            // BIT_6_C,
            // BIT_6_D,
            // BIT_6_E,
            // BIT_6_H,
            // BIT_6_L,
            // BIT_6_xHLx,
            // BIT_6_A,
            // BIT_7_B,
            // BIT_7_C,
            // BIT_7_D,
            // BIT_7_E,
            // BIT_7_H,
            // BIT_7_L,
            // BIT_7_xHLx,
            // BIT_7_A,
            // RES_0_B,
            // RES_0_C,
            // RES_0_D,
            // RES_0_E,
            // RES_0_H,
            // RES_0_L,
            // RES_0_xHLx,
            // RES_0_A,
            // RES_1_B,
            // RES_1_C,
            // RES_1_D,
            // RES_1_E,
            // RES_1_H,
            // RES_1_L,
            // RES_1_xHLx,
            // RES_1_A,
            // RES_2_B,
            // RES_2_C,
            // RES_2_D,
            // RES_2_E,
            // RES_2_H,
            // RES_2_L,
            // RES_2_xHLx,
            // RES_2_A,
            // RES_3_B,
            // RES_3_C,
            // RES_3_D,
            // RES_3_E,
            // RES_3_H,
            // RES_3_L,
            // RES_3_xHLx,
            // RES_3_A,
            // RES_4_B,
            // RES_4_C,
            // RES_4_D,
            // RES_4_E,
            // RES_4_H,
            // RES_4_L,
            // RES_4_xHLx,
            // RES_4_A,
            // RES_5_B,
            // RES_5_C,
            // RES_5_D,
            // RES_5_E,
            // RES_5_H,
            // RES_5_L,
            // RES_5_xHLx,
            // RES_5_A,
            // RES_6_B,
            // RES_6_C,
            // RES_6_D,
            // RES_6_E,
            // RES_6_H,
            // RES_6_L,
            // RES_6_xHLx,
            // RES_6_A,
            // RES_7_B,
            // RES_7_C,
            // RES_7_D,
            // RES_7_E,
            // RES_7_H,
            // RES_7_L,
            // RES_7_xHLx,
            // RES_7_A,
            // SET_0_B,
            // SET_0_C,
            // SET_0_D,
            // SET_0_E,
            // SET_0_H,
            // SET_0_L,
            // SET_0_xHLx,
            // SET_0_A,
            // SET_1_B,
            // SET_1_C,
            // SET_1_D,
            // SET_1_E,
            // SET_1_H,
            // SET_1_L,
            // SET_1_xHLx,
            // SET_1_A,
            // SET_2_B,
            // SET_2_C,
            // SET_2_D,
            // SET_2_E,
            // SET_2_H,
            // SET_2_L,
            // SET_2_xHLx,
            // SET_2_A,
            // SET_3_B,
            // SET_3_C,
            // SET_3_D,
            // SET_3_E,
            // SET_3_H,
            // SET_3_L,
            // SET_3_xHLx,
            // SET_3_A,
            // SET_4_B,
            // SET_4_C,
            // SET_4_D,
            // SET_4_E,
            // SET_4_H,
            // SET_4_L,
            // SET_4_xHLx,
            // SET_4_A,
            // SET_5_B,
            // SET_5_C,
            // SET_5_D,
            // SET_5_E,
            // SET_5_H,
            // SET_5_L,
            // SET_5_xHLx,
            // SET_5_A,
            // SET_6_B,
            // SET_6_C,
            // SET_6_D,
            // SET_6_E,
            // SET_6_H,
            // SET_6_L,
            // SET_6_xHLx,
            // SET_6_A,
            // SET_7_B,
            // SET_7_C,
            // SET_7_D,
            // SET_7_E,
            // SET_7_H,
            // SET_7_L,
            // SET_7_xHLx,
            // SET_7_A,
            _ => unimplemented!(), // TODO: Removeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
        }
    }
}

enum R8 {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
}

enum R16 {
    BC,
    DE,
    HL,
    SP,
}

struct Registers;

impl Registers {
    fn r8(&self, r: R8) -> u8 {
        unimplemented!()
    }

    fn r8_mut(&mut self, r: R8) -> &mut u8 {
        unimplemented!()
    }

    fn r16(&self, r: R16) -> u16 {
        unimplemented!()
    }

    fn r16_mut(&mut self, r: R16) -> &mut u16 {
        unimplemented!()
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

struct Flags {
    z: bool,
    n: bool,
    h: bool,
    c: bool,
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
    fn into_value(self, cpu: &CPU) -> Result<u8, MemoryAccessError> {
        Ok(match self {
            Operand::Reg(r) => cpu.reg.r8(r),
            Operand::HLAddr => cpu.mem.get8(cpu.reg.r16(R16::HL))?,
        })
    }

    fn into_ref(self, cpu: &mut CPU) -> Result<&mut u8, MemoryAccessError> {
        Ok(match self {
            Operand::Reg(r) => cpu.reg.r8_mut(r),
            Operand::HLAddr => cpu.mem.get8_mut(cpu.reg.r16(R16::HL))?,
        })
    }
}

impl From<R8> for Operand {
    fn from(r: R8) -> Self {
        Operand::Reg(r)
    }
}

#[allow(non_camel_case_types, dead_code)]
#[repr(u8)]
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
