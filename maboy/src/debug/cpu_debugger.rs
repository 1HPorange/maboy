use super::{fmt::FmtNum, CpuEvt, DbgEvtLogger, DbgEvtSrc, PpuEvt};
use crate::cartridge::CartridgeMem;
use crate::{
    address::{Addr, PpuReg},
    board::Board,
    cpu::{ByteInstr, Registers, CPU, R16, R8},
    ppu::{LCDC, LCDS, PPU},
    Emulator,
};
use console::{style, StyledObject, Term};
use std::fmt::Write;

// TODO: When printing upcoming instructions, keep in mind that
// we cannot know those instructions if they live in IO registers
// or MBC-switched areas

pub struct CpuDebugger {
    pub breakpoints: Vec<u16>,
    pub mem_breakpoints: Vec<(u16, BreakCond)>,
    break_in: Option<usize>,
    output_buffer: String,
}

#[derive(Debug, Copy, Clone)]
pub enum BreakCond {
    ReadWrite,
    Read,
    Write,
}

enum BreakReason {
    UserRequest,
    BreakpointHit(u16),
    CondBreakpointHit(u16, BreakCond),
}

impl CpuDebugger {
    pub fn new() -> CpuDebugger {
        CpuDebugger {
            breakpoints: Vec::new(),
            mem_breakpoints: Vec::new(),
            break_in: None,
            output_buffer: String::new(),
        }
    }

    /// Call this *before* calling Emulator::emulate_step()
    pub fn try_run_blocking<CMem: CartridgeMem, PpuDbg: DbgEvtSrc<PpuEvt>>(
        &mut self,
        emu: &Emulator<CMem, DbgEvtLogger<CpuEvt>, PpuDbg>,
    ) {
        if let Some(break_reason) = self.break_reason(emu) {
            self.output_buffer.clear();
            self.print_break_reason(break_reason);
        } else {
            return;
        }

        writeln!(self.output_buffer, "CPU").unwrap();
        self.print_cpu_state(&emu.cpu.reg);

        writeln!(self.output_buffer, "\nPPU").unwrap();
        self.print_ppu_state(&emu.board.ppu);

        writeln!(self.output_buffer, "\nMem").unwrap();
        self.print_preceding_instr(emu);
        self.print_upcoming_instr(&emu.cpu, &emu.board);

        let term = Term::stdout();
        term.clear_screen().unwrap();

        term.write_line(&self.output_buffer).unwrap();

        loop {
            term.write_str(&style("Enter command: ").yellow().to_string())
                .unwrap();
            let command = term.read_line().unwrap();

            match &command[..] {
                "run" => break,
                _ if command.starts_with("step") => {
                    if self.cmd_step(&term, command.split_ascii_whitespace().skip(1)) {
                        break;
                    }
                }
                _ if command.starts_with("bp") => {
                    cmd_bp::execute(self, &term, command.split_ascii_whitespace().skip(1));
                }
                _ => term
                    .write_line(&style("Unknown command\n").red().to_string())
                    .unwrap(),
            }
        }

        term.clear_screen().unwrap();
    }

    fn break_reason<CMem: CartridgeMem, PpuDbg: DbgEvtSrc<PpuEvt>>(
        &mut self,
        emu: &Emulator<CMem, DbgEvtLogger<CpuEvt>, PpuDbg>,
    ) -> Option<BreakReason> {
        if let Some(steps) = &mut self.break_in {
            if *steps == 0 {
                self.break_in = None;
                return Some(BreakReason::UserRequest);
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
                return Some(BreakReason::BreakpointHit(bp));
            }
        }

        for (bp, cond) in self.mem_breakpoints.iter().copied() {
            // Can't move this outside of the loop, or it will be consumed by the first breakpoint!
            let mut latest_mem_acceses = emu
                .board
                .cpu_evt_src
                .evts()
                .rev()
                .take_while(|evt| !matches!(evt, CpuEvt::Exec(_, _)));

            if latest_mem_acceses.any(|evt| match evt {
                CpuEvt::ReadMem(addr, _) => {
                    bp == *addr && matches!(cond, BreakCond::Read | BreakCond::ReadWrite)
                }
                CpuEvt::WriteMem(addr, _) => {
                    bp == *addr && matches!(cond, BreakCond::Write | BreakCond::ReadWrite)
                }
                _ => false,
            }) {
                return Some(BreakReason::CondBreakpointHit(bp, cond));
            }
        }

        None
    }

