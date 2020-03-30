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
    interrupts_enabled: bool, // TODO: Remove, move where it belongs (0xFFFF)
    mem: Memory,              // TODO: Think about if this should sit here
}

impl CPU {
    // TODO: Research these values!
    pub fn new() -> CPU {
        CPU {
            reg: Registers::new(),
            pc: 0,
            flags: Flags::new(),
            interrupts_enabled: false,
            mem: Memory::new(),
        }
    }

    pub async fn run(&mut self, clock: &Clock) -> Result<(), MemoryAccessError> {
        loop {
            // Safe transmute since we have u8::MAX instructions
            sa::const_assert_eq!(Instruction::RST_38H as u8, std::u8::MAX);
            let instruction: Instruction = unsafe { std::mem::transmute(self.read8()?) };

            self.execute(clock, instruction).await?;
        }
    }

    fn read8(&mut self) -> Result<u8, MemoryAccessError> {
        let res = self.mem.get8(self.pc)?;
        self.pc += 1;
        Ok(res)
    }

    fn read16(&mut self) -> Result<u16, MemoryAccessError> {
        let res = self.mem.get16(self.pc)?;
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

    fn inc8<O: Into<Operand>>(&mut self, target: O) -> Result<(), MemoryAccessError> {
        let target = target.into().into_ref(self)?;

        let h = (*target & 0b1111) == 0b1111;

        *target = target.wrapping_add(1);

        self.flags[Flag::Z] = *target == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = h;

        Ok(())
    }

    fn dec8<O: Into<Operand>>(&mut self, target: O) -> Result<(), MemoryAccessError> {
        let target = target.into().into_ref(self)?;

        let h = *target == 0;

        *target = target.wrapping_sub(1);

        self.flags[Flag::Z] = *target == 0;
        self.flags[Flag::N] = true;
        self.flags[Flag::H] = h;

        Ok(())
    }

    fn add<O: Into<Operand>>(&mut self, n: O) -> Result<(), MemoryAccessError> {
        let n = n.into().into_val(self)?;
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

        Ok(())
    }

    fn adc<O: Into<Operand>>(&mut self, n: O) -> Result<(), MemoryAccessError> {
        let n = n.into().into_val(self)?;
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

        Ok(())
    }

    fn sub<O: Into<Operand>>(&mut self, n: O) -> Result<(), MemoryAccessError> {
        *self.reg.r8_mut(R8::A) = self.cp(n)?;
        Ok(())
    }

    fn sbc<O: Into<Operand>>(&mut self, n: O) -> Result<(), MemoryAccessError> {
        let mut n = n.into().into_val(self)?;
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

        Ok(())
    }

    /// Returns A-n, which can be used to implement SUB_n
    fn cp<O: Into<Operand>>(&mut self, n: O) -> Result<u8, MemoryAccessError> {
        let n = n.into().into_val(self)?;
        let reference = self.reg.r8(R8::A);

        let h = n > reference;

        let (diff, c) = reference.overflowing_sub(n);

        self.flags[Flag::Z] = diff == 0;
        self.flags[Flag::N] = true;
        self.flags[Flag::H] = h;
        self.flags[Flag::C] = c;

        Ok(diff)
    }

    fn and<O: Into<Operand>>(&mut self, n: O) -> Result<(), MemoryAccessError> {
        let n = n.into().into_val(self)?;
        let target = self.reg.r8_mut(R8::A);

        *target &= n;

        self.flags[Flag::Z] = *target == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = true;
        self.flags[Flag::C] = false;

        Ok(())
    }

    fn xor<O: Into<Operand>>(&mut self, n: O) -> Result<(), MemoryAccessError> {
        let n = n.into().into_val(self)?;
        let target = self.reg.r8_mut(R8::A);

        *target ^= n;

        self.flags[Flag::Z] = *target == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = false;
        self.flags[Flag::C] = false;

        Ok(())
    }

    fn or<O: Into<Operand>>(&mut self, n: O) -> Result<(), MemoryAccessError> {
        let n = n.into().into_val(self)?;
        let target = self.reg.r8_mut(R8::A);

        *target |= n;

        self.flags[Flag::Z] = *target == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = false;
        self.flags[Flag::C] = false;

        Ok(())
    }

    fn jmpr(&mut self) -> Result<(), MemoryAccessError> {
        // TODO: Investigate if we should convert pc to i16
        self.pc = (self.pc as i16 + self.read8()? as i16) as u16;
        Ok(())
    }

    fn pop(&mut self) -> Result<u16, MemoryAccessError> {
        let sp = self.reg.r16_mut(R16::SP);
        let val = self.mem.get16(*sp)?;
        *sp = sp.wrapping_add(2);
        Ok(val)
    }

    fn push(&mut self, val: u16) -> Result<(), MemoryAccessError> {
        let sp = self.reg.r16_mut(R16::SP);
        *self.mem.get16_mut(*sp)? = val;
        *sp = sp.wrapping_sub(2);
        Ok(())
    }

    async fn execute(
        &mut self,
        clock: &Clock,
        instruction: Instruction,
    ) -> Result<(), MemoryAccessError> {
        use Instruction::*;
        use R16::*;
        use R8::*;

        return Ok(match instruction {
            NOP => {
                clock.cycles(4).await;
                panic!("You did it! You reached NOP!")
            }
            LD_BC_d16 => {
                clock.cycles(12).await;
                *self.reg.r16_mut(BC) = self.read16()?;
            }
            LD_xBCx_A => {
                clock.cycles(8).await;
                *self.mem.get8_mut(self.reg.r16(BC))? = self.reg.r8(A);
            }
            INC_BC => {
                clock.cycles(8).await;
                let bc = self.reg.r16_mut(BC);
                *bc = bc.wrapping_add(1);
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
                let addr = self.read16()?;
                *self.mem.get16_mut(addr)? = self.reg.r16(SP);
            }
            ADD_HL_BC => {
                clock.cycles(8).await;
                self.add_hl(BC).await;
            }
            LD_A_xBCx => {
                clock.cycles(8).await;
                *self.reg.r8_mut(A) = self.mem.get8(self.reg.r16(BC))?;
            }
            DEC_BC => {
                clock.cycles(8).await;
                let bc = self.reg.r16_mut(BC);
                *bc = bc.wrapping_sub(1);
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
                *self.mem.get8_mut(self.reg.r16(DE))? = self.reg.r8(A);
            }
            INC_DE => {
                clock.cycles(8).await;
                let de = self.reg.r16_mut(DE);
                *de = de.wrapping_add(1);
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
                // Can't us self.rl because it sets the zero flag
                clock.cycles(4).await;
                let target = self.reg.r8_mut(A);
                let c = self.flags[Flag::C];
                self.flags[Flag::C] = (*target & 0b1000_0000) != 0;
                *target <<= 1;
                if c {
                    *target += 1;
                }
            }
            JR_r8 => {
                clock.cycles(12).await;
                self.jmpr()?;
            }
            ADD_HL_DE => {
                clock.cycles(8).await;
                self.add_hl(DE).await;
            }
            LD_A_xDEx => {
                clock.cycles(8).await;
                *self.reg.r8_mut(A) = self.mem.get8(self.reg.r16(DE))?;
            }
            DEC_DE => {
                clock.cycles(8).await;
                let de = self.reg.r16_mut(DE);
                *de = de.wrapping_sub(1);
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
                // Can't us self.rr because it sets the zero flag
                clock.cycles(4).await;
                let target = self.reg.r8_mut(A);
                let c = self.flags[Flag::C];
                self.flags[Flag::C] = (*target & 0b1) != 0;
                *target >>= 1;
                if c {
                    *target += 0b1000_0000;
                }
            }
            JR_NZ_r8 => {
                clock.cycles(8).await;
                if !self.flags[Flag::Z] {
                    clock.cycles(4).await;
                    self.jmpr()?;
                }
            }
            LD_HL_d16 => {
                clock.cycles(12).await;
                *self.reg.r16_mut(HL) = self.read16()?;
            }
            LD_xHLix_A => {
                clock.cycles(8).await;
                *self.mem.get8_mut(self.reg.r16(HL))? = self.reg.r8(A);
                let hl = self.reg.r16_mut(HL);
                *hl = hl.wrapping_add(1);
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
            DAA => {
                // DAA is kind of infamous for having complicated behaviour
                // This is why I took the source code from https://forums.nesdev.com/viewtopic.php?t=15944

                clock.cycles(4).await;

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
                clock.cycles(8).await;
                if self.flags[Flag::Z] {
                    clock.cycles(4).await;
                    self.jmpr()?;
                }
            }
            ADD_HL_HL => {
                clock.cycles(8).await;
                self.add_hl(HL).await;
            }
            LD_A_xHLix => {
                clock.cycles(8).await;
                *self.reg.r8_mut(A) = self.mem.get8(self.reg.r16(HL))?;
                let hl = self.reg.r16_mut(HL);
                *hl = hl.wrapping_add(1);
            }
            DEC_HL => {
                clock.cycles(8).await;
                let hl = self.reg.r16_mut(HL);
                *hl = hl.wrapping_sub(1);
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
            CPL => {
                clock.cycles(4).await;
                let a = self.reg.r8_mut(A);
                *a = !*a;
                self.flags[Flag::N] = true;
                self.flags[Flag::H] = true;
            }
            JR_NC_r8 => {
                clock.cycles(8).await;
                if !self.flags[Flag::C] {
                    clock.cycles(4).await;
                    self.jmpr()?;
                }
            }
            LD_SP_d16 => {
                clock.cycles(12).await;
                *self.reg.r16_mut(SP) = self.read16()?;
            }
            LD_xHLdx_A => {
                clock.cycles(8).await;
                *self.mem.get8_mut(self.reg.r16(HL))? = self.reg.r8(A);
                let hl = self.reg.r16_mut(HL);
                *hl = hl.wrapping_sub(1);
            }
            INC_SP => {
                clock.cycles(8).await;
                let sp = self.reg.r16_mut(SP);
                *sp = sp.wrapping_add(1);
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
                *self.mem.get8_mut(self.reg.r16(HL))? = self.read8()?;
            }
            SCF => {
                clock.cycles(4).await;
                self.flags[Flag::N] = false;
                self.flags[Flag::H] = false;
                self.flags[Flag::C] = true;
            }
            JR_C_r8 => {
                clock.cycles(8).await;
                if self.flags[Flag::C] {
                    clock.cycles(4).await;
                    self.jmpr()?;
                }
            }
            ADD_HL_SP => {
                clock.cycles(8).await;
                self.add_hl(SP).await;
            }
            LD_A_xHLdx => {
                clock.cycles(8).await;
                *self.reg.r8_mut(A) = self.mem.get8(self.reg.r16(HL))?;
                let hl = self.reg.r16_mut(HL);
                *hl = hl.wrapping_sub(1);
            }
            DEC_SP => {
                clock.cycles(8).await;
                let sp = self.reg.r16_mut(SP);
                *sp = sp.wrapping_sub(1);
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
            CCF => {
                clock.cycles(4).await;
                self.flags[Flag::N] = false;
                self.flags[Flag::H] = false;
                self.flags[Flag::C] = !self.flags[Flag::C];
            }
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
                *self.reg.r8_mut(B) = self.mem.get8(self.reg.r16(HL))?;
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
                *self.reg.r8_mut(C) = self.mem.get8(self.reg.r16(HL))?;
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
                *self.reg.r8_mut(D) = self.mem.get8(self.reg.r16(HL))?;
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
                *self.reg.r8_mut(E) = self.mem.get8(self.reg.r16(HL))?;
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
                *self.reg.r8_mut(H) = self.mem.get8(self.reg.r16(HL))?;
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
                *self.reg.r8_mut(L) = self.mem.get8(self.reg.r16(HL))?;
            }
            LD_L_A => {
                clock.cycles(4).await;
                *self.reg.r8_mut(L) = self.reg.r8(A);
            }
            LD_xHLx_B => {
                clock.cycles(8).await;
                *self.mem.get8_mut(self.reg.r16(HL))? = self.reg.r8(B);
            }
            LD_xHLx_C => {
                clock.cycles(8).await;
                *self.mem.get8_mut(self.reg.r16(HL))? = self.reg.r8(C);
            }
            LD_xHLx_D => {
                clock.cycles(8).await;
                *self.mem.get8_mut(self.reg.r16(HL))? = self.reg.r8(D);
            }
            LD_xHLx_E => {
                clock.cycles(8).await;
                *self.mem.get8_mut(self.reg.r16(HL))? = self.reg.r8(E);
            }
            LD_xHLx_H => {
                clock.cycles(8).await;
                *self.mem.get8_mut(self.reg.r16(HL))? = self.reg.r8(H);
            }
            LD_xHLx_L => {
                clock.cycles(8).await;
                *self.mem.get8_mut(self.reg.r16(HL))? = self.reg.r8(L);
            }
            HALT => {
                panic!("Reached HALT instruction");
            }
            LD_xHLx_A => {
                clock.cycles(8).await;
                *self.mem.get8_mut(self.reg.r16(HL))? = self.reg.r8(A);
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
                *self.reg.r8_mut(A) = self.mem.get8(self.reg.r16(HL))?;
            }
            LD_A_A => {
                clock.cycles(4).await;
                // self.reg.r8_mut(A) = self.reg.r8(A);
            }
            ADD_A_B => {
                clock.cycles(4).await;
                self.add(B)?;
            }
            ADD_A_C => {
                clock.cycles(4).await;
                self.add(C)?;
            }
            ADD_A_D => {
                clock.cycles(4).await;
                self.add(D)?;
            }
            ADD_A_E => {
                clock.cycles(4).await;
                self.add(E)?;
            }
            ADD_A_H => {
                clock.cycles(4).await;
                self.add(H)?;
            }
            ADD_A_L => {
                clock.cycles(4).await;
                self.add(L)?;
            }
            ADD_A_xHLx => {
                clock.cycles(8).await;
                self.add(Operand::HLAddr)?;
            }
            ADD_A_A => {
                clock.cycles(4).await;
                self.add(A)?;
            }
            ADC_A_B => {
                clock.cycles(4).await;
                self.adc(B)?;
            }
            ADC_A_C => {
                clock.cycles(4).await;
                self.adc(C)?;
            }
            ADC_A_D => {
                clock.cycles(4).await;
                self.adc(D)?;
            }
            ADC_A_E => {
                clock.cycles(4).await;
                self.adc(E)?;
            }
            ADC_A_H => {
                clock.cycles(4).await;
                self.adc(H)?;
            }
            ADC_A_L => {
                clock.cycles(4).await;
                self.adc(L)?;
            }
            ADC_A_xHLx => {
                clock.cycles(8).await;
                self.adc(Operand::HLAddr)?;
            }
            ADC_A_A => {
                clock.cycles(4).await;
                self.adc(A)?;
            }
            SUB_B => {
                clock.cycles(4).await;
                self.sub(B)?;
            }
            SUB_C => {
                clock.cycles(4).await;
                self.sub(C)?;
            }
            SUB_D => {
                clock.cycles(4).await;
                self.sub(D)?;
            }
            SUB_E => {
                clock.cycles(4).await;
                self.sub(E)?;
            }
            SUB_H => {
                clock.cycles(4).await;
                self.sub(H)?;
            }
            SUB_L => {
                clock.cycles(4).await;
                self.sub(L)?;
            }
            SUB_xHLx => {
                clock.cycles(8).await;
                self.sub(Operand::HLAddr)?;
            }
            SUB_A => {
                clock.cycles(4).await;
                self.sub(A)?;
            }
            SBC_A_B => {
                clock.cycles(4).await;
                self.sbc(B)?;
            }
            SBC_A_C => {
                clock.cycles(4).await;
                self.sbc(C)?;
            }
            SBC_A_D => {
                clock.cycles(4).await;
                self.sbc(D)?;
            }
            SBC_A_E => {
                clock.cycles(4).await;
                self.sbc(E)?;
            }
            SBC_A_H => {
                clock.cycles(4).await;
                self.sbc(H)?;
            }
            SBC_A_L => {
                clock.cycles(4).await;
                self.sbc(L)?;
            }
            SBC_A_xHLx => {
                clock.cycles(8).await;
                self.sbc(Operand::HLAddr)?;
            }
            SBC_A_A => {
                clock.cycles(4).await;
                self.sbc(A)?;
            }
            AND_B => {
                clock.cycles(4).await;
                self.and(B)?;
            }
            AND_C => {
                clock.cycles(4).await;
                self.and(C)?;
            }
            AND_D => {
                clock.cycles(4).await;
                self.and(D)?;
            }
            AND_E => {
                clock.cycles(4).await;
                self.and(E)?;
            }
            AND_H => {
                clock.cycles(4).await;
                self.and(H)?;
            }
            AND_L => {
                clock.cycles(4).await;
                self.and(L)?;
            }
            AND_xHLx => {
                clock.cycles(8).await;
                self.and(Operand::HLAddr)?;
            }
            AND_A => {
                clock.cycles(4).await;
                self.and(A)?;
            }
            XOR_B => {
                clock.cycles(4).await;
                self.xor(B)?;
            }
            XOR_C => {
                clock.cycles(4).await;
                self.xor(C)?;
            }
            XOR_D => {
                clock.cycles(4).await;
                self.xor(D)?;
            }
            XOR_E => {
                clock.cycles(4).await;
                self.xor(E)?;
            }
            XOR_H => {
                clock.cycles(4).await;
                self.xor(H)?;
            }
            XOR_L => {
                clock.cycles(4).await;
                self.xor(L)?;
            }
            XOR_xHLx => {
                clock.cycles(8).await;
                self.xor(Operand::HLAddr)?;
            }
            XOR_A => {
                clock.cycles(4).await;
                self.xor(A)?;
            }
            OR_B => {
                clock.cycles(4).await;
                self.or(B)?;
            }
            OR_C => {
                clock.cycles(4).await;
                self.or(C)?;
            }
            OR_D => {
                clock.cycles(4).await;
                self.or(D)?;
            }
            OR_E => {
                clock.cycles(4).await;
                self.or(E)?;
            }
            OR_H => {
                clock.cycles(4).await;
                self.or(H)?;
            }
            OR_L => {
                clock.cycles(4).await;
                self.or(L)?;
            }
            OR_xHLx => {
                clock.cycles(4).await;
                self.or(Operand::HLAddr)?;
            }
            OR_A => {
                clock.cycles(4).await;
                self.or(A)?;
            }
            CP_B => {
                clock.cycles(4).await;
                self.cp(B)?;
            }
            CP_C => {
                clock.cycles(4).await;
                self.cp(C)?;
            }
            CP_D => {
                clock.cycles(4).await;
                self.cp(D)?;
            }
            CP_E => {
                clock.cycles(4).await;
                self.cp(E)?;
            }
            CP_H => {
                clock.cycles(4).await;
                self.cp(H)?;
            }
            CP_L => {
                clock.cycles(4).await;
                self.cp(L)?;
            }
            CP_xHLx => {
                clock.cycles(8).await;
                self.cp(Operand::HLAddr)?;
            }
            CP_A => {
                clock.cycles(4).await;
                self.cp(A)?;
            }
            RET_NZ => {
                clock.cycles(8).await;
                if !self.flags[Flag::Z] {
                    clock.cycles(12).await;
                    self.pc = self.pop()?;
                }
            }
            POP_BC => {
                clock.cycles(12).await;
                *self.reg.r16_mut(BC) = self.pop()?;
            }
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
            CALL_NZ_a16 => {
                clock.cycles(12).await;
                let target = self.read16()?;

                if !self.flags[Flag::Z] {
                    clock.cycles(12).await;
                    self.push(self.pc)?;
                    self.pc = target;
                }
            }
            PUSH_BC => {
                clock.cycles(16).await;
                self.push(self.reg.r16(BC))?;
            }
            ADD_A_d8 => {
                clock.cycles(8).await;
                self.add(Operand::Immediate)?;
            }
            RST_00H => {
                clock.cycles(16).await;
                self.push(self.pc)?;
                self.pc = 0x00;
            }
            RET_Z => {
                clock.cycles(8).await;
                if self.flags[Flag::Z] {
                    clock.cycles(12).await;
                    self.pc = self.pop()?;
                }
            }
            RET => {
                clock.cycles(16).await;
                self.pc = self.pop()?;
            }
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
            CALL_Z_a16 => {
                clock.cycles(12).await;
                let target = self.read16()?;

                if self.flags[Flag::Z] {
                    clock.cycles(12).await;
                    self.push(self.pc)?;
                    self.pc = target;
                }
            }
            CALL_a16 => {
                clock.cycles(24).await;
                self.push(self.pc)?;
                self.pc = self.read16()?;
            }
            ADC_A_d8 => {
                clock.cycles(8).await;
                self.adc(Operand::Immediate)?;
            }
            RST_08H => {
                clock.cycles(16).await;
                self.push(self.pc)?;
                self.pc = 0x08;
            }
            RET_NC => {
                clock.cycles(8).await;
                if !self.flags[Flag::C] {
                    clock.cycles(12).await;
                    self.pc = self.pop()?;
                }
            }
            POP_DE => {
                clock.cycles(12).await;
                *self.reg.r16_mut(DE) = self.pop()?;
            }
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
            CALL_NC_a16 => {
                clock.cycles(12).await;
                let target = self.read16()?;

                if !self.flags[Flag::C] {
                    clock.cycles(12).await;
                    self.push(self.pc)?;
                    self.pc = target;
                }
            }
            PUSH_DE => {
                clock.cycles(16).await;
                self.push(self.reg.r16(DE))?;
            }
            SUB_d8 => {
                clock.cycles(8).await;
                self.sub(Operand::Immediate)?;
            }
            RST_10H => {
                clock.cycles(16).await;
                self.push(self.pc)?;
                self.pc = 0x10;
            }
            RET_C => {
                clock.cycles(8).await;
                if self.flags[Flag::C] {
                    clock.cycles(12).await;
                    self.pc = self.pop()?;
                }
            }
            RETI => {
                clock.cycles(16).await;
                self.pc = self.pop()?;
                self.interrupts_enabled = true;
            }
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
            CALL_C_a16 => {
                clock.cycles(12).await;
                let target = self.read16()?;

                if self.flags[Flag::C] {
                    clock.cycles(12).await;
                    self.push(self.pc)?;
                    self.pc = target;
                }
            }
            NOT_USED_1 => {
                panic!("Attempted to execute unused instruction");
            }
            SBC_A_d8 => {
                clock.cycles(8).await;
                self.sbc(Operand::HLAddr)?;
            }
            RST_18H => {
                clock.cycles(16).await;
                self.push(self.pc)?;
                self.pc = 0x18;
            }
            LDH_xa8x_A => {
                clock.cycles(12).await;
                let offset = self.read8()?;
                *self.mem.get8_mut(0xff00 + offset as u16)? = self.reg.r8(A);
            }
            POP_HL => {
                clock.cycles(12).await;
                *self.reg.r16_mut(HL) = self.pop()?;
            }
            LD_xCx_A => {
                clock.cycles(8).await;
                *self.mem.get8_mut(self.reg.r8(C) as u16)? = self.reg.r8(A);
            }
            NOT_USED_2 => {
                panic!("Attempted to execute unused instruction");
            }
            NOT_USED_3 => {
                panic!("Attempted to execute unused instruction");
            }
            PUSH_HL => {
                clock.cycles(16).await;
                self.push(self.reg.r16(HL))?;
            }
            AND_d8 => {
                clock.cycles(8).await;
                self.and(Operand::Immediate)?;
            }
            RST_20H => {
                clock.cycles(16).await;
                self.push(self.pc)?;
                self.pc = 0x20;
            }
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
                let addr = self.read16()?;
                *self.mem.get8_mut(addr)? = self.reg.r8(A);
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
                self.xor(Operand::Immediate)?;
            }
            RST_28H => {
                clock.cycles(16).await;
                self.push(self.pc)?;
                self.pc = 0x28;
            }
            LDH_A_xa8x => {
                clock.cycles(12).await;
                let offset = self.read8()?;
                *self.reg.r8_mut(A) = self.mem.get8(0xff00 + offset as u16)?;
            }
            POP_AF => {
                clock.cycles(12).await;
                let af = self.pop()?;
                *self.reg.r8_mut(A) = (af >> 4) as u8;
                self.flags.set_from_u8(af as u8);
            }
            LD_A_xCx => {
                clock.cycles(8).await;
                *self.reg.r8_mut(A) = self.mem.get8(self.reg.r8(C) as u16)?;
            }
            DI => {
                clock.cycles(4).await;
                self.interrupts_enabled = false;
            }
            NOT_USED_7 => {
                panic!("Attempted to execute unused instruction");
            }
            PUSH_AF => {
                clock.cycles(16).await;
                let af = (self.reg.r8(A) as u16) << 4 + self.flags.as_u8();
                self.push(af)?;
            }
            OR_d8 => {
                clock.cycles(8).await;
                self.or(Operand::Immediate)?;
            }
            RST_30H => {
                clock.cycles(16).await;
                self.push(self.pc)?;
                self.pc = 0x30;
            }
            LD_HL_SPpr8 => {
                clock.cycles(12).await;
                *self.reg.r16_mut(HL) = (self.reg.r16(SP) as i16 + self.read8()? as i16) as u16;
            }
            LD_SP_HL => {
                clock.cycles(8).await;
                *self.reg.r16_mut(SP) = self.reg.r16(HL);
            }
            LD_A_xa16x => {
                clock.cycles(16).await;
                let addr = self.read16()?;
                *self.reg.r8_mut(A) = self.mem.get8(addr)?;
            }
            EI => {
                clock.cycles(4).await;
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
                clock.cycles(8).await;
                self.cp(Operand::Immediate)?;
            }
            RST_38H => {
                clock.cycles(16).await;
                self.push(self.pc)?;
                self.pc = 0x38;
            }
        });
    }

    fn rlc<O: Into<Operand>>(&mut self, target: O) -> Result<(), MemoryAccessError> {
        let target = target.into().into_ref(self)?;
        let old = *target;

        *target = target.rotate_left(1);

        self.flags[Flag::Z] = old == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = false;
        self.flags[Flag::C] = old & 0b1000_0000 != 0;

        Ok(())
    }

    fn rrc<O: Into<Operand>>(&mut self, target: O) -> Result<(), MemoryAccessError> {
        let target = target.into().into_ref(self)?;
        let old = *target;

        *target = target.rotate_right(1);

        self.flags[Flag::Z] = old == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = false;
        self.flags[Flag::C] = old & 0b1 != 0;

        Ok(())
    }

    fn rl<O: Into<Operand>>(&mut self, target: O) -> Result<(), MemoryAccessError> {
        let c = self.flags[Flag::C];
        let target = target.into().into_ref(self)?;
        let old = *target;

        *target <<= 1;
        if c {
            *target += 1;
        }

        self.flags[Flag::Z] = *target == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = false;
        self.flags[Flag::C] = old & 0b1000_0000 != 0;

        Ok(())
    }

    fn rr<O: Into<Operand>>(&mut self, target: O) -> Result<(), MemoryAccessError> {
        let c = self.flags[Flag::C];
        let target = target.into().into_ref(self)?;
        let old = *target;

        *target >>= 1;
        if c {
            *target += 0b1000_0000;
        }

        self.flags[Flag::Z] = *target == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = false;
        self.flags[Flag::C] = old & 0b1 != 0;

        Ok(())
    }

    fn sla<O: Into<Operand>>(&mut self, target: O) -> Result<(), MemoryAccessError> {
        let target = target.into().into_ref(self)?;
        let old = *target;

        *target <<= 1;

        self.flags[Flag::Z] = *target == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = false;
        self.flags[Flag::C] = old & 0b1000_0000 != 0;

        Ok(())
    }

    fn sra<O: Into<Operand>>(&mut self, target: O) -> Result<(), MemoryAccessError> {
        let target = target.into().into_ref(self)?;
        let old = *target;

        *target >>= 1;
        *target |= old & 0b1000_0000;

        self.flags[Flag::Z] = *target == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = false;
        self.flags[Flag::C] = old & 0b1 != 0;

        Ok(())
    }

    fn srl<O: Into<Operand>>(&mut self, target: O) -> Result<(), MemoryAccessError> {
        let target = target.into().into_ref(self)?;
        let old = *target;

        *target >>= 1;

        self.flags[Flag::Z] = *target == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = false;
        self.flags[Flag::C] = old & 0b1 != 0;

        Ok(())
    }

    fn swap<O: Into<Operand>>(&mut self, target: O) -> Result<(), MemoryAccessError> {
        let target = target.into().into_ref(self)?;

        *target = (*target >> 4) + (*target << 4);

        self.flags[Flag::Z] = *target == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = false;
        self.flags[Flag::C] = false;

        Ok(())
    }

    fn bit<O: Into<Operand>>(&mut self, bit: u8, target: O) -> Result<(), MemoryAccessError> {
        let target = target.into().into_val(self)?;

        self.flags[Flag::Z] = (target >> bit) & 0b1 == 0;
        self.flags[Flag::N] = false;
        self.flags[Flag::H] = true;

        Ok(())
    }

    fn res<O: Into<Operand>>(&mut self, bit: u8, target: O) -> Result<(), MemoryAccessError> {
        *target.into().into_ref(self)? &= !(1 << bit);

        Ok(())
    }

    fn set<O: Into<Operand>>(&mut self, bit: u8, target: O) -> Result<(), MemoryAccessError> {
        *target.into().into_ref(self)? |= 1 << bit;

        Ok(())
    }

    async fn execute_cb(
        &mut self,
        clock: &Clock,
        instruction: CBInstruction,
    ) -> Result<(), MemoryAccessError> {
        use CBInstruction::*;
        use R8::*;

        match instruction {
            RLC_B => {
                clock.cycles(8).await;
                self.rlc(B)?;
            }
            RLC_C => {
                clock.cycles(8).await;
                self.rlc(C)?;
            }
            RLC_D => {
                clock.cycles(8).await;
                self.rlc(D)?;
            }
            RLC_E => {
                clock.cycles(8).await;
                self.rlc(E)?;
            }
            RLC_H => {
                clock.cycles(8).await;
                self.rlc(H)?;
            }
            RLC_L => {
                clock.cycles(8).await;
                self.rlc(L)?;
            }
            RLC_xHLx => {
                clock.cycles(16).await;
                self.rlc(Operand::HLAddr)?;
            }
            RLC_A => {
                clock.cycles(8).await;
                self.rlc(A)?;
            }
            RRC_B => {
                clock.cycles(8).await;
                self.rrc(B)?;
            }
            RRC_C => {
                clock.cycles(8).await;
                self.rrc(C)?;
            }
            RRC_D => {
                clock.cycles(8).await;
                self.rrc(D)?;
            }
            RRC_E => {
                clock.cycles(8).await;
                self.rrc(E)?;
            }
            RRC_H => {
                clock.cycles(8).await;
                self.rrc(H)?;
            }
            RRC_L => {
                clock.cycles(8).await;
                self.rrc(L)?;
            }
            RRC_xHLx => {
                clock.cycles(16).await;
                self.rrc(Operand::HLAddr)?;
            }
            RRC_A => {
                clock.cycles(8).await;
                self.rrc(A)?;
            }
            RL_B => {
                clock.cycles(8).await;
                self.rl(B)?;
            }
            RL_C => {
                clock.cycles(8).await;
                self.rl(C)?;
            }
            RL_D => {
                clock.cycles(8).await;
                self.rl(D)?;
            }
            RL_E => {
                clock.cycles(8).await;
                self.rl(E)?;
            }
            RL_H => {
                clock.cycles(8).await;
                self.rl(H)?;
            }
            RL_L => {
                clock.cycles(8).await;
                self.rl(L)?;
            }
            RL_xHLx => {
                clock.cycles(16).await;
                self.rl(Operand::HLAddr)?;
            }
            RL_A => {
                clock.cycles(8).await;
                self.rl(A)?;
            }
            RR_B => {
                clock.cycles(8).await;
                self.rr(B)?;
            }
            RR_C => {
                clock.cycles(8).await;
                self.rr(C)?;
            }
            RR_D => {
                clock.cycles(8).await;
                self.rr(D)?;
            }
            RR_E => {
                clock.cycles(8).await;
                self.rr(E)?;
            }
            RR_H => {
                clock.cycles(8).await;
                self.rr(H)?;
            }
            RR_L => {
                clock.cycles(8).await;
                self.rr(L)?;
            }
            RR_xHLx => {
                clock.cycles(16).await;
                self.rr(Operand::HLAddr)?;
            }
            RR_A => {
                clock.cycles(8).await;
                self.rr(A)?;
            }
            SLA_B => {
                clock.cycles(8).await;
                self.sla(B)?;
            }
            SLA_C => {
                clock.cycles(8).await;
                self.sla(C)?;
            }
            SLA_D => {
                clock.cycles(8).await;
                self.sla(D)?;
            }
            SLA_E => {
                clock.cycles(8).await;
                self.sla(E)?;
            }
            SLA_H => {
                clock.cycles(8).await;
                self.sla(H)?;
            }
            SLA_L => {
                clock.cycles(8).await;
                self.sla(L)?;
            }
            SLA_xHLx => {
                clock.cycles(16).await;
                self.sla(Operand::HLAddr)?;
            }
            SLA_A => {
                clock.cycles(8).await;
                self.sla(A)?;
            }
            SRA_B => {
                clock.cycles(8).await;
                self.sra(B)?;
            }
            SRA_C => {
                clock.cycles(8).await;
                self.sra(C)?;
            }
            SRA_D => {
                clock.cycles(8).await;
                self.sra(D)?;
            }
            SRA_E => {
                clock.cycles(8).await;
                self.sra(E)?;
            }
            SRA_H => {
                clock.cycles(8).await;
                self.sra(H)?;
            }
            SRA_L => {
                clock.cycles(8).await;
                self.sra(L)?;
            }
            SRA_xHLx => {
                clock.cycles(16).await;
                self.sra(Operand::HLAddr)?;
            }
            SRA_A => {
                clock.cycles(8).await;
                self.sra(A)?;
            }
            SWAP_B => {
                clock.cycles(8).await;
                self.swap(B)?;
            }
            SWAP_C => {
                clock.cycles(8).await;
                self.swap(C)?;
            }
            SWAP_D => {
                clock.cycles(8).await;
                self.swap(D)?;
            }
            SWAP_E => {
                clock.cycles(8).await;
                self.swap(E)?;
            }
            SWAP_H => {
                clock.cycles(8).await;
                self.swap(H)?;
            }
            SWAP_L => {
                clock.cycles(8).await;
                self.swap(L)?;
            }
            SWAP_xHLx => {
                clock.cycles(16).await;
                self.swap(Operand::HLAddr)?;
            }
            SWAP_A => {
                clock.cycles(8).await;
                self.swap(A)?;
            }
            SRL_B => {
                clock.cycles(8).await;
                self.srl(B)?;
            }
            SRL_C => {
                clock.cycles(8).await;
                self.srl(C)?;
            }
            SRL_D => {
                clock.cycles(8).await;
                self.srl(D)?;
            }
            SRL_E => {
                clock.cycles(8).await;
                self.srl(E)?;
            }
            SRL_H => {
                clock.cycles(8).await;
                self.srl(H)?;
            }
            SRL_L => {
                clock.cycles(8).await;
                self.srl(L)?;
            }
            SRL_xHLx => {
                clock.cycles(16).await;
                self.srl(Operand::HLAddr)?;
            }
            SRL_A => {
                clock.cycles(8).await;
                self.srl(A)?;
            }
            BIT_0_B => {
                clock.cycles(8).await;
                self.bit(0, B)?;
            }
            BIT_0_C => {
                clock.cycles(8).await;
                self.bit(0, C)?;
            }
            BIT_0_D => {
                clock.cycles(8).await;
                self.bit(0, D)?;
            }
            BIT_0_E => {
                clock.cycles(8).await;
                self.bit(0, E)?;
            }
            BIT_0_H => {
                clock.cycles(8).await;
                self.bit(0, H)?;
            }
            BIT_0_L => {
                clock.cycles(8).await;
                self.bit(0, L)?;
            }
            BIT_0_xHLx => {
                clock.cycles(16).await;
                self.bit(0, Operand::HLAddr)?;
            }
            BIT_0_A => {
                clock.cycles(8).await;
                self.bit(0, A)?;
            }
            BIT_1_B => {
                clock.cycles(8).await;
                self.bit(1, B)?;
            }
            BIT_1_C => {
                clock.cycles(8).await;
                self.bit(1, C)?;
            }
            BIT_1_D => {
                clock.cycles(8).await;
                self.bit(1, D)?;
            }
            BIT_1_E => {
                clock.cycles(8).await;
                self.bit(1, E)?;
            }
            BIT_1_H => {
                clock.cycles(8).await;
                self.bit(1, H)?;
            }
            BIT_1_L => {
                clock.cycles(8).await;
                self.bit(1, L)?;
            }
            BIT_1_xHLx => {
                clock.cycles(16).await;
                self.bit(1, Operand::HLAddr)?;
            }
            BIT_1_A => {
                clock.cycles(8).await;
                self.bit(1, A)?;
            }
            BIT_2_B => {
                clock.cycles(8).await;
                self.bit(2, B)?;
            }
            BIT_2_C => {
                clock.cycles(8).await;
                self.bit(2, C)?;
            }
            BIT_2_D => {
                clock.cycles(8).await;
                self.bit(2, D)?;
            }
            BIT_2_E => {
                clock.cycles(8).await;
                self.bit(2, E)?;
            }
            BIT_2_H => {
                clock.cycles(8).await;
                self.bit(2, H)?;
            }
            BIT_2_L => {
                clock.cycles(8).await;
                self.bit(2, L)?;
            }
            BIT_2_xHLx => {
                clock.cycles(16).await;
                self.bit(2, Operand::HLAddr)?;
            }
            BIT_2_A => {
                clock.cycles(8).await;
                self.bit(2, A)?;
            }
            BIT_3_B => {
                clock.cycles(8).await;
                self.bit(3, B)?;
            }
            BIT_3_C => {
                clock.cycles(8).await;
                self.bit(3, C)?;
            }
            BIT_3_D => {
                clock.cycles(8).await;
                self.bit(3, D)?;
            }
            BIT_3_E => {
                clock.cycles(8).await;
                self.bit(3, E)?;
            }
            BIT_3_H => {
                clock.cycles(8).await;
                self.bit(3, H)?;
            }
            BIT_3_L => {
                clock.cycles(8).await;
                self.bit(3, L)?;
            }
            BIT_3_xHLx => {
                clock.cycles(16).await;
                self.bit(3, Operand::HLAddr)?;
            }
            BIT_3_A => {
                clock.cycles(8).await;
                self.bit(3, A)?;
            }
            BIT_4_B => {
                clock.cycles(8).await;
                self.bit(4, B)?;
            }
            BIT_4_C => {
                clock.cycles(8).await;
                self.bit(4, C)?;
            }
            BIT_4_D => {
                clock.cycles(8).await;
                self.bit(4, D)?;
            }
            BIT_4_E => {
                clock.cycles(8).await;
                self.bit(4, E)?;
            }
            BIT_4_H => {
                clock.cycles(8).await;
                self.bit(4, H)?;
            }
            BIT_4_L => {
                clock.cycles(8).await;
                self.bit(4, L)?;
            }
            BIT_4_xHLx => {
                clock.cycles(16).await;
                self.bit(4, Operand::HLAddr)?;
            }
            BIT_4_A => {
                clock.cycles(8).await;
                self.bit(4, A)?;
            }
            BIT_5_B => {
                clock.cycles(8).await;
                self.bit(5, B)?;
            }
            BIT_5_C => {
                clock.cycles(8).await;
                self.bit(5, C)?;
            }
            BIT_5_D => {
                clock.cycles(8).await;
                self.bit(5, D)?;
            }
            BIT_5_E => {
                clock.cycles(8).await;
                self.bit(5, E)?;
            }
            BIT_5_H => {
                clock.cycles(8).await;
                self.bit(5, H)?;
            }
            BIT_5_L => {
                clock.cycles(8).await;
                self.bit(5, L)?;
            }
            BIT_5_xHLx => {
                clock.cycles(16).await;
                self.bit(5, Operand::HLAddr)?;
            }
            BIT_5_A => {
                clock.cycles(8).await;
                self.bit(5, A)?;
            }
            BIT_6_B => {
                clock.cycles(8).await;
                self.bit(6, B)?;
            }
            BIT_6_C => {
                clock.cycles(8).await;
                self.bit(6, C)?;
            }
            BIT_6_D => {
                clock.cycles(8).await;
                self.bit(6, D)?;
            }
            BIT_6_E => {
                clock.cycles(8).await;
                self.bit(6, E)?;
            }
            BIT_6_H => {
                clock.cycles(8).await;
                self.bit(6, H)?;
            }
            BIT_6_L => {
                clock.cycles(8).await;
                self.bit(6, L)?;
            }
            BIT_6_xHLx => {
                clock.cycles(16).await;
                self.bit(6, Operand::HLAddr)?;
            }
            BIT_6_A => {
                clock.cycles(8).await;
                self.bit(6, A)?;
            }
            BIT_7_B => {
                clock.cycles(8).await;
                self.bit(7, B)?;
            }
            BIT_7_C => {
                clock.cycles(8).await;
                self.bit(7, C)?;
            }
            BIT_7_D => {
                clock.cycles(8).await;
                self.bit(7, D)?;
            }
            BIT_7_E => {
                clock.cycles(8).await;
                self.bit(7, E)?;
            }
            BIT_7_H => {
                clock.cycles(8).await;
                self.bit(7, H)?;
            }
            BIT_7_L => {
                clock.cycles(8).await;
                self.bit(7, L)?;
            }
            BIT_7_xHLx => {
                clock.cycles(16).await;
                self.bit(7, Operand::HLAddr)?;
            }
            BIT_7_A => {
                clock.cycles(8).await;
                self.bit(7, A)?;
            }
            RES_0_B => {
                clock.cycles(8).await;
                self.res(0, B)?;
            }
            RES_0_C => {
                clock.cycles(8).await;
                self.res(0, C)?;
            }
            RES_0_D => {
                clock.cycles(8).await;
                self.res(0, D)?;
            }
            RES_0_E => {
                clock.cycles(8).await;
                self.res(0, E)?;
            }
            RES_0_H => {
                clock.cycles(8).await;
                self.res(0, H)?;
            }
            RES_0_L => {
                clock.cycles(8).await;
                self.res(0, L)?;
            }
            RES_0_xHLx => {
                clock.cycles(16).await;
                self.res(0, Operand::HLAddr)?;
            }
            RES_0_A => {
                clock.cycles(8).await;
                self.res(0, A)?;
            }
            RES_1_B => {
                clock.cycles(8).await;
                self.res(1, B)?;
            }
            RES_1_C => {
                clock.cycles(8).await;
                self.res(1, C)?;
            }
            RES_1_D => {
                clock.cycles(8).await;
                self.res(1, D)?;
            }
            RES_1_E => {
                clock.cycles(8).await;
                self.res(1, E)?;
            }
            RES_1_H => {
                clock.cycles(8).await;
                self.res(1, H)?;
            }
            RES_1_L => {
                clock.cycles(8).await;
                self.res(1, L)?;
            }
            RES_1_xHLx => {
                clock.cycles(16).await;
                self.res(1, Operand::HLAddr)?;
            }
            RES_1_A => {
                clock.cycles(8).await;
                self.res(1, A)?;
            }
            RES_2_B => {
                clock.cycles(8).await;
                self.res(2, B)?;
            }
            RES_2_C => {
                clock.cycles(8).await;
                self.res(2, C)?;
            }
            RES_2_D => {
                clock.cycles(8).await;
                self.res(2, D)?;
            }
            RES_2_E => {
                clock.cycles(8).await;
                self.res(2, E)?;
            }
            RES_2_H => {
                clock.cycles(8).await;
                self.res(2, H)?;
            }
            RES_2_L => {
                clock.cycles(8).await;
                self.res(2, L)?;
            }
            RES_2_xHLx => {
                clock.cycles(16).await;
                self.res(2, Operand::HLAddr)?;
            }
            RES_2_A => {
                clock.cycles(8).await;
                self.res(2, A)?;
            }
            RES_3_B => {
                clock.cycles(8).await;
                self.res(3, B)?;
            }
            RES_3_C => {
                clock.cycles(8).await;
                self.res(3, C)?;
            }
            RES_3_D => {
                clock.cycles(8).await;
                self.res(3, D)?;
            }
            RES_3_E => {
                clock.cycles(8).await;
                self.res(3, E)?;
            }
            RES_3_H => {
                clock.cycles(8).await;
                self.res(3, H)?;
            }
            RES_3_L => {
                clock.cycles(8).await;
                self.res(3, L)?;
            }
            RES_3_xHLx => {
                clock.cycles(16).await;
                self.res(3, Operand::HLAddr)?;
            }
            RES_3_A => {
                clock.cycles(8).await;
                self.res(3, A)?;
            }
            RES_4_B => {
                clock.cycles(8).await;
                self.res(4, B)?;
            }
            RES_4_C => {
                clock.cycles(8).await;
                self.res(4, C)?;
            }
            RES_4_D => {
                clock.cycles(8).await;
                self.res(4, D)?;
            }
            RES_4_E => {
                clock.cycles(8).await;
                self.res(4, E)?;
            }
            RES_4_H => {
                clock.cycles(8).await;
                self.res(4, H)?;
            }
            RES_4_L => {
                clock.cycles(8).await;
                self.res(4, L)?;
            }
            RES_4_xHLx => {
                clock.cycles(16).await;
                self.res(4, Operand::HLAddr)?;
            }
            RES_4_A => {
                clock.cycles(8).await;
                self.res(4, A)?;
            }
            RES_5_B => {
                clock.cycles(8).await;
                self.res(5, B)?;
            }
            RES_5_C => {
                clock.cycles(8).await;
                self.res(5, C)?;
            }
            RES_5_D => {
                clock.cycles(8).await;
                self.res(5, D)?;
            }
            RES_5_E => {
                clock.cycles(8).await;
                self.res(5, E)?;
            }
            RES_5_H => {
                clock.cycles(8).await;
                self.res(5, H)?;
            }
            RES_5_L => {
                clock.cycles(8).await;
                self.res(5, L)?;
            }
            RES_5_xHLx => {
                clock.cycles(16).await;
                self.res(5, Operand::HLAddr)?;
            }
            RES_5_A => {
                clock.cycles(8).await;
                self.res(5, A)?;
            }
            RES_6_B => {
                clock.cycles(8).await;
                self.res(6, B)?;
            }
            RES_6_C => {
                clock.cycles(8).await;
                self.res(6, C)?;
            }
            RES_6_D => {
                clock.cycles(8).await;
                self.res(6, D)?;
            }
            RES_6_E => {
                clock.cycles(8).await;
                self.res(6, E)?;
            }
            RES_6_H => {
                clock.cycles(8).await;
                self.res(6, H)?;
            }
            RES_6_L => {
                clock.cycles(8).await;
                self.res(6, L)?;
            }
            RES_6_xHLx => {
                clock.cycles(16).await;
                self.res(6, Operand::HLAddr)?;
            }
            RES_6_A => {
                clock.cycles(8).await;
                self.res(6, A)?;
            }
            RES_7_B => {
                clock.cycles(8).await;
                self.res(7, B)?;
            }
            RES_7_C => {
                clock.cycles(8).await;
                self.res(7, C)?;
            }
            RES_7_D => {
                clock.cycles(8).await;
                self.res(7, D)?;
            }
            RES_7_E => {
                clock.cycles(8).await;
                self.res(7, E)?;
            }
            RES_7_H => {
                clock.cycles(8).await;
                self.res(7, H)?;
            }
            RES_7_L => {
                clock.cycles(8).await;
                self.res(7, L)?;
            }
            RES_7_xHLx => {
                clock.cycles(16).await;
                self.res(7, Operand::HLAddr)?;
            }
            RES_7_A => {
                clock.cycles(8).await;
                self.res(7, A)?;
            }
            SET_0_B => {
                clock.cycles(8).await;
                self.set(0, B)?;
            }
            SET_0_C => {
                clock.cycles(8).await;
                self.set(0, C)?;
            }
            SET_0_D => {
                clock.cycles(8).await;
                self.set(0, D)?;
            }
            SET_0_E => {
                clock.cycles(8).await;
                self.set(0, E)?;
            }
            SET_0_H => {
                clock.cycles(8).await;
                self.set(0, H)?;
            }
            SET_0_L => {
                clock.cycles(8).await;
                self.set(0, L)?;
            }
            SET_0_xHLx => {
                clock.cycles(16).await;
                self.set(0, Operand::HLAddr)?;
            }
            SET_0_A => {
                clock.cycles(8).await;
                self.set(0, A)?;
            }
            SET_1_B => {
                clock.cycles(8).await;
                self.set(1, B)?;
            }
            SET_1_C => {
                clock.cycles(8).await;
                self.set(1, C)?;
            }
            SET_1_D => {
                clock.cycles(8).await;
                self.set(1, D)?;
            }
            SET_1_E => {
                clock.cycles(8).await;
                self.set(1, E)?;
            }
            SET_1_H => {
                clock.cycles(8).await;
                self.set(1, H)?;
            }
            SET_1_L => {
                clock.cycles(8).await;
                self.set(1, L)?;
            }
            SET_1_xHLx => {
                clock.cycles(16).await;
                self.set(1, Operand::HLAddr)?;
            }
            SET_1_A => {
                clock.cycles(8).await;
                self.set(1, A)?;
            }
            SET_2_B => {
                clock.cycles(8).await;
                self.set(2, B)?;
            }
            SET_2_C => {
                clock.cycles(8).await;
                self.set(2, C)?;
            }
            SET_2_D => {
                clock.cycles(8).await;
                self.set(2, D)?;
            }
            SET_2_E => {
                clock.cycles(8).await;
                self.set(2, E)?;
            }
            SET_2_H => {
                clock.cycles(8).await;
                self.set(2, H)?;
            }
            SET_2_L => {
                clock.cycles(8).await;
                self.set(2, L)?;
            }
            SET_2_xHLx => {
                clock.cycles(16).await;
                self.set(2, Operand::HLAddr)?;
            }
            SET_2_A => {
                clock.cycles(8).await;
                self.set(2, A)?;
            }
            SET_3_B => {
                clock.cycles(8).await;
                self.set(3, B)?;
            }
            SET_3_C => {
                clock.cycles(8).await;
                self.set(3, C)?;
            }
            SET_3_D => {
                clock.cycles(8).await;
                self.set(3, D)?;
            }
            SET_3_E => {
                clock.cycles(8).await;
                self.set(3, E)?;
            }
            SET_3_H => {
                clock.cycles(8).await;
                self.set(3, H)?;
            }
            SET_3_L => {
                clock.cycles(8).await;
                self.set(3, L)?;
            }
            SET_3_xHLx => {
                clock.cycles(16).await;
                self.set(3, Operand::HLAddr)?;
            }
            SET_3_A => {
                clock.cycles(8).await;
                self.set(3, A)?;
            }
            SET_4_B => {
                clock.cycles(8).await;
                self.set(4, B)?;
            }
            SET_4_C => {
                clock.cycles(8).await;
                self.set(4, C)?;
            }
            SET_4_D => {
                clock.cycles(8).await;
                self.set(4, D)?;
            }
            SET_4_E => {
                clock.cycles(8).await;
                self.set(4, E)?;
            }
            SET_4_H => {
                clock.cycles(8).await;
                self.set(4, H)?;
            }
            SET_4_L => {
                clock.cycles(8).await;
                self.set(4, L)?;
            }
            SET_4_xHLx => {
                clock.cycles(16).await;
                self.set(4, Operand::HLAddr)?;
            }
            SET_4_A => {
                clock.cycles(8).await;
                self.set(4, A)?;
            }
            SET_5_B => {
                clock.cycles(8).await;
                self.set(5, B)?;
            }
            SET_5_C => {
                clock.cycles(8).await;
                self.set(5, C)?;
            }
            SET_5_D => {
                clock.cycles(8).await;
                self.set(5, D)?;
            }
            SET_5_E => {
                clock.cycles(8).await;
                self.set(5, E)?;
            }
            SET_5_H => {
                clock.cycles(8).await;
                self.set(5, H)?;
            }
            SET_5_L => {
                clock.cycles(8).await;
                self.set(5, L)?;
            }
            SET_5_xHLx => {
                clock.cycles(16).await;
                self.set(5, Operand::HLAddr)?;
            }
            SET_5_A => {
                clock.cycles(8).await;
                self.set(5, A)?;
            }
            SET_6_B => {
                clock.cycles(8).await;
                self.set(6, B)?;
            }
            SET_6_C => {
                clock.cycles(8).await;
                self.set(6, C)?;
            }
            SET_6_D => {
                clock.cycles(8).await;
                self.set(6, D)?;
            }
            SET_6_E => {
                clock.cycles(8).await;
                self.set(6, E)?;
            }
            SET_6_H => {
                clock.cycles(8).await;
                self.set(6, H)?;
            }
            SET_6_L => {
                clock.cycles(8).await;
                self.set(6, L)?;
            }
            SET_6_xHLx => {
                clock.cycles(16).await;
                self.set(6, Operand::HLAddr)?;
            }
            SET_6_A => {
                clock.cycles(8).await;
                self.set(6, A)?;
            }
            SET_7_B => {
                clock.cycles(8).await;
                self.set(7, B)?;
            }
            SET_7_C => {
                clock.cycles(8).await;
                self.set(7, C)?;
            }
            SET_7_D => {
                clock.cycles(8).await;
                self.set(7, D)?;
            }
            SET_7_E => {
                clock.cycles(8).await;
                self.set(7, E)?;
            }
            SET_7_H => {
                clock.cycles(8).await;
                self.set(7, H)?;
            }
            SET_7_L => {
                clock.cycles(8).await;
                self.set(7, L)?;
            }
            SET_7_xHLx => {
                clock.cycles(16).await;
                self.set(7, Operand::HLAddr)?;
            }
            SET_7_A => {
                clock.cycles(8).await;
                self.set(7, A)?;
            }
        }

        Ok(())
    }
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
    Immediate,
}

impl Operand {
    fn into_val(self, cpu: &mut CPU) -> Result<u8, MemoryAccessError> {
        Ok(match self {
            Operand::Reg(r) => cpu.reg.r8(r),
            Operand::HLAddr => cpu.mem.get8(cpu.reg.r16(R16::HL))?,
            Operand::Immediate => cpu.read8()?,
        })
    }

    fn into_ref(self, cpu: &mut CPU) -> Result<&mut u8, MemoryAccessError> {
        Ok(match self {
            Operand::Reg(r) => cpu.reg.r8_mut(r),
            Operand::HLAddr => cpu.mem.get8_mut(cpu.reg.r16(R16::HL))?,
            Operand::Immediate => unreachable!(), // TODO: Maybe unchecked?
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
