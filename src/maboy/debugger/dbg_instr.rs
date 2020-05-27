use crate::maboy::cpu::{ByteInstr, CBByteInstr};
pub(super) enum OperandType {
    /// 8 bit arbitrary data
    D8,

    /// 16 bit arbitrary data
    D16,

    /// Unsigned lower 8 bits of an address, with the upper 8 bits being 0xFF
    A8,

    /// Unsigned memory address
    A16,

    /// 2s complement signed 8 bit address offset
    R8,

    /// Instruction with a 0xCB prefix
    PrefixInstr,

    /// Since the STOP instruction is two bytes long, we treat the second
    /// byte as an operand. If the second byte is not 0x00, we call the
    /// STOP instruction 'corrupted STOP'. It does weird stuff.
    StopOperand,
}

impl ByteInstr {
    /// Technically we don't need this for the emulator, but it is
    /// very useful for the debugger.
    pub(super) fn operand_type(self) -> Option<OperandType> {
        use ByteInstr::*;
        use OperandType::*;

        match self {
            NOP => None,
            LD_BC_d16 => Some(D16),
            LD_xBCx_A => None,
            INC_BC => None,
            INC_B => None,
            DEC_B => None,
            LD_B_d8 => Some(D8),
            RLCA => None,
            LD_xa16x_SP => Some(A16),
            ADD_HL_BC => None,
            LD_A_xBCx => None,
            DEC_BC => None,
            INC_C => None,
            DEC_C => None,
            LD_C_d8 => Some(D8),
            RRCA => None,
            /// This is sort of a special case - STOP is actually a 2 byte instruction (0x1000),
            /// with corrupted stop instructions (0x10nn) performing weird stuff.
            STOP => Some(StopOperand),
            LD_DE_d16 => Some(D16),
            LD_xDEx_A => None,
            INC_DE => None,
            INC_D => None,
            DEC_D => None,
            LD_D_d8 => Some(D8),
            RLA => None,
            JR_r8 => Some(R8),
            ADD_HL_DE => None,
            LD_A_xDEx => None,
            DEC_DE => None,
            INC_E => None,
            DEC_E => None,
            LD_E_d8 => Some(D8),
            RRA => None,
            JR_NZ_r8 => Some(R8),
            LD_HL_d16 => Some(D16),
            LD_xHLix_A => None,
            INC_HL => None,
            INC_H => None,
            DEC_H => None,
            LD_H_d8 => Some(D8),
            DAA => None,
            JR_Z_r8 => Some(R8),
            ADD_HL_HL => None,
            LD_A_xHLix => None,
            DEC_HL => None,
            INC_L => None,
            DEC_L => None,
            LD_L_d8 => Some(D8),
            CPL => None,
            JR_NC_r8 => Some(R8),
            LD_SP_d16 => Some(D16),
            LD_xHLdx_A => None,
            INC_SP => None,
            INC_xHLx => None,
            DEC_xHLx => None,
            LD_xHLx_d8 => Some(D8),
            SCF => None,
            JR_C_r8 => Some(R8),
            ADD_HL_SP => None,
            LD_A_xHLdx => None,
            DEC_SP => None,
            INC_A => None,
            DEC_A => None,
            LD_A_d8 => Some(D8),
            CCF => None,
            LD_B_B => None,
            LD_B_C => None,
            LD_B_D => None,
            LD_B_E => None,
            LD_B_H => None,
            LD_B_L => None,
            LD_B_xHLx => None,
            LD_B_A => None,
            LD_C_B => None,
            LD_C_C => None,
            LD_C_D => None,
            LD_C_E => None,
            LD_C_H => None,
            LD_C_L => None,
            LD_C_xHLx => None,
            LD_C_A => None,
            LD_D_B => None,
            LD_D_C => None,
            LD_D_D => None,
            LD_D_E => None,
            LD_D_H => None,
            LD_D_L => None,
            LD_D_xHLx => None,
            LD_D_A => None,
            LD_E_B => None,
            LD_E_C => None,
            LD_E_D => None,
            LD_E_E => None,
            LD_E_H => None,
            LD_E_L => None,
            LD_E_xHLx => None,
            LD_E_A => None,
            LD_H_B => None,
            LD_H_C => None,
            LD_H_D => None,
            LD_H_E => None,
            LD_H_H => None,
            LD_H_L => None,
            LD_H_xHLx => None,
            LD_H_A => None,
            LD_L_B => None,
            LD_L_C => None,
            LD_L_D => None,
            LD_L_E => None,
            LD_L_H => None,
            LD_L_L => None,
            LD_L_xHLx => None,
            LD_L_A => None,
            LD_xHLx_B => None,
            LD_xHLx_C => None,
            LD_xHLx_D => None,
            LD_xHLx_E => None,
            LD_xHLx_H => None,
            LD_xHLx_L => None,
            HALT => None,
            LD_xHLx_A => None,
            LD_A_B => None,
            LD_A_C => None,
            LD_A_D => None,
            LD_A_E => None,
            LD_A_H => None,
            LD_A_L => None,
            LD_A_xHLx => None,
            LD_A_A => None,
            ADD_A_B => None,
            ADD_A_C => None,
            ADD_A_D => None,
            ADD_A_E => None,
            ADD_A_H => None,
            ADD_A_L => None,
            ADD_A_xHLx => None,
            ADD_A_A => None,
            ADC_A_B => None,
            ADC_A_C => None,
            ADC_A_D => None,
            ADC_A_E => None,
            ADC_A_H => None,
            ADC_A_L => None,
            ADC_A_xHLx => None,
            ADC_A_A => None,
            SUB_B => None,
            SUB_C => None,
            SUB_D => None,
            SUB_E => None,
            SUB_H => None,
            SUB_L => None,
            SUB_xHLx => None,
            SUB_A => None,
            SBC_A_B => None,
            SBC_A_C => None,
            SBC_A_D => None,
            SBC_A_E => None,
            SBC_A_H => None,
            SBC_A_L => None,
            SBC_A_xHLx => None,
            SBC_A_A => None,
            AND_B => None,
            AND_C => None,
            AND_D => None,
            AND_E => None,
            AND_H => None,
            AND_L => None,
            AND_xHLx => None,
            AND_A => None,
            XOR_B => None,
            XOR_C => None,
            XOR_D => None,
            XOR_E => None,
            XOR_H => None,
            XOR_L => None,
            XOR_xHLx => None,
            XOR_A => None,
            OR_B => None,
            OR_C => None,
            OR_D => None,
            OR_E => None,
            OR_H => None,
            OR_L => None,
            OR_xHLx => None,
            OR_A => None,
            CP_B => None,
            CP_C => None,
            CP_D => None,
            CP_E => None,
            CP_H => None,
            CP_L => None,
            CP_xHLx => None,
            CP_A => None,
            RET_NZ => None,
            POP_BC => None,
            JP_NZ_a16 => Some(A16),
            JP_a16 => Some(A16),
            CALL_NZ_a16 => Some(A16),
            PUSH_BC => None,
            ADD_A_d8 => Some(D8),
            RST_00H => None,
            RET_Z => None,
            RET => None,
            JP_Z_a16 => Some(A16),
            PREFIX_CB => Some(PrefixInstr),
            CALL_Z_a16 => Some(A16),
            CALL_a16 => Some(A16),
            ADC_A_d8 => Some(D8),
            RST_08H => None,
            RET_NC => None,
            POP_DE => None,
            JP_NC_a16 => Some(A16),
            NOT_USED => None,
            CALL_NC_a16 => Some(A16),
            PUSH_DE => None,
            SUB_d8 => Some(D8),
            RST_10H => None,
            RET_C => None,
            RETI => None,
            JP_C_a16 => Some(A16),
            NOT_USED_0 => None,
            CALL_C_a16 => Some(A16),
            NOT_USED_1 => None,
            SBC_A_d8 => Some(D8),
            RST_18H => None,
            LDH_xa8x_A => Some(A8),
            POP_HL => None,
            LD_xCx_A => None,
            NOT_USED_2 => None,
            NOT_USED_3 => None,
            PUSH_HL => None,
            AND_d8 => Some(D8),
            RST_20H => None,
            ADD_SP_r8 => Some(R8),
            JP_xHLx => None,
            LD_xa16x_A => Some(A16),
            NOT_USED_4 => None,
            NOT_USED_5 => None,
            NOT_USED_6 => None,
            XOR_d8 => Some(D8),
            RST_28H => None,
            LDH_A_xa8x => Some(A8),
            POP_AF => None,
            LD_A_xCx => None,
            DI => None,
            NOT_USED_7 => None,
            PUSH_AF => None,
            OR_d8 => Some(D8),
            RST_30H => None,
            LD_HL_SPpr8 => Some(R8),
            LD_SP_HL => None,
            LD_A_xa16x => Some(A16),
            EI => None,
            NOT_USED_8 => None,
            NOT_USED_9 => None,
            CP_d8 => Some(D8),
            RST_38H => None,
        }
    }

