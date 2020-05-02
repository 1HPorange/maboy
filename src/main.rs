mod maboy;
mod maboy_windows;
// mod maboy_old;

use maboy::*;
use maboy_windows::*;
use std::time::{Duration, Instant};

fn main() {
    // Initialize Emulator
    let cartridge = Cartridge::from_file("./roms/02-interrupts.gb");
    let cartridge_mem = CartridgeMem::from(cartridge);

    let mut emu = Emulator::new(cartridge_mem);

    // Initialize Window
    let mut window_factory = WindowFactory::new();
    let game_window = window_factory
        .create_window("MaBoy Emulatin'", 160, 144, |msg, w_param, l_param| {
            MsgHandlerResult::RunDefaultMsgHandler
        })
        .expect("Could not create game window");
    game_window.show();

    // Initialize DirectX to draw into the window
    let gfx_device = GfxDevice::new().expect("Could not access graphics device");
    let mut gfx_window = gfx_device
        .create_gfx_window(&game_window)
        .expect("Could not attach graphics device to game window");

    // Clear first frame to black (screen off)
    {
        let mut frame = gfx_window.next_frame();
        frame.clear(&[0.0, 0.0, 0.0, 1.0]);
        frame.present(false).expect("Could not present frame");
    }

    let mut frame = gfx_window.next_frame();

    // Set an interval for polling window messages
    let mut last_window_msg_poll = Instant::now();

    loop {
        emu.emulate_step();

        match emu.query_video_frame_status() {
            VideoFrameStatus::NotReady => (),
            VideoFrameStatus::Ready(frame_data) => {
                frame.copy_from_slice(frame_data);
                frame.present(false).expect("Could not present frame");
                frame = gfx_window.next_frame();
            }
            VideoFrameStatus::LcdTurnedOff => {
                frame.clear(&[0.0, 0.0, 0.0, 1.0]);
                frame = gfx_window.next_frame();
            }
        }

        if last_window_msg_poll.elapsed() > Duration::from_millis(16) {
            window_factory.dispatch_window_msgs();
            last_window_msg_poll = Instant::now();
        }
    }
}
