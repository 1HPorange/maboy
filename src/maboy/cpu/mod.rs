mod execute;
mod instruction;
mod operands;
mod registers;

use super::board::Board;
use super::cartridge::CartridgeMem;
use super::{
    debug::{CpuEvt, DbgEvtSrc},
    interrupt_system::Interrupt,
};
use execute::*;
use operands::{HighRamOperand, HlOperand, Imm8, ImmAddr};

pub use instruction::{ByteInstr, CBByteInstr};
pub use registers::{Flags, Registers, R16, R8};

// TODO: Pop AF forces lower 4 bits to be zero, no matter what is popped!
// TODO: OAM DMA Takes the same to in both double and single speed mode!
// TODO: PAUSE and STOP pause a DMA copy, but it will complete afterwards
// TODO: const fn reserach

pub struct CPU {
    /// Shared memory for all 8 and 16 bit registers, including SP
    pub reg: Registers,

    /// Interrupt Master Enable: Dictates whether the CPU jumps to one of
    /// the corresponding interrupt routines and clears the interrupt
    /// request (true), or if it ignores the interrupts (false). Note that
    /// HALT and STOP instructions can still be interrupted even when IME
    /// is false, but the jump to the interrupt routine is not performed.
    pub ime: bool,

    /// The special HALT and STOP instructions can suspend CPU operation
    /// until an interrupt occurs. They also have minor timing
    /// implications and provide opportunity for power savings.
    pub halt_state: HaltState,
}

// TODO: Respect these states!
#[derive(Debug, Copy, Clone)]
pub enum HaltState {
    Running,

    /// Reached after encountering a HALT instruction
    Halted,

    /// Reached after encountering a STOP instruction
    Stopped,

    /// Reached after encountering one of the unused Instructions. There is
    /// no way to recover from this state.
    Stuck, // TODO: Respect this state
}

impl CPU {
    pub fn new() -> CPU {
        CPU {
            reg: Registers::new(),
            ime: false,
            halt_state: HaltState::Running,
        }
    }

    pub fn step_instr<B: Board>(&mut self, board: &mut B) {
        match board.ir_system().query_interrupt_request() {
            Some(interrupt) if self.ime => self.jmp_to_interrupt_handler(board, interrupt),
            // Interrupt flags are ONLY cleared if we take the jump, so we don't
            // need to clear them that in any of the other match arms
            None if matches!(self.halt_state, HaltState::Halted) => {
                // Just idle until we get an interrupt
                board.advance_mcycle();
            }
            Some(_) if matches!(self.halt_state, HaltState::Halted) => {
                self.set_halt_state(board, HaltState::Running);
                let instr_pc = self.reg.pc();
                let instr = self.prefetch(board);
                board.push_cpu_evt(CpuEvt::Exec(instr_pc, instr));
                self.execute(board, instr);
            }
            _ => {
                let instr_pc = self.reg.pc();
                let instr = self.prefetch(board);
                board.push_cpu_evt(CpuEvt::Exec(instr_pc, instr));
                self.execute(board, instr);
            }
        }
    }

    fn read8i<B: Board>(&mut self, board: &mut B) -> u8 {
        let result = board.read8(self.reg.r16(R16::PC));
        *self.reg.r16_mut(R16::PC) = self.reg.r16(R16::PC).wrapping_add(1);
        result
    }

    fn read16i<B: Board>(&mut self, board: &mut B) -> u16 {
        let result = board.read16(self.reg.r16(R16::PC));
        *self.reg.r16_mut(R16::PC) = self.reg.r16(R16::PC).wrapping_add(2);
        result
    }

    fn jmp_to_interrupt_handler<B: Board>(&mut self, board: &mut B, interrupt: Interrupt) {
        // TODO: Add additional 4 clock wait if waking from HALT (and STOP???)
        // TODO: Recheck the timing in this function

        board.push_cpu_evt(CpuEvt::HandleIR(interrupt));

        self.set_ime(board, false);

        // TODO: Make this stuff prettier... I mean we have IRSystem...
        // TODO: Move this code into IRSystem
        // Clear the interrupt request in the IF register
        let old_if = board.ir_system().read_if();
        board.ir_system().write_if(old_if & !(interrupt as u8));

        // Timing stuff... The entire thing should take 20 cycles / 5 MCycles
        board.advance_mcycle(); // 1

        push(self, board, R16::PC); // 2,3,4
        *self.reg.pc_mut() = match interrupt {
            Interrupt::VBlank => 0x40,
            Interrupt::LcdStat => 0x48,
            Interrupt::Timer => 0x50,
            Interrupt::Serial => 0x58,
            Interrupt::Joypad => 0x60,
        };

        // 5 (The last clock is spent during the next prefetch)
    }

