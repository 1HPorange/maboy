use super::dbg_instr::OperandType;
use super::fmt::FmtNum;
use crate::maboy::cartridge::CartridgeMem;
use crate::maboy::{
    address::Addr,
    board::Board,
    cpu::{ByteInstr, CBByteInstr, Registers, CPU, R16, R8},
};
use console::{style, Style, StyledObject, Term};

// TODO: When printing upcoming instructions, keep in mind that
// we cannot know those instructions if they set in IO registers

#[derive(Copy, Clone)]
pub struct BreakPoint(pub u16);

fn print_cpu_state(term: &Term, reg: &Registers) {
    use R16::*;
    use R8::*;

    term.write_line(&format!(
        " PC: {}, SP: {}, BC: {}, DE: {}, HL: {}",
        reg.r16(PC).fmt(),
        reg.r16(SP).fmt(),
        reg.r16(BC).fmt(),
        reg.r16(DE).fmt(),
        reg.r16(HL).fmt(),
    ))
    .unwrap();

    term.write_line(&format!(
        " A: {}, B: {}, C: {}, D: {}, E: {}, H: {}, L: {}",
        reg.r8(A).fmt(),
        reg.r8(B).fmt(),
        reg.r8(C).fmt(),
        reg.r8(D).fmt(),
        reg.r8(E).fmt(),
        reg.r8(H).fmt(),
        reg.r8(L).fmt()
    ))
    .unwrap();

    term.write_line(&format!(
        " Flags: {}",
        style(format!("{:?}", reg.flags())).green()
    ))
    .unwrap();
}
