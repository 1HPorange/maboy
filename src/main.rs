mod maboy;
mod maboy_windows;
// mod maboy_old;

use maboy::*;

fn main() {
    let cartridge = Cartridge::from_file("./roms/01-special.gb");
    let cartridge_mem = CartridgeMem::from(cartridge);

    let mut emu = Emulator::new(cartridge_mem);

    loop {
        emu.emulate_step();
        if let Some(frame_data) = emu.query_video_frame_ready() {
            println!("Frame ready: {} pixels", frame_data.len());
        }
    }
}
