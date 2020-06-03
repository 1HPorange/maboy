mod cpu_debugger;
mod dbg_instr;
mod fmt;

use super::cpu::{ByteInstr, CBByteInstr, HaltState};
use super::{address::Addr, interrupt_system::Interrupt};
use std::collections::VecDeque;
use std::iter::Iterator;

pub use cpu_debugger::CpuDebugger;

pub const MAX_EVTS_LOGGED: usize = 50;

pub trait DbgEvtSrc<T> {
    fn push(&mut self, evt: T);
}

#[derive(Debug, Copy, Clone)]
pub enum CpuEvt {
    Exec(u16, ByteInstr),
    ExecCB(CBByteInstr),
    ReadMem(u16, u8),
    WriteMem(u16, u8),
    HandleIR(Interrupt),
    TakeJmpTo(u16),
    SkipJmpTo(u16),
    EnterHalt(HaltState),
    IrEnable,
    IrDisable,
}

pub enum PpuEvt {}

pub struct NoDbgLogger;

impl<T> DbgEvtSrc<T> for NoDbgLogger {
    fn push(&mut self, evt: T) {}
}

pub struct DbgEvtLogger<T>(VecDeque<T>);

impl<T> DbgEvtLogger<T> {
    pub fn new() -> Self {
        Self(VecDeque::with_capacity(MAX_EVTS_LOGGED))
    }

    pub fn evts(&self) -> impl DoubleEndedIterator<Item = &T> {
        self.0.iter()
    }
}

impl<T> DbgEvtSrc<T> for DbgEvtLogger<T> {
    fn push(&mut self, evt: T) {
        if self.0.len() == MAX_EVTS_LOGGED {
            self.0.pop_front();
        }
        self.0.push_back(evt)
    }
}
