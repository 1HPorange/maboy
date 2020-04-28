mod board;
mod cartridge;
mod cpu;
mod memory;
mod ppu;
mod util;

use board::Board;
use cpu::CPU;
use memory::{cartridge_mem::CartridgeRam, internal_mem::InternalMem, Memory};

pub use cartridge::Cartridge;
pub use memory::cartridge_mem::CartridgeMem;

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
}
