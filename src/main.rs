mod maboy;
// mod maboy_old;

use maboy::*;

fn main() {
    let cartridge = Cartridge::from_file("./roms/11-op a,(hl).gb");
    let cartridge_mem = CartridgeMem::from(cartridge);

    let mut emu = Emulator::new(cartridge_mem);

    loop {
        emu.emulate_step();
    }
}
