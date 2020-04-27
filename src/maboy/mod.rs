mod board;
mod cartridge;
mod cpu;
mod memory;
mod util;

use board::Board;
use cpu::CPU;
use memory::cartridge_mem::CartridgeRam;
use memory::Memory;

pub use cartridge::Cartridge;
pub use memory::{cartridge_mem::CartridgeMem, internal_mem::InternalMem};

pub struct Emulator<'m, CRAM: CartridgeRam> {
    cpu: CPU,
    board: Board<'m, CRAM>,
}

impl<'m, CRAM: CartridgeRam> Emulator<'m, CRAM> {
    pub fn new(
        internal_mem: &'m mut InternalMem,
        cartridge_mem: &'m mut CartridgeMem<CRAM>,
    ) -> Emulator<'m, CRAM> {
        let mem = Memory::new(internal_mem, cartridge_mem);

        Emulator {
            cpu: CPU::new(),
            board: Board::new(mem),
        }
    }

    pub fn emulate_step(&mut self) {
        self.cpu.step_instr(&mut self.board);
    }
}