    pub fn request_break(&mut self) {
        self.break_in(0);
    }

    fn break_in(&mut self, steps: usize) {
        self.break_in = Some(steps);
    }

    /// Returns true if the command was succesful
    fn cmd_step<'a, I: Iterator<Item = &'a str>>(&mut self, term: &Term, mut args: I) -> bool {
        match args.next() {
            Some("line") => self.break_in(114 - 4),
            Some("frame") => self.break_in(17556 - 4),
            Some(steps_str) => match steps_str.parse::<usize>() {
                Ok(steps) => self.break_in(steps),
                Err(err) => {
                    term.write_line(&format!(
                        "{} {}",
                        style("Could not parse number of steps").red(),
                        style(err).red()
                    ))
                    .unwrap();
                    return false;
                }
            },
            None => self.break_in(0),
        }

        true
    }

    fn print_break_reason(&mut self, break_reason: BreakReason) {
        match break_reason {
            BreakReason::UserRequest => (),
            BreakReason::BreakpointHit(addr) => writeln!(
                self.output_buffer,
                "{} {}\n",
                style("Hit breakpoint at").red(),
                addr.fmt_addr()
            )
            .unwrap(),
            BreakReason::CondBreakpointHit(addr, cond) => writeln!(
                self.output_buffer,
                "{} {} ({:?})\n",
                style("Memory breakpoint hit at").red(),
                addr.fmt_addr(),
                cond
            )
            .unwrap(),
        }
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

    fn print_ppu_state(&mut self, ppu: &PPU) {
        fn print_on_off(val: bool) -> StyledObject<&'static str> {
            if val {
                style("On").green()
            } else {
                style("Off").red()
            }
        }

        let lcdc = LCDC(ppu.read_reg(PpuReg::LCDC));
        let lcds = LCDS::from_raw(ppu.read_reg(PpuReg::LCDS));

        writeln!(
            self.output_buffer,
            " LCD: {} ({:?}), WND: {}, OBJ: {} ({:?}), BG: {}, LYC==LY: {}",
            print_on_off(lcdc.lcd_enabled()),
            lcds.mode(),
            print_on_off(lcdc.window_enabled()),
            print_on_off(lcdc.sprites_enabled()),
            lcdc.sprite_size(),
            print_on_off(lcdc.bg_enabled()),
            if lcds.lyc_equals_ly() {
                style("Yes").green()
            } else {
                style("No").red()
            },
        )
        .unwrap();

        writeln!(
            self.output_buffer,
            " Interrupts: LYC: {}, OAMSearch: {}, VBlank: {}, HBlank: {}",
            print_on_off(lcds.ly_coincidence_interrupt()),
            print_on_off(lcds.oam_search_interrupt()),
            print_on_off(lcds.v_blank_interrupt()),
            print_on_off(lcds.h_blank_interrupt()),
        )
        .unwrap();

        writeln!(
            self.output_buffer,
            " SCY: {}, SCX: {}, LY: {}, LYC: {}, WY: {}, WX: {}",
            ppu.read_reg(PpuReg::SCY).fmt_val(),
            ppu.read_reg(PpuReg::SCX).fmt_val(),
            ppu.read_reg(PpuReg::LY).fmt_val(),
            ppu.read_reg(PpuReg::LYC).fmt_val(),
            ppu.read_reg(PpuReg::WY).fmt_val(),
            ppu.read_reg(PpuReg::WX).fmt_val(),
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
                    writeln!(self.output_buffer, " [{}] {:?}", pc.fmt_addr(), instr).unwrap()
                }
                CpuEvt::ExecCB(instr) => {
                    writeln!(self.output_buffer, "  Executing {:?}", instr).unwrap()
                }
                CpuEvt::ReadMem(addr, val) => writeln!(
                    self.output_buffer,
                    "  Read {} from {}",
                    val.fmt_val(),
                    addr.fmt_addr(),
                )
                .unwrap(),
                CpuEvt::WriteMem(addr, val) => writeln!(
                    self.output_buffer,
                    "  Write {} to {}",
                    val.fmt_val(),
                    addr.fmt_addr(),
                )
                .unwrap(),
                CpuEvt::HandleIR(ir) => {
                    writeln!(self.output_buffer, " Jumping to {:?} interrupt handler", ir).unwrap()
                }
                CpuEvt::TakeJmpTo(addr) => writeln!(
                    self.output_buffer,
                    " {} {}",
                    style("Taking jump to:").green(),
                    addr.fmt_addr()
                )
                .unwrap(),
                CpuEvt::SkipJmpTo(addr) => writeln!(
                    self.output_buffer,
                    " {} {}",
                    style("Skipping jump to").red(),
                    addr.fmt_addr()
                )
                .unwrap(),
                CpuEvt::EnterHalt(halt_state) => writeln!(
                    self.output_buffer,
                    " {} {:?}",
                    style("Entering halt state:").red(),
                    halt_state
                )
                .unwrap(),
                CpuEvt::IrEnable => writeln!(
                    self.output_buffer,
                    " {}",
                    style("Interrupts enabled").green()
                )
                .unwrap(),
                CpuEvt::IrDisable => writeln!(
                    self.output_buffer,
                    " {}",
                    style("Interrupts Disabled").red()
                )
                .unwrap(),
            }
        }
    }

    fn print_upcoming_instr<B: Board>(&mut self, cpu: &CPU, board: &B) {
        writeln!(
            self.output_buffer,
            "{}",
            style(">>>>>>>>> You are here >>>>>>>>>").green()
        )
        .unwrap();

        let mut pc = cpu.reg.pc();
        let instr: ByteInstr =
            unsafe { std::mem::transmute(board.read8_instant(Addr::from(cpu.reg.pc()))) };

        self.print_single_instr(board, &mut pc, instr);

        if instr.is_control_flow_change() {
            return;
        }

        for _ in 0..10 {
            let instr: ByteInstr =
                unsafe { std::mem::transmute(board.read8_instant(Addr::from(pc))) };

            self.print_single_instr(board, &mut pc, instr);

            if instr.is_control_flow_change() {
                return;
            }
        }
    }

    /// Returns new PC after reading the instruction
    fn print_single_instr<B: Board>(&mut self, board: &B, pc: &mut u16, instr: ByteInstr) {
        if let Some(operand) = instr.operand_type() {
            writeln!(
                self.output_buffer,
                " [{}] {:?} {}",
                pc.fmt_addr(),
                instr,
                operand.fmt(board, *pc)
            )
            .unwrap();

            *pc = pc.wrapping_add(1 + operand.len() as u16);
        } else {
            writeln!(self.output_buffer, " [{}] {:?}", pc.fmt_addr(), instr).unwrap();

            *pc = pc.wrapping_add(1);
        }
    }
}

