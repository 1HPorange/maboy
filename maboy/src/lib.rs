//! MaBoy is the platform-agnostic core of the MaBoy Game Boy emulator.
//! It doesn't contain any frontend code for rendering, window management
//! and input, since that stuff is usually platform-specific. If you want
//! a whole working emulator implementation, look at
//! [maboy-windows](https://github.com/1HPorange/maboy)
//!
//! The emulation loop is controlled from the outside, which makes it easy
//! to implement stuff like custom debuggers, but also means that it takes
//! a little more work to get everything running.
//!
//! Anyway, here the basic framework; Code that needs to be provided by the
//! frontend is denoted by comments. Note that this is only an implementation
//! example; The layout largely depends on your frontend design.
//!
//! ```
//! fn main() {
//!    let rom_path = "my_rom.gb";
//!
//!     let cartridge =
//!         CartridgeVariant::from_file(rom_path).expect("Could not open rom file");
//!     
//!     dispatch_emulator(rom_path, cartridge);
//! }
//!
//! fn dispatch_emulator(rom_path: &str, mut cartridge: CartridgeVariant) {
//!     match &mut cartridge {
//!         CartridgeVariant::Rom(c) => run_emu(rom_path, c),
//!         CartridgeVariant::RomRam(c) => run_emu(rom_path, c),
//!         CartridgeVariant::RomRamBanked(c) => run_emu(rom_path, c),
//!         CartridgeVariant::MBC1(c) => run_emu(rom_path, c),
//!         CartridgeVariant::MBC1Ram(c) => run_emu(rom_path, c),
//!         // And so on... you get the idea
//!         _ => unimplemented(),
//!     }
//! }
//!
//! fn run_emu<C: Cartridge + Savegame + Metadata>(rom_path: &str, mut cartridge: C) {
//!     // If you want useful debugging features, pass a debug logger (to CPU and/or PPU)
//!     // via DbgEvtLogger::new() instead of NoDbgLogger
//!     let mut emu = Emulator::with_debugger(&mut cartridge, NoDbgLogger, NoDbgLogger);
//!
//!     loop {
//!         emu.emulate_step();
//!
//!         // If the condition is true, we query the OS for window/input events. We should
//!         // avoid doing that too often since it's usually kind of expensive.
//!         let perform_os_update = match emu.query_video_frame_status() {
//!             VideoFrameStatus::NotReady => {
//!                 // Either the Game Boy is in the middle of a frame, or the LCD is off.
//!                 // In the latter case, we use the condition below to make sure that we
//!                 // don't query the OS for window events a million times a second.
//!                 last_os_update.elapsed() > Duration::from_millis(20)
//!             },
//!             VideoFrameStatus::Ready(frame_data) => {
//!                 // A frame is ready, so this is the time to render it to the screen and
//!                 // present it to the user. This is also a good place to throttle the
//!                 // emulator so it doesn't run at a gazillion FPS. It's very OS-dependent
//!                 // how you would want to do that, so I won't give any example here.
//!                 true
//!             }
//!             VideoFrameStatus::LcdTurnedOff => {
//!                 // Basically the same as the previous match arm, but you should render a
//!                 // blank screen instead of a frame
//!                 true
//!             }
//!         };
//!
//!         if perform_os_update {
//!             if !os_update(&mut emu) {
//!                 break;
//!             }
//!             last_os_update = Instant::now();
//!         }
//!     }
//! }
//!
//! // I promise this function signature will become a bit prettier some day...
//! fn os_update<CMem: Cartridge, CpuDbg: DbgEvtSrc<CpuEvt>, PpuDbg: DbgEvtSrc<PpuEvt>>(
//!     emu: &mut Emulator<CMem, CpuDbg, PpuDbg>,
//! ) -> bool {
//!     // Handle window events here. If the user closed the window or terminated the
//!     // application any other way, return false
//!
//!     let mut buttons = Buttons::new();
//!
//!     // Here, query the current input state and write it to `buttons`
//!
//!     emu.notify_buttons_state(button_states);
//!
//!     true
//! }
//!
//! ```

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
