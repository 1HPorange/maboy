use super::dbg_instr::OperandType;
use crate::maboy::address::Addr;
use crate::maboy::board::Board;
use console::{style, StyledObject};

/// Formats u8 as blue (value) and u16 as yellow (address)
pub trait FmtNum {
    fn fmt(self) -> StyledObject<String>;
}

impl FmtNum for u8 {
    fn fmt(self) -> StyledObject<String> {
        style(format!("{:#04X}", self)).blue()
    }
}

impl FmtNum for u16 {
    fn fmt(self) -> StyledObject<String> {
        style(format!("{:#06X}", self)).yellow()
    }
}

impl OperandType {
    fn fmt<B: Board>(self, board: &B, pc: u16) -> StyledObject<String> {
        let pc = pc.wrapping_add(1);

        match self {
            OperandType::D8 => {
                style(format!("{:#04X}", board.read8_instant(Addr::from(pc)))).blue()
            }
            OperandType::D16 => style(format!("{:#06X}", board.read16_instant(pc))).blue(),
            OperandType::A8 => {
                style(format!("{:#04X}", board.read8_instant(Addr::from(pc)))).yellow()
            }
            OperandType::A16 => style(format!("{:#06X}", board.read16_instant(pc))).yellow(),
            OperandType::R8 => style("TODO:".to_owned()).red(),
            OperandType::PrefixInstr => style("TODO:".to_owned()).red(),
            OperandType::StopOperand => {
                let operand = board.read8_instant(Addr::from(pc));

                if operand == 0 {
                    style("0x00 (Valid STOP)".to_owned()).green()
                } else {
                    style(format!("{:#04X} (Corrupted STOP)", operand)).red()
                }
            }
        }
    }
}