mod cmd_bp {
    use super::*;

    pub fn execute<'a, I: Iterator<Item = &'a str>>(
        dbg: &mut CpuDebugger,
        term: &Term,
        mut args: I,
    ) {
        let mut output = String::new();

        match args.by_ref().next() {
            Some("set") => set(dbg, &mut output, args),
            Some("mem") => mem(dbg, &mut output, args),
            Some("list") => list(dbg, &mut output),
            Some("rm") => rm(dbg, &mut output, args),
            Some("clear") => clear(dbg, &mut output),
            _ => writeln!(
                output,
                "{}",
                style("ERROR: Use either 'set', 'mem', 'rm', 'list' or 'clear'").red()
            )
            .unwrap(),
        }

        term.write_line(&output).unwrap();
    }

    fn set<'a, I: Iterator<Item = &'a str>>(
        dbg: &mut CpuDebugger,
        output: &mut String,
        mut args: I,
    ) {
        cmd_bp::exec_with_addr(args.next(), output, |addr, output: &mut String| {
            dbg.breakpoints.push(addr);
            writeln!(
                output,
                "{} {}",
                style("Added breakpoint at").green(),
                addr.fmt_addr()
            )
            .unwrap();
        });
    }

    fn mem<'a, I: Iterator<Item = &'a str>>(
        dbg: &mut CpuDebugger,
        output: &mut String,
        mut args: I,
    ) {
        fn print_bp_added_msg(addr: u16, output: &mut String) {
            writeln!(
                output,
                "{} {}",
                style("Breakpoint added at").green(),
                addr.fmt_addr()
            )
            .unwrap()
        };

        match args.by_ref().next() {
            Some("r") => cmd_bp::exec_with_addr(args.next(), output, |addr, output| {
                dbg.mem_breakpoints.push((addr, BreakCond::Read));
                print_bp_added_msg(addr, output);
            }),
            Some("w") => cmd_bp::exec_with_addr(args.next(), output, |addr, output| {
                dbg.mem_breakpoints.push((addr, BreakCond::Write));
                print_bp_added_msg(addr, output);
            }),
            Some("rw") => cmd_bp::exec_with_addr(args.next(), output, |addr, output| {
                dbg.mem_breakpoints.push((addr, BreakCond::ReadWrite));
                print_bp_added_msg(addr, output);
            }),
            _ => writeln!(output, "{}", style("Use either 'r', 'w', or 'rw'").red()).unwrap(),
        }
    }

