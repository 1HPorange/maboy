use super::dbg_instr::OperandType;
use crate::maboy::address::{Addr, IOReg};
use crate::maboy::{board::Board, cpu::CBByteInstr};
use console::{style, StyledObject};
use std::convert::TryFrom;

pub trait FmtNum {
    fn fmt_val(self) -> StyledObject<String>;
    fn fmt_addr(self) -> StyledObject<String>;
}

impl FmtNum for u8 {
    fn fmt_val(self) -> StyledObject<String> {
        style(format!("{} ({:#04X})", self, self)).blue()
    }

    fn fmt_addr(self) -> StyledObject<String> {
        style(format!("{:#04X}", self)).yellow()
    }
}

impl FmtNum for u16 {
    fn fmt_val(self) -> StyledObject<String> {
        style(format!("{} ({:#06X})", self, self)).blue()
    }

    fn fmt_addr(self) -> StyledObject<String> {
        match IOReg::try_from(self) {
            Ok(reg) => style(format!("{:?}", reg)).green(),
            _ => style(format!("{:#06X}", self)).yellow(),
        }
    }
}

impl OperandType {
    // TODO: Handle non-constant PC values here (basically everything
    // that external components like MBCs and the PPU can influence)
    pub fn fmt<B: Board>(self, board: &B, pc: u16) -> StyledObject<String> {
        let pc = pc.wrapping_add(1);

        match self {
            OperandType::D8 => board.read8_instant(Addr::from(pc)).fmt_val(),
            OperandType::D16 => board.read16_instant(pc).fmt_val(),
            OperandType::A8 => (0xFF00 + board.read8_instant(Addr::from(pc)) as u16).fmt_addr(),
            OperandType::A16 => board.read16_instant(pc).fmt_addr(),
            OperandType::R8 => {
                let offset = board.read8_instant(Addr::from(pc)) as i8;
                (pc.wrapping_add(offset as u16)).fmt_addr()
            }
            OperandType::PrefixInstr => {
                let instr: CBByteInstr =
                    unsafe { std::mem::transmute(board.read8_instant(Addr::from(pc))) };
                style(format!("{:?}", instr)).blue()
            }
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