    fn set_halt_state<B: Board>(&mut self, board: &mut B, halt_state: HaltState) {
        board.push_cpu_evt(CpuEvt::EnterHalt(halt_state));

        self.halt_state = halt_state;

        // TODO: This moves away once we implemented all HaltStates correctly
        match halt_state {
            HaltState::Halted => (),
            HaltState::Running => (),
            _ => unimplemented!("{:?}", halt_state),
        }
    }

    fn set_ime<B: Board>(&mut self, board: &mut B, ime: bool) {
        self.ime = ime;

        if ime {
            board.push_cpu_evt(CpuEvt::IrEnable);
        } else {
            board.push_cpu_evt(CpuEvt::IrDisable);
        }
    }

    fn prefetch<B: Board>(&mut self, board: &mut B) -> ByteInstr {
        unsafe { std::mem::transmute(self.read8i(board)) }
    }

    fn fetch_cb<B: Board>(&mut self, board: &mut B) -> CBByteInstr {
        unsafe { std::mem::transmute(self.read8i(board)) }
    }

    fn execute<B: Board>(&mut self, board: &mut B, instr: ByteInstr) {
        use ByteInstr::*;
        use HlOperand::*;
        use R16::*;
        use R8::*;

        match instr {
            NOP => (),
            LD_BC_d16 => ld_rr_d16(self, board, BC),
            LD_xBCx_A => ld8(self, board, BC, A),
            INC_BC => inc_rr(self, board, BC),
            INC_B => inc8(self, board, B),
            DEC_B => dec8(self, board, B),
            LD_B_d8 => ld8(self, board, B, Imm8),
            RLCA => rlca(self),
            LD_xa16x_SP => ld_a16_sp(self, board),
            ADD_HL_BC => add_hl_rr(self, board, BC),
            LD_A_xBCx => ld8(self, board, A, BC),
            DEC_BC => dec_rr(self, board, BC),
            INC_C => inc8(self, board, C),
            DEC_C => dec8(self, board, C),
            LD_C_d8 => ld8(self, board, C, Imm8),
            RRCA => rrca(self),
            STOP => self.set_halt_state(board, HaltState::Stopped),
            LD_DE_d16 => ld_rr_d16(self, board, DE),
            LD_xDEx_A => ld8(self, board, DE, A),
            INC_DE => inc_rr(self, board, DE),
            INC_D => inc8(self, board, D),
            DEC_D => dec8(self, board, D),
            LD_D_d8 => ld8(self, board, D, Imm8),
            RLA => rla(self),
            JR_r8 => jr_cond(self, board, true),
            ADD_HL_DE => add_hl_rr(self, board, DE),
            LD_A_xDEx => ld8(self, board, A, DE),
            DEC_DE => dec_rr(self, board, DE),
            INC_E => inc8(self, board, E),
            DEC_E => dec8(self, board, E),
            LD_E_d8 => ld8(self, board, E, Imm8),
            RRA => rra(self),
            JR_NZ_r8 => jr_cond(self, board, !self.reg.flags().contains(Flags::Z)),
            LD_HL_d16 => ld_rr_d16(self, board, HL),
            LD_xHLix_A => ld8(self, board, HLi, A),
            INC_HL => inc_rr(self, board, HL),
            INC_H => inc8(self, board, H),
            DEC_H => dec8(self, board, H),
            LD_H_d8 => ld8(self, board, H, Imm8),
            DAA => daa(self),
            JR_Z_r8 => jr_cond(self, board, self.reg.flags().contains(Flags::Z)),
            ADD_HL_HL => add_hl_rr(self, board, HL),
            LD_A_xHLix => ld8(self, board, A, HLi),
            DEC_HL => dec_rr(self, board, HL),
            INC_L => inc8(self, board, L),
            DEC_L => dec8(self, board, L),
            LD_L_d8 => ld8(self, board, L, Imm8),
            CPL => cpl(self),
            JR_NC_r8 => jr_cond(self, board, !self.reg.flags().contains(Flags::C)),
            LD_SP_d16 => ld_rr_d16(self, board, SP),
            LD_xHLdx_A => ld8(self, board, HLd, A),
            INC_SP => inc_rr(self, board, SP),
            INC_xHLx => inc8(self, board, HL),
            DEC_xHLx => dec8(self, board, HL),
            LD_xHLx_d8 => ld8(self, board, HL, Imm8),
            SCF => scf(self),
            JR_C_r8 => jr_cond(self, board, self.reg.flags().contains(Flags::C)),
            ADD_HL_SP => add_hl_rr(self, board, SP),
            LD_A_xHLdx => ld8(self, board, A, HLd),
            DEC_SP => dec_rr(self, board, SP),
            INC_A => inc8(self, board, A),
            DEC_A => dec8(self, board, A),
            LD_A_d8 => ld8(self, board, A, Imm8),
            CCF => ccf(self),
            LD_B_B => ld8(self, board, B, B),
            LD_B_C => ld8(self, board, B, C),
            LD_B_D => ld8(self, board, B, D),
            LD_B_E => ld8(self, board, B, E),
            LD_B_H => ld8(self, board, B, H),
            LD_B_L => ld8(self, board, B, L),
            LD_B_xHLx => ld8(self, board, B, HL),
            LD_B_A => ld8(self, board, B, A),
            LD_C_B => ld8(self, board, C, B),
            LD_C_C => ld8(self, board, C, C),
            LD_C_D => ld8(self, board, C, D),
            LD_C_E => ld8(self, board, C, E),
            LD_C_H => ld8(self, board, C, H),
            LD_C_L => ld8(self, board, C, L),
            LD_C_xHLx => ld8(self, board, C, HL),
            LD_C_A => ld8(self, board, C, A),
            LD_D_B => ld8(self, board, D, B),
            LD_D_C => ld8(self, board, D, C),
            LD_D_D => ld8(self, board, D, D),
            LD_D_E => ld8(self, board, D, E),
            LD_D_H => ld8(self, board, D, H),
            LD_D_L => ld8(self, board, D, L),
            LD_D_xHLx => ld8(self, board, D, HL),
            LD_D_A => ld8(self, board, D, A),
            LD_E_B => ld8(self, board, E, B),
            LD_E_C => ld8(self, board, E, C),
            LD_E_D => ld8(self, board, E, D),
            LD_E_E => ld8(self, board, E, E),
            LD_E_H => ld8(self, board, E, H),
            LD_E_L => ld8(self, board, E, L),
            LD_E_xHLx => ld8(self, board, E, HL),
            LD_E_A => ld8(self, board, E, A),
            LD_H_B => ld8(self, board, H, B),
            LD_H_C => ld8(self, board, H, C),
            LD_H_D => ld8(self, board, H, D),
            LD_H_E => ld8(self, board, H, E),
            LD_H_H => ld8(self, board, H, H),
            LD_H_L => ld8(self, board, H, L),
            LD_H_xHLx => ld8(self, board, H, HL),
            LD_H_A => ld8(self, board, H, A),
            LD_L_B => ld8(self, board, L, B),
            LD_L_C => ld8(self, board, L, C),
            LD_L_D => ld8(self, board, L, D),
            LD_L_E => ld8(self, board, L, E),
            LD_L_H => ld8(self, board, L, H),
            LD_L_L => ld8(self, board, L, L),
            LD_L_xHLx => ld8(self, board, L, HL),
            LD_L_A => ld8(self, board, L, A),
            LD_xHLx_B => ld8(self, board, HL, B),
            LD_xHLx_C => ld8(self, board, HL, C),
            LD_xHLx_D => ld8(self, board, HL, D),
            LD_xHLx_E => ld8(self, board, HL, E),
            LD_xHLx_H => ld8(self, board, HL, H),
            LD_xHLx_L => ld8(self, board, HL, L),
            HALT => self.set_halt_state(board, HaltState::Halted),
            LD_xHLx_A => ld8(self, board, HL, A),
            LD_A_B => ld8(self, board, A, B),
            LD_A_C => ld8(self, board, A, C),
            LD_A_D => ld8(self, board, A, D),
            LD_A_E => ld8(self, board, A, E),
            LD_A_H => ld8(self, board, A, H),
            LD_A_L => ld8(self, board, A, L),
            LD_A_xHLx => ld8(self, board, A, HL),
            LD_A_A => ld8(self, board, A, A),
            ADD_A_B => add8(self, board, B),
            ADD_A_C => add8(self, board, C),
            ADD_A_D => add8(self, board, D),
            ADD_A_E => add8(self, board, E),
            ADD_A_H => add8(self, board, H),
            ADD_A_L => add8(self, board, L),
            ADD_A_xHLx => add8(self, board, HL),
            ADD_A_A => add8(self, board, A),
            ADC_A_B => adc8(self, board, B),
            ADC_A_C => adc8(self, board, C),
            ADC_A_D => adc8(self, board, D),
            ADC_A_E => adc8(self, board, E),
            ADC_A_H => adc8(self, board, H),
            ADC_A_L => adc8(self, board, L),
            ADC_A_xHLx => adc8(self, board, HL),
            ADC_A_A => adc8(self, board, A),
            SUB_B => sub8(self, board, B),
            SUB_C => sub8(self, board, C),
            SUB_D => sub8(self, board, D),
            SUB_E => sub8(self, board, E),
            SUB_H => sub8(self, board, H),
            SUB_L => sub8(self, board, L),
            SUB_xHLx => sub8(self, board, HL),
            SUB_A => sub8(self, board, A),
            SBC_A_B => sbc8(self, board, B),
            SBC_A_C => sbc8(self, board, C),
            SBC_A_D => sbc8(self, board, D),
            SBC_A_E => sbc8(self, board, E),
            SBC_A_H => sbc8(self, board, H),
            SBC_A_L => sbc8(self, board, L),
            SBC_A_xHLx => sbc8(self, board, HL),
            SBC_A_A => sbc8(self, board, A),
            AND_B => and8(self, board, B),
            AND_C => and8(self, board, C),
            AND_D => and8(self, board, D),
            AND_E => and8(self, board, E),
            AND_H => and8(self, board, H),
            AND_L => and8(self, board, L),
            AND_xHLx => and8(self, board, HL),
            AND_A => and8(self, board, A),
            XOR_B => xor8(self, board, B),
            XOR_C => xor8(self, board, C),
            XOR_D => xor8(self, board, D),
            XOR_E => xor8(self, board, E),
            XOR_H => xor8(self, board, H),
            XOR_L => xor8(self, board, L),
            XOR_xHLx => xor8(self, board, HL),
            XOR_A => xor8(self, board, A),
            OR_B => or8(self, board, B),
            OR_C => or8(self, board, C),
            OR_D => or8(self, board, D),
            OR_E => or8(self, board, E),
            OR_H => or8(self, board, H),
            OR_L => or8(self, board, L),
            OR_xHLx => or8(self, board, HL),
            OR_A => or8(self, board, A),
            CP_B => drop(cp8(self, board, B)),
            CP_C => drop(cp8(self, board, C)),
            CP_D => drop(cp8(self, board, D)),
            CP_E => drop(cp8(self, board, E)),
            CP_H => drop(cp8(self, board, H)),
            CP_L => drop(cp8(self, board, L)),
            CP_xHLx => drop(cp8(self, board, HL)),
            CP_A => drop(cp8(self, board, A)),
            RET_NZ => ret_cond(self, board, !self.reg.flags().contains(Flags::Z)),
            POP_BC => pop(self, board, BC),
            JP_NZ_a16 => jp_cond(self, board, !self.reg.flags().contains(Flags::Z)),
            JP_a16 => jp_cond(self, board, true),
            CALL_NZ_a16 => call_cond(self, board, !self.reg.flags().contains(Flags::Z)),
            PUSH_BC => push(self, board, BC),
            ADD_A_d8 => add8(self, board, Imm8),
            RST_00H => rst(self, board, 0x00),
            RET_Z => ret_cond(self, board, self.reg.flags().contains(Flags::Z)),
            RET => ret(self, board, false),
            JP_Z_a16 => jp_cond(self, board, self.reg.flags().contains(Flags::Z)),
            PREFIX_CB => self.fetch_execute_cb(board),
            CALL_Z_a16 => call_cond(self, board, self.reg.flags().contains(Flags::Z)),
            CALL_a16 => call_cond(self, board, true),
            ADC_A_d8 => adc8(self, board, Imm8),
            RST_08H => rst(self, board, 0x08),
            RET_NC => ret_cond(self, board, !self.reg.flags().contains(Flags::C)),
            POP_DE => pop(self, board, DE),
            JP_NC_a16 => jp_cond(self, board, !self.reg.flags().contains(Flags::C)),
            NOT_USED => self.set_halt_state(board, HaltState::Stuck),
            CALL_NC_a16 => call_cond(self, board, !self.reg.flags().contains(Flags::C)),
            PUSH_DE => push(self, board, DE),
            SUB_d8 => sub8(self, board, Imm8),
            RST_10H => rst(self, board, 0x10),
            RET_C => ret_cond(self, board, self.reg.flags().contains(Flags::C)),
            RETI => ret(self, board, true),
            JP_C_a16 => jp_cond(self, board, self.reg.flags().contains(Flags::C)),
            NOT_USED_0 => self.set_halt_state(board, HaltState::Stuck),
            CALL_C_a16 => call_cond(self, board, self.reg.flags().contains(Flags::C)),
            NOT_USED_1 => self.set_halt_state(board, HaltState::Stuck),
            SBC_A_d8 => sbc8(self, board, Imm8),
            RST_18H => rst(self, board, 0x18),
            LDH_xa8x_A => ld8(self, board, HighRamOperand::Imm8, A),
            POP_HL => pop(self, board, HL),
            LD_xCx_A => ld8(self, board, HighRamOperand::C, A),
            NOT_USED_2 => self.set_halt_state(board, HaltState::Stuck),
            NOT_USED_3 => self.set_halt_state(board, HaltState::Stuck),
            PUSH_HL => push(self, board, HL),
            AND_d8 => and8(self, board, Imm8),
            RST_20H => rst(self, board, 0x20),
            ADD_SP_r8 => add_sp_r8(self, board),
            JP_xHLx => jp_hl(self, board),
            LD_xa16x_A => ld8(self, board, ImmAddr, A),
            NOT_USED_4 => self.set_halt_state(board, HaltState::Stuck),
            NOT_USED_5 => self.set_halt_state(board, HaltState::Stuck),
            NOT_USED_6 => self.set_halt_state(board, HaltState::Stuck),
            XOR_d8 => xor8(self, board, Imm8),
            RST_28H => rst(self, board, 0x28),
            LDH_A_xa8x => ld8(self, board, A, HighRamOperand::Imm8),
            POP_AF => pop_af(self, board),
            LD_A_xCx => ld8(self, board, A, HighRamOperand::C),
            DI => self.set_ime(board, false),
            NOT_USED_7 => self.set_halt_state(board, HaltState::Stuck),
            PUSH_AF => push(self, board, AF),
            OR_d8 => or8(self, board, Imm8),
            RST_30H => rst(self, board, 0x30),
            LD_HL_SPpr8 => ld_hl_sp_r8(self, board),
            LD_SP_HL => ld_sp_hl(self, board),
            LD_A_xa16x => ld8(self, board, A, ImmAddr),
            EI => self.set_ime(board, true),
            NOT_USED_8 => self.set_halt_state(board, HaltState::Stuck),
            NOT_USED_9 => self.set_halt_state(board, HaltState::Stuck),
            CP_d8 => drop(cp8(self, board, Imm8)),
            RST_38H => rst(self, board, 0x38),
        }
    }

