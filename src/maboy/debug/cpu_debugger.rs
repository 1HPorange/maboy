use super::dbg_instr::OperandType;
use super::{fmt::FmtNum, CpuEvt, DbgEvtLogger, DbgEvtSrc, PpuEvt};
use crate::maboy::cartridge::CartridgeMem;
use crate::maboy::{
    address::Addr,
    board::Board,
    cpu::{ByteInstr, CBByteInstr, Registers, CPU, R16, R8},
    Emulator,
};
use console::{style, Style, StyledObject, Term};
use std::fmt::Write;
use std::time::Duration;

// TODO: When printing upcoming instructions, keep in mind that
// we cannot know those instructions if they live in IO registers
// or MBC-switched areas

pub struct CpuDebugger {
    pub breakpoints: Vec<u16>,
    break_in: Option<u16>,
    output_buffer: String,
}

impl CpuDebugger {
    pub fn new() -> CpuDebugger {
        CpuDebugger {
            breakpoints: Vec::new(),
            break_in: None,
            output_buffer: String::new(),
        }
    }

    /// Call this *before* calling Emulator::emulate_step()
    pub fn try_run_blocking<CMem: CartridgeMem, PpuDbg: DbgEvtSrc<PpuEvt>>(
        &mut self,
        emu: &Emulator<CMem, DbgEvtLogger<CpuEvt>, PpuDbg>,
    ) {
        if !self.break_cond_met(emu) {
            return;
        }

        self.output_buffer.clear();

        writeln!(&mut self.output_buffer, "CPU").unwrap();
        self.print_cpu_state(&emu.cpu.reg);

        writeln!(&mut self.output_buffer, "\nMem").unwrap();
        self.print_preceding_instr(emu);
        self.print_upcoming_instr(&emu.cpu, &emu.board);

        let term = Term::stdout();
        term.clear_screen().unwrap();

        term.write_line(&self.output_buffer).unwrap();

        loop {
            term.write_str("\nEnter command: ").unwrap();
            let command = term.read_line().unwrap();

            match &command[..] {
                "run" => break,
                _ => (),
            }
        }

        term.clear_screen().unwrap();
    }

    fn break_cond_met<CMem: CartridgeMem, PpuDbg: DbgEvtSrc<PpuEvt>>(
        &mut self,
        emu: &Emulator<CMem, DbgEvtLogger<CpuEvt>, PpuDbg>,
    ) -> bool {
        if let Some(steps) = &mut self.break_in {
            if *steps == 0 {
                self.break_in = None;
                return true;
            } else {
                *steps -= 1;
            }
        }

        let instr_start = emu.cpu.reg.pc();
        let instr: ByteInstr =
            unsafe { std::mem::transmute(emu.board.read8_instant(Addr::from(instr_start))) };
        let instr_end =
            instr_start.wrapping_add(instr.operand_type().map(|o| o.len()).unwrap_or(0) as u16);

        for bp in self.breakpoints.iter().copied() {
            if bp >= instr_start && bp <= instr_end {
                return true;
            }
        }

        false
    }

    pub fn request_break(&mut self) {
        self.break_in(0);
    }

    fn break_in(&mut self, steps: u16) {
        self.break_in = Some(steps);
    }

    fn print_cpu_state(&mut self, reg: &Registers) {
        use R16::*;
        use R8::*;

        writeln!(
            self.output_buffer,
            " PC: {}, SP: {}, BC: {}, DE: {}, HL: {}",
            reg.r16(PC).fmt_val(),
            reg.r16(SP).fmt_val(),
            reg.r16(BC).fmt_val(),
            reg.r16(DE).fmt_val(),
            reg.r16(HL).fmt_val()
        )
        .unwrap();

        writeln!(
            self.output_buffer,
            " A: {}, B: {}, C: {}, D: {}, E: {}, H: {}, L: {}",
            reg.r8(A).fmt_val(),
            reg.r8(B).fmt_val(),
            reg.r8(C).fmt_val(),
            reg.r8(D).fmt_val(),
            reg.r8(E).fmt_val(),
            reg.r8(H).fmt_val(),
            reg.r8(L).fmt_val()
        )
        .unwrap();

        writeln!(
            self.output_buffer,
            " Flags: {}",
            style(format!("{:?}", reg.flags())).green()
        )
        .unwrap();
    }

    fn print_preceding_instr<CMem: CartridgeMem, PpuDbg: DbgEvtSrc<PpuEvt>>(
        &mut self,
        emu: &Emulator<CMem, DbgEvtLogger<CpuEvt>, PpuDbg>,
    ) {
        for evt in emu.board.cpu_evt_src.evts() {
            match evt {
                CpuEvt::Exec(pc, instr) => {
                    write!(self.output_buffer, "\n [{}] {:?}", pc.fmt_addr(), instr).unwrap()
                }
                CpuEvt::ExecCB(instr) => {
                    write!(self.output_buffer, "\n  Executing {:?}", instr).unwrap()
                }
                CpuEvt::ReadMem(addr, val) => write!(
                    self.output_buffer,
                    "\n  Read {} from {}",
                    val.fmt_val(),
                    addr.fmt_addr(),
                )
                .unwrap(),
                CpuEvt::WriteMem(addr, val) => write!(
                    self.output_buffer,
                    "\n  Write {} to {}",
                    val.fmt_val(),
                    addr.fmt_addr(),
                )
                .unwrap(),
                CpuEvt::HandleIR(ir) => write!(
                    self.output_buffer,
                    "\n Jumping to {:?} interrupt handler",
                    ir
                )
                .unwrap(),
                CpuEvt::TakeJmpTo(addr) => write!(
                    self.output_buffer,
                    "\n {} {}",
                    style("Taking jump to:").green(),
                    addr.fmt_addr()
                )
                .unwrap(),
                CpuEvt::SkipJmpTo(addr) => write!(
                    self.output_buffer,
                    "\n {} {}",
                    style("Skipping jump to").red(),
                    addr.fmt_addr()
                )
                .unwrap(),
                CpuEvt::EnterHalt(halt_state) => write!(
                    self.output_buffer,
                    "\n {} {:?}",
                    style("Entering halt state:").red(),
                    halt_state
                )
                .unwrap(),
            }
        }
    }

    fn print_upcoming_instr<B: Board>(&mut self, cpu: &CPU, board: &B) {
        let mut pc = cpu.reg.pc();
        let instr: ByteInstr =
            unsafe { std::mem::transmute(board.read8_instant(Addr::from(cpu.reg.pc()))) };

        pc = self.print_single_instr(board, pc, instr);

        write!(self.output_buffer, "{}", style(" <- You are here").green()).unwrap();

        if instr.is_control_flow_change() {
            return;
        }

        for _ in 0..10 {
            let instr: ByteInstr =
                unsafe { std::mem::transmute(board.read8_instant(Addr::from(cpu.reg.pc()))) };

            if instr.is_control_flow_change() {
                return;
            }

            pc = self.print_single_instr(board, pc, instr);
        }
    }

    /// Returns new PC after reading the instruction
    fn print_single_instr<B: Board>(&mut self, board: &B, pc: u16, instr: ByteInstr) -> u16 {
        if let Some(operand) = instr.operand_type() {
            write!(
                self.output_buffer,
                "\n [{}] {:?} {}",
                pc.fmt_addr(),
                instr,
                operand.fmt(board, pc)
            )
            .unwrap();

            pc.wrapping_add(1 + operand.len() as u16)
        } else {
            write!(self.output_buffer, "\n [{}] {:?}", pc.fmt_addr(), instr).unwrap();

            pc.wrapping_add(1)
        }
    }
}
