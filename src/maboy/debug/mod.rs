mod cpu_debugger;
mod dbg_instr;
mod fmt;

use super::cpu::{ByteInstr, CBByteInstr, HaltState};
use super::interrupt_system::Interrupt;
use std::collections::VecDeque;

pub const MAX_EVTS_LOGGED: usize = 50;

pub use cpu_debugger::BreakPoint;

pub trait DbgEvtSrc<T> {
    fn push(&mut self, evt: T);
    fn pop(&mut self) -> Option<T>;
}

pub enum CpuEvt {
    Exec(u16, ByteInstr),
    ExecCB(CBByteInstr),
    ReadMem(u16, u8),
    WriteMem(u16, u8),
    HandleIR(Interrupt),
    TakeJmpTo(u16),
    SkipJmpTo(u16),
    EnterHalt(HaltState),
}

pub enum PpuEvt {}

pub struct NoDbgLogger;

impl<T> DbgEvtSrc<T> for NoDbgLogger {
    fn push(&mut self, evt: T) {}
    fn pop(&mut self) -> Option<T> {
        None
    }
}

pub struct DbgEvtLogger<T>(VecDeque<T>);

impl<T> DbgEvtLogger<T> {
    pub fn new() -> Self {
        Self(VecDeque::with_capacity(MAX_EVTS_LOGGED))
    }
}

impl<T> DbgEvtSrc<T> for DbgEvtLogger<T> {
    fn push(&mut self, evt: T) {
        if self.0.len() == MAX_EVTS_LOGGED {
            self.0.pop_front();
        }
        self.0.push_back(evt)
    }

    fn pop(&mut self) -> Option<T> {
        self.0.pop_front()
    }
}
