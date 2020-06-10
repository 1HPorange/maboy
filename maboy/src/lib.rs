// TODO: Become lib

mod address;
mod board;
mod cartridge;
mod cpu;
pub mod debug;
mod interrupt_system;
mod joypad;
mod memory;
mod ppu;
mod serial_port;
mod timer;
mod util;

use board::BoardImpl;
use cpu::CPU;
use debug::*;
use memory::{InternalMem, Memory};

pub use cartridge::*;

pub use joypad::Buttons;
pub use ppu::{MemPixel, VideoFrameStatus};

pub struct Emulator<C, CpuDbg, PpuDbg> {
    cpu: CPU,
    board: BoardImpl<C, CpuDbg, PpuDbg>,
}

impl<C: Cartridge> Emulator<C, NoDbgLogger, NoDbgLogger> {
    pub fn new(cartridge: C) -> Self {
        Self::with_debugger(cartridge, NoDbgLogger, NoDbgLogger)
    }
}

impl<C: Cartridge, CpuDbg: DbgEvtSrc<CpuEvt>, PpuDbg: DbgEvtSrc<PpuEvt>>
    Emulator<C, CpuDbg, PpuDbg>
{
    pub fn with_debugger(cartridge: C, cpu_logger: CpuDbg, ppu_logger: PpuDbg) -> Self {
        let mem = Memory::new(InternalMem::new(), cartridge);

        Self {
            cpu: CPU::new(),
            board: BoardImpl::new(mem, cpu_logger, ppu_logger),
        }
    }

    pub fn emulate_step(&mut self) {
        self.cpu.step_instr(&mut self.board);
    }

    pub fn query_video_frame_status(&mut self) -> VideoFrameStatus {
        self.board.query_video_frame_status()
    }

    /// Call this if your frontend encounters a KEY_DOWN event (or sth equivalent).
    /// `Buttons::A | Buttons::B` means A and B were both pressed, with no info
    /// available about the other buttons, which will remain unchanged.
    pub fn notify_buttons_pressed(&mut self, buttons: Buttons) {
        self.board.notify_buttons_pressed(buttons);
    }

    /// Call this if your frontend encounters a KEY_UP event (or sth equivalent).
    /// `Buttons::A | Buttons::B` means A and B were both released, with no info
    /// available about the other buttons, which will remain unchanged.
    pub fn notify_buttons_released(&mut self, buttons: Buttons) {
        self.board.notify_buttons_released(buttons);
    }

    /// Alternative API if your frontend isn't suited for or doesn't provide 'KEY_UP'
    //and 'KEY_DOWN' events. `Buttons::A | Buttons::B` means A and B are pressed,
    /// and the rest of the buttons are not pressed.
    pub fn notify_buttons_state(&mut self, buttons: Buttons) {
        self.board.notify_buttons_state(buttons);
    }
}