    pub(super) fn is__control_flow_change(&self) -> bool {
        match self {
            // Unconditional
            ByteInstr::JR_r8 => true,
            ByteInstr::JP_a16 => true,
            ByteInstr::JP_xHLx => true,
            ByteInstr::RET => true,
            ByteInstr::RETI => true,
            ByteInstr::CALL_a16 => true,
            ByteInstr::RST_00H => true,
            ByteInstr::RST_08H => true,
            ByteInstr::RST_10H => true,
            ByteInstr::RST_18H => true,
            ByteInstr::RST_20H => true,
            ByteInstr::RST_28H => true,
            ByteInstr::RST_30H => true,
            ByteInstr::RST_38H => true,

            // Conditional
            ByteInstr::JR_NZ_r8 => true,
            ByteInstr::JR_Z_r8 => true,
            ByteInstr::JR_NC_r8 => true,
            ByteInstr::JR_C_r8 => true,
            ByteInstr::RET_NZ => true,
            ByteInstr::RET_Z => true,
            ByteInstr::RET_NC => true,
            ByteInstr::RET_C => true,
            ByteInstr::JP_NZ_a16 => true,
            ByteInstr::JP_Z_a16 => true,
            ByteInstr::JP_NC_a16 => true,
            ByteInstr::JP_C_a16 => true,
            ByteInstr::CALL_NZ_a16 => true,
            ByteInstr::CALL_Z_a16 => true,
            ByteInstr::CALL_NC_a16 => true,
            ByteInstr::CALL_C_a16 => true,

            _ => false,
        }
    }
}

impl OperandType {
    /// Length of operator (without instruction) in bytes
    pub(super) fn len(&self) -> u8 {
        match self {
            OperandType::D8 => 1,
            OperandType::D16 => 2,
            OperandType::A8 => 1,
            OperandType::A16 => 2,
            OperandType::R8 => 1,
            OperandType::PrefixInstr => 1,
            OperandType::StopOperand => 1,
        }
    }
}