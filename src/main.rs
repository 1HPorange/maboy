mod maboy;
// mod maboy_old;

use maboy::*;

fn main() {
    let cartridge = Cartridge::from_file("./roms/cpu_instrs.gb");
    let mut cartridge_mem = CartridgeMem::from(cartridge);
    let mut internal_mem = InternalMem::new();

    let mut emu = Emulator::new(&mut internal_mem, &mut cartridge_mem);

    loop {
        emu.emulate_step();
    }
}