    fn list(dbg: &CpuDebugger, output: &mut String) {
        for (idx, bp) in dbg.breakpoints.iter().copied().enumerate() {
            writeln!(output, " {:>3}. {}", idx, bp.fmt_addr()).unwrap();
        }

        for (idx, (addr, cond)) in dbg.mem_breakpoints.iter().copied().enumerate() {
            writeln!(
                output,
                " {:>3}. {} ({:?})",
                idx + dbg.breakpoints.len(),
                addr.fmt_addr(),
                cond
            )
            .unwrap();
        }
    }

    fn rm<'a, I: Iterator<Item = &'a str>>(
        dbg: &mut CpuDebugger,
        output: &mut String,
        mut args: I,
    ) {
        match args.next() {
            Some(idx) => match idx.parse::<usize>() {
                Ok(idx) => {
                    if idx < dbg.breakpoints.len() {
                        dbg.breakpoints.remove(idx);
                        writeln!(output, "{}", style("Breakpoint removed").green()).unwrap();
                    } else {
                        let idx = idx - dbg.breakpoints.len();
                        if idx < dbg.mem_breakpoints.len() {
                            dbg.mem_breakpoints.remove(idx);
                            writeln!(output, "{}", style("Breakpoint removed").green()).unwrap();
                        } else {
                            writeln!(output, "{}", style("Invalid breakpoint index").red())
                                .unwrap();
                        }
                    }
                }
                Err(err) => writeln!(
                    output,
                    "{} {}",
                    style("Could not parse index:").red(),
                    style(err).red()
                )
                .unwrap(),
            },
            None => writeln!(
                output,
                "{}",
                style("Needs parameter: Breakpoint index").red()
            )
            .unwrap(),
        }
    }

    fn clear(dbg: &mut CpuDebugger, output: &mut String) {
        dbg.breakpoints.clear();
        dbg.mem_breakpoints.clear();
        writeln!(output, "{}", style("All breakpoints cleared").green()).unwrap();
    }

    fn exec_with_addr<F: FnMut(u16, &mut String)>(
        addr_str: Option<&str>,
        output: &mut String,
        mut f: F,
    ) {
        match addr_str {
            Some(addr_str) => match parse_int::parse(addr_str) {
                Ok(addr) => f(addr, output),
                Err(err) => writeln!(
                    output,
                    "{} {}",
                    style("Could not parse address:").red(),
                    style(err).red()
                )
                .unwrap(),
            },
            None => writeln!(
                output,
                "{}",
                style("Needs argument: Breakpoint address").red()
            )
            .unwrap(),
        }
    }
}
