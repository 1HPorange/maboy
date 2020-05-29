mod cpu_debugger;
mod dbg_instr;

use super::cartridge::CartridgeMem;
use super::{board::Board, cpu::CPU};
use std::borrow::BorrowMut;

pub use cpu_debugger::{BreakPoint, CpuDebugger};

// This should probably be forced to store DBG events, which other components can just throw
pub trait Debugger {
    fn update<C: CartridgeMem>(&mut self, cpu: &mut CPU, board: &mut Board<C>);
    fn schedule_break(&mut self);
}

pub struct NoDebugger;

impl Debugger for NoDebugger {
    fn update<C: CartridgeMem>(&mut self, cpu: &mut CPU, board: &mut Board<C>) {}
    fn schedule_break(&mut self) {}
}
