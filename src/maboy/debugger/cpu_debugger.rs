use super::{dbg_instr::OperandType, Debugger};
use crate::maboy::cartridge::CartridgeMem;
use crate::maboy::{
    address::Addr,
    board::Board,
    cpu::{ByteInstr, CBByteInstr, Registers, CPU, R16, R8},
};
use console::{style, Style, StyledObject, Term};

#[derive(Copy, Clone)]
pub struct BreakPoint(pub u16);

pub struct CpuDebugger {
    pub breakpoints: Vec<BreakPoint>,
    break_in: Option<u16>,
    last_instr_pc: u16,
}

impl Debugger for CpuDebugger {
    fn update<C: CartridgeMem>(&mut self, cpu: &mut CPU, board: &mut Board<C>) {
        self.update(cpu, board);
        self.last_instr_pc = cpu.reg.pc();
    }

    fn schedule_break(&mut self) {
        self.break_in = Some(0);
    }
}

impl CpuDebugger {
    pub fn new() -> Self {
        Self {
            breakpoints: Vec::new(),
            break_in: None,
            last_instr_pc: 0,
        }
    }

    fn run_debug_cli<C: CartridgeMem>(&mut self, cpu: &mut CPU, board: &mut Board<C>) {
        let term = Term::stdout();

        // TODO: Write this all using a single mutable String, to which we push lines
        loop {
            term.clear_screen().unwrap();

            term.write_line("CPU").unwrap();
            print_cpu_registers(&term, &cpu.reg);

            term.write_line("\nMem").unwrap();
            self.print_last_instr(&term);
            print_next_instr(&term, board, cpu.reg.pc(), &self.breakpoints, 30);

            let user_command = term.read_line_initial_text("\n:").unwrap();

            break;
        }

        term.clear_screen().unwrap();
    }

    fn update<C: CartridgeMem>(&mut self, cpu: &mut CPU, board: &mut Board<C>) {
        if let Some(instr_count) = &mut self.break_in {
            if *instr_count == 0 {
                self.break_in = None;
                self.run_debug_cli(cpu, board);
                return;
            } else {
                self.break_in = Some(*instr_count - 1);
            }
        }

        let next_instr_base = cpu.reg.pc();
        let next_instr: ByteInstr =
            unsafe { std::mem::transmute(board.read8_instant(Addr::from(next_instr_base))) };
        let next_instr_end = next_instr_base
            .saturating_add(next_instr.operand_type().map(|o| o.len()).unwrap_or(1) as u16);

        for BreakPoint(bp_addr) in &self.breakpoints {
            if *bp_addr >= next_instr_base && *bp_addr <= next_instr_end {
                // We hit a breakpoint. For now, let's just run a debug CLI here.
                self.run_debug_cli(cpu, board);
                break;
            }
        }
    }

    fn print_last_instr(&self, term: &Term) {
        let line = format!(" [{:#06X}] Previous Instruction", self.last_instr_pc);
        term.write_line(&style(line).cyan().to_string()).unwrap();
    }
}

fn print_cpu_registers(term: &Term, reg: &Registers) {
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

fn print_next_instr<C: CartridgeMem>(
    term: &Term,
    board: &Board<C>,
    mut pc: u16,
    breakpoints: &[BreakPoint],
    len: u8,
) {
    for _ in 0..len {
        let instr: ByteInstr = unsafe { std::mem::transmute(board.read8_instant(Addr::from(pc))) };

        let instr_text = if let ByteInstr::PREFIX_CB = instr {
            let cb_instr: CBByteInstr =
                unsafe { std::mem::transmute(board.read8_instant(Addr::from(pc.wrapping_add(1)))) };
            format!("{:?}", cb_instr)
        } else {
            format!("{:?}", instr)
        };

        let operand = instr.operand_type().map(|o| o.fmt(board, pc)).flatten();

        term.write_line(&format!(
            " [{}] {} {}",
            pc.fmt(),
            instr_text,
            operand.map(|s| s.to_string()).unwrap_or(String::new())
        ))
        .unwrap();

        if instr.is_control_flow_change() {
            break;
        }

        pc = pc.wrapping_add(instr.operand_type().map(|o| o.len() + 1).unwrap_or(1) as u16);
    }
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
        style(format!("{:#06X}", self)).yellow()
    }
}

impl OperandType {
    fn fmt<C: CartridgeMem>(self, board: &Board<C>, pc: u16) -> Option<StyledObject<String>> {
        let pc = pc.wrapping_add(1);

        Some(match self {
            OperandType::D8 => {
                style(format!("{:#04X}", board.read8_instant(Addr::from(pc)))).blue()
            }
            OperandType::D16 => style(format!("{:#06X}", board.read16_instant(pc))).blue(),
            OperandType::A8 => {
                style(format!("{:#04X}", board.read8_instant(Addr::from(pc)))).yellow()
            }
            OperandType::A16 => style(format!("{:#06X}", board.read16_instant(pc))).yellow(),
            OperandType::R8 => style("Too lazy to implement".to_owned()).red(),
            OperandType::PrefixInstr => return None,
            OperandType::StopOperand => {
                if board.read8_instant(Addr::from(pc)) == 0x00 {
                    return None;
                } else {
                    style("CORRUPTED!".to_owned()).red()
                }
            }
        })
    }
}