    fn fetch_execute_cb<B: Board>(&mut self, board: &mut B) {
        use CBByteInstr::*;
        use R16::HL;
        use R8::*;

        let instr = self.fetch_cb(board);
        board.push_cpu_evt(CpuEvt::ExecCB(instr));

        match instr {
            RLC_B => rlc(self, board, B),
            RLC_C => rlc(self, board, C),
            RLC_D => rlc(self, board, D),
            RLC_E => rlc(self, board, E),
            RLC_H => rlc(self, board, H),
            RLC_L => rlc(self, board, L),
            RLC_xHLx => rlc(self, board, HL),
            RLC_A => rlc(self, board, A),
            RRC_B => rrc(self, board, B),
            RRC_C => rrc(self, board, C),
            RRC_D => rrc(self, board, D),
            RRC_E => rrc(self, board, E),
            RRC_H => rrc(self, board, H),
            RRC_L => rrc(self, board, L),
            RRC_xHLx => rrc(self, board, HL),
            RRC_A => rrc(self, board, A),
            RL_B => rl(self, board, B),
            RL_C => rl(self, board, C),
            RL_D => rl(self, board, D),
            RL_E => rl(self, board, E),
            RL_H => rl(self, board, H),
            RL_L => rl(self, board, L),
            RL_xHLx => rl(self, board, HL),
            RL_A => rl(self, board, A),
            RR_B => rr(self, board, B),
            RR_C => rr(self, board, C),
            RR_D => rr(self, board, D),
            RR_E => rr(self, board, E),
            RR_H => rr(self, board, H),
            RR_L => rr(self, board, L),
            RR_xHLx => rr(self, board, HL),
            RR_A => rr(self, board, A),
            SLA_B => sla(self, board, B),
            SLA_C => sla(self, board, C),
            SLA_D => sla(self, board, D),
            SLA_E => sla(self, board, E),
            SLA_H => sla(self, board, H),
            SLA_L => sla(self, board, L),
            SLA_xHLx => sla(self, board, HL),
            SLA_A => sla(self, board, A),
            SRA_B => sra(self, board, B),
            SRA_C => sra(self, board, C),
            SRA_D => sra(self, board, D),
            SRA_E => sra(self, board, E),
            SRA_H => sra(self, board, H),
            SRA_L => sra(self, board, L),
            SRA_xHLx => sra(self, board, HL),
            SRA_A => sra(self, board, A),
            SWAP_B => swap(self, board, B),
            SWAP_C => swap(self, board, C),
            SWAP_D => swap(self, board, D),
            SWAP_E => swap(self, board, E),
            SWAP_H => swap(self, board, H),
            SWAP_L => swap(self, board, L),
            SWAP_xHLx => swap(self, board, HL),
            SWAP_A => swap(self, board, A),
            SRL_B => srl(self, board, B),
            SRL_C => srl(self, board, C),
            SRL_D => srl(self, board, D),
            SRL_E => srl(self, board, E),
            SRL_H => srl(self, board, H),
            SRL_L => srl(self, board, L),
            SRL_xHLx => srl(self, board, HL),
            SRL_A => srl(self, board, A),
            BIT_0_B => bit(self, board, 0, B),
            BIT_0_C => bit(self, board, 0, C),
            BIT_0_D => bit(self, board, 0, D),
            BIT_0_E => bit(self, board, 0, E),
            BIT_0_H => bit(self, board, 0, H),
            BIT_0_L => bit(self, board, 0, L),
            BIT_0_xHLx => bit(self, board, 0, HL),
            BIT_0_A => bit(self, board, 0, A),
            BIT_1_B => bit(self, board, 1, B),
            BIT_1_C => bit(self, board, 1, C),
            BIT_1_D => bit(self, board, 1, D),
            BIT_1_E => bit(self, board, 1, E),
            BIT_1_H => bit(self, board, 1, H),
            BIT_1_L => bit(self, board, 1, L),
            BIT_1_xHLx => bit(self, board, 1, HL),
            BIT_1_A => bit(self, board, 1, A),
            BIT_2_B => bit(self, board, 2, B),
            BIT_2_C => bit(self, board, 2, C),
            BIT_2_D => bit(self, board, 2, D),
            BIT_2_E => bit(self, board, 2, E),
            BIT_2_H => bit(self, board, 2, H),
            BIT_2_L => bit(self, board, 2, L),
            BIT_2_xHLx => bit(self, board, 2, HL),
            BIT_2_A => bit(self, board, 2, A),
            BIT_3_B => bit(self, board, 3, B),
            BIT_3_C => bit(self, board, 3, C),
            BIT_3_D => bit(self, board, 3, D),
            BIT_3_E => bit(self, board, 3, E),
            BIT_3_H => bit(self, board, 3, H),
            BIT_3_L => bit(self, board, 3, L),
            BIT_3_xHLx => bit(self, board, 3, HL),
            BIT_3_A => bit(self, board, 3, A),
            BIT_4_B => bit(self, board, 4, B),
            BIT_4_C => bit(self, board, 4, C),
            BIT_4_D => bit(self, board, 4, D),
            BIT_4_E => bit(self, board, 4, E),
            BIT_4_H => bit(self, board, 4, H),
            BIT_4_L => bit(self, board, 4, L),
            BIT_4_xHLx => bit(self, board, 4, HL),
            BIT_4_A => bit(self, board, 4, A),
            BIT_5_B => bit(self, board, 5, B),
            BIT_5_C => bit(self, board, 5, C),
            BIT_5_D => bit(self, board, 5, D),
            BIT_5_E => bit(self, board, 5, E),
            BIT_5_H => bit(self, board, 5, H),
            BIT_5_L => bit(self, board, 5, L),
            BIT_5_xHLx => bit(self, board, 5, HL),
            BIT_5_A => bit(self, board, 5, A),
            BIT_6_B => bit(self, board, 6, B),
            BIT_6_C => bit(self, board, 6, C),
            BIT_6_D => bit(self, board, 6, D),
            BIT_6_E => bit(self, board, 6, E),
            BIT_6_H => bit(self, board, 6, H),
            BIT_6_L => bit(self, board, 6, L),
            BIT_6_xHLx => bit(self, board, 6, HL),
            BIT_6_A => bit(self, board, 6, A),
            BIT_7_B => bit(self, board, 7, B),
            BIT_7_C => bit(self, board, 7, C),
            BIT_7_D => bit(self, board, 7, D),
            BIT_7_E => bit(self, board, 7, E),
            BIT_7_H => bit(self, board, 7, H),
            BIT_7_L => bit(self, board, 7, L),
            BIT_7_xHLx => bit(self, board, 7, HL),
            BIT_7_A => bit(self, board, 7, A),
            RES_0_B => res(self, board, 0, B),
            RES_0_C => res(self, board, 0, C),
            RES_0_D => res(self, board, 0, D),
            RES_0_E => res(self, board, 0, E),
            RES_0_H => res(self, board, 0, H),
            RES_0_L => res(self, board, 0, L),
            RES_0_xHLx => res(self, board, 0, HL),
            RES_0_A => res(self, board, 0, A),
            RES_1_B => res(self, board, 1, B),
            RES_1_C => res(self, board, 1, C),
            RES_1_D => res(self, board, 1, D),
            RES_1_E => res(self, board, 1, E),
            RES_1_H => res(self, board, 1, H),
            RES_1_L => res(self, board, 1, L),
            RES_1_xHLx => res(self, board, 1, HL),
            RES_1_A => res(self, board, 1, A),
            RES_2_B => res(self, board, 2, B),
            RES_2_C => res(self, board, 2, C),
            RES_2_D => res(self, board, 2, D),
            RES_2_E => res(self, board, 2, E),
            RES_2_H => res(self, board, 2, H),
            RES_2_L => res(self, board, 2, L),
            RES_2_xHLx => res(self, board, 2, HL),
            RES_2_A => res(self, board, 2, A),
            RES_3_B => res(self, board, 3, B),
            RES_3_C => res(self, board, 3, C),
            RES_3_D => res(self, board, 3, D),
            RES_3_E => res(self, board, 3, E),
            RES_3_H => res(self, board, 3, H),
            RES_3_L => res(self, board, 3, L),
            RES_3_xHLx => res(self, board, 3, HL),
            RES_3_A => res(self, board, 3, A),
            RES_4_B => res(self, board, 4, B),
            RES_4_C => res(self, board, 4, C),
            RES_4_D => res(self, board, 4, D),
            RES_4_E => res(self, board, 4, E),
            RES_4_H => res(self, board, 4, H),
            RES_4_L => res(self, board, 4, L),
            RES_4_xHLx => res(self, board, 4, HL),
            RES_4_A => res(self, board, 4, A),
            RES_5_B => res(self, board, 5, B),
            RES_5_C => res(self, board, 5, C),
            RES_5_D => res(self, board, 5, D),
            RES_5_E => res(self, board, 5, E),
            RES_5_H => res(self, board, 5, H),
            RES_5_L => res(self, board, 5, L),
            RES_5_xHLx => res(self, board, 5, HL),
            RES_5_A => res(self, board, 5, A),
            RES_6_B => res(self, board, 6, B),
            RES_6_C => res(self, board, 6, C),
            RES_6_D => res(self, board, 6, D),
            RES_6_E => res(self, board, 6, E),
            RES_6_H => res(self, board, 6, H),
            RES_6_L => res(self, board, 6, L),
            RES_6_xHLx => res(self, board, 6, HL),
            RES_6_A => res(self, board, 6, A),
            RES_7_B => res(self, board, 7, B),
            RES_7_C => res(self, board, 7, C),
            RES_7_D => res(self, board, 7, D),
            RES_7_E => res(self, board, 7, E),
            RES_7_H => res(self, board, 7, H),
            RES_7_L => res(self, board, 7, L),
            RES_7_xHLx => res(self, board, 7, HL),
            RES_7_A => res(self, board, 7, A),
            SET_0_B => set(self, board, 0, B),
            SET_0_C => set(self, board, 0, C),
            SET_0_D => set(self, board, 0, D),
            SET_0_E => set(self, board, 0, E),
            SET_0_H => set(self, board, 0, H),
            SET_0_L => set(self, board, 0, L),
            SET_0_xHLx => set(self, board, 0, HL),
            SET_0_A => set(self, board, 0, A),
            SET_1_B => set(self, board, 1, B),
            SET_1_C => set(self, board, 1, C),
            SET_1_D => set(self, board, 1, D),
            SET_1_E => set(self, board, 1, E),
            SET_1_H => set(self, board, 1, H),
            SET_1_L => set(self, board, 1, L),
            SET_1_xHLx => set(self, board, 1, HL),
            SET_1_A => set(self, board, 1, A),
            SET_2_B => set(self, board, 2, B),
            SET_2_C => set(self, board, 2, C),
            SET_2_D => set(self, board, 2, D),
            SET_2_E => set(self, board, 2, E),
            SET_2_H => set(self, board, 2, H),
            SET_2_L => set(self, board, 2, L),
            SET_2_xHLx => set(self, board, 2, HL),
            SET_2_A => set(self, board, 2, A),
            SET_3_B => set(self, board, 3, B),
            SET_3_C => set(self, board, 3, C),
            SET_3_D => set(self, board, 3, D),
            SET_3_E => set(self, board, 3, E),
            SET_3_H => set(self, board, 3, H),
            SET_3_L => set(self, board, 3, L),
            SET_3_xHLx => set(self, board, 3, HL),
            SET_3_A => set(self, board, 3, A),
            SET_4_B => set(self, board, 4, B),
            SET_4_C => set(self, board, 4, C),
            SET_4_D => set(self, board, 4, D),
            SET_4_E => set(self, board, 4, E),
            SET_4_H => set(self, board, 4, H),
            SET_4_L => set(self, board, 4, L),
            SET_4_xHLx => set(self, board, 4, HL),
            SET_4_A => set(self, board, 4, A),
            SET_5_B => set(self, board, 5, B),
            SET_5_C => set(self, board, 5, C),
            SET_5_D => set(self, board, 5, D),
            SET_5_E => set(self, board, 5, E),
            SET_5_H => set(self, board, 5, H),
            SET_5_L => set(self, board, 5, L),
            SET_5_xHLx => set(self, board, 5, HL),
            SET_5_A => set(self, board, 5, A),
            SET_6_B => set(self, board, 6, B),
            SET_6_C => set(self, board, 6, C),
            SET_6_D => set(self, board, 6, D),
            SET_6_E => set(self, board, 6, E),
            SET_6_H => set(self, board, 6, H),
            SET_6_L => set(self, board, 6, L),
            SET_6_xHLx => set(self, board, 6, HL),
            SET_6_A => set(self, board, 6, A),
            SET_7_B => set(self, board, 7, B),
            SET_7_C => set(self, board, 7, C),
            SET_7_D => set(self, board, 7, D),
            SET_7_E => set(self, board, 7, E),
            SET_7_H => set(self, board, 7, H),
            SET_7_L => set(self, board, 7, L),
            SET_7_xHLx => set(self, board, 7, HL),
            SET_7_A => set(self, board, 7, A),
        }
    }
}
