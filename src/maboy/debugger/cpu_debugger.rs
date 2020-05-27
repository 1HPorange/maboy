use super::Debugger;
use crate::maboy::cartridge::CartridgeMem;
use crate::maboy::{
    address::Addr,
    board::Board,
    cpu::{ByteInstr, Registers, CPU, R16, R8},
};

#[derive(Copy, Clone)]
pub struct BreakPoint(pub u16);

pub struct CpuDebugger {
    pub breakpoints: Vec<BreakPoint>,
}

impl CpuDebugger {
    pub fn new() -> Self {
        Self {
            breakpoints: Vec::new(),
        }
    }
}

impl Debugger for CpuDebugger {
    fn update<C: CartridgeMem>(&mut self, cpu: &mut CPU, board: &mut Board<C>) {
        let next_instr_base = cpu.reg.pc();
        let next_instr: ByteInstr =
            unsafe { std::mem::transmute(board.read8_instant(Addr::from(next_instr_base))) };
        let next_instr_end = next_instr_base
            .saturating_add(next_instr.operand_type().map(|o| o.len() + 1).unwrap_or(1) as u16);

        for BreakPoint(bp_addr) in &self.breakpoints {
            if *bp_addr >= next_instr_base && *bp_addr <= next_instr_end {
                // We hit a breakpoint. For now, let's just run a debug CLI here.
                self.run_debug_cli(cpu, board);
                break;
            }
        }
    }
}

use console::{style, StyledObject, Term};

impl CpuDebugger {
    fn run_debug_cli<C: CartridgeMem>(&mut self, cpu: &mut CPU, board: &mut Board<C>) {
        let term = Term::stdout();

        loop {
            term.clear_screen().unwrap();

            print_cpu_registers(&term, &cpu.reg);
        }
    }
}

fn print_cpu_registers(term: &Term, reg: &Registers) {
    use R16::*;
    use R8::*;

    term.write_line(&format!(
        "PC: {}, SP: {}, B: {}, C: {}, D: {}, E: {}, H: {}, L: {}, BC: {}, DE: {}, HL: {}",
        reg.pc(),
        reg.sp(),
        reg.r8(B),
        reg.r8(C),
        reg.r8(D),
        reg.r8(E),
        reg.r8(H),
        reg.r8(L),
        reg.r16(BC),
        reg.r16(DE),
        reg.r16(HL),
    ))
    .unwrap();

    term.write_line(&format!("Flags: {:?}", reg.flags()))
        .unwrap();
}

trait FmtNum {
    fn fmt(self) -> StyledObject<String>;
}

impl FmtNum for u8 {
    fn fmt(self) -> StyledObject<String> {
        style(format!("{:#04X}", self)).blue()
    }
}

impl FmtNum for u16 {
    fn fmt(self) -> StyledObject<String> {
        style(format!("{:#04X}", self)).yellow()
    }
}
