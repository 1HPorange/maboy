mod address;
mod board;
mod cartridge;
mod cpu;
mod interrupt_system;
mod joypad;
mod memory;
mod ppu;
mod serial_port;
mod timer;
mod util;

use board::Board;
use cpu::CPU;
use memory::{cartridge_mem::CartridgeRam, internal_mem::InternalMem, Memory};

pub use cartridge::Cartridge;
pub use joypad::Buttons;
pub use memory::cartridge_mem::CartridgeMem;
pub use ppu::mem_frame::MemPixel;
pub use ppu::VideoFrameStatus;

pub struct Emulator<CRAM: CartridgeRam> {
    cpu: CPU,
    board: Board<CRAM>,
}

impl<CRAM: CartridgeRam> Emulator<CRAM> {
    pub fn new(cartridge_mem: CartridgeMem<CRAM>) -> Emulator<CRAM> {
        let mem = Memory::new(InternalMem::new(), cartridge_mem);

        Emulator {
            cpu: CPU::new(),
            board: Board::new(mem),
        }
    }

    pub fn emulate_step(&mut self) {
        self.cpu.step_instr(&mut self.board);
    }

    pub fn query_video_frame_status(&self) -> VideoFrameStatus {
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
