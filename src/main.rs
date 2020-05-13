mod maboy;
mod maboy_windows;
// mod maboy_old;

use maboy::*;
use maboy_windows::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::{Duration, Instant};

// TODO: Move this into some sort of input mapping struct
const A_BUTTON_KEY: KeyboardKey = KeyboardKey::K;
const B_BUTTON_KEY: KeyboardKey = KeyboardKey::J;
const START_BUTTON_KEY: KeyboardKey = KeyboardKey::N;
const SELECT_BUTTON_KEY: KeyboardKey = KeyboardKey::B;
const UP_BUTTON_KEY: KeyboardKey = KeyboardKey::W;
const RIGHT_BUTTON_KEY: KeyboardKey = KeyboardKey::D;
const DOWN_BUTTON_KEY: KeyboardKey = KeyboardKey::S;
const LEFT_BUTTON_KEY: KeyboardKey = KeyboardKey::A;

fn main() {
    env_logger::init();

    // Parse argument as path to ROM
    let rom_path = std::env::args()
        .skip(1)
        .next()
        .expect("Please provide the path to a GameBoy ROM (.gb) as a command-line argument.");

    // Parse Cartridge
    let cartridge = CartridgeVariant::from_file(rom_path).expect("Could not open ROM file");

    match cartridge {
        CartridgeVariant::Unbanked(c) => run_emulation(c),
        CartridgeVariant::MBC1_NoRam(c) => run_emulation(c),
    }
}

fn run_emulation<C: CartridgeMem>(cartridge: C) {
    let mut emu = Emulator::new(cartridge);

    // Initialize input system
    let window_input = Rc::new(RefCell::new(WindowInput::from_watched_keys(&[
        A_BUTTON_KEY,
        B_BUTTON_KEY,
        START_BUTTON_KEY,
        SELECT_BUTTON_KEY,
        UP_BUTTON_KEY,
        RIGHT_BUTTON_KEY,
        DOWN_BUTTON_KEY,
        LEFT_BUTTON_KEY,
    ])));

    // Initialize throttle clock
    let mut os_timing = OsTiming::new(59.7)
        .expect("Could not create OS timer. This timer is used to throttle the game.");

    // Initialize Window
    let mut window_factory = WindowFactory::new();

    let game_window = {
        let window_input = Rc::clone(&window_input);
        window_factory
            .create_window(
                "MaBoy Emulatin'",
                160 * 4,
                144 * 4,
                Box::new(move |msg, w_param, _l_param| {
                    window_input.borrow_mut().update(msg, w_param);
                    MsgHandlerResult::RunDefaultMsgHandler
                }),
            )
            .expect("Could not create game window")
    };
    game_window.show();

    // Initialize DirectX to draw into the window
    let gfx_device = GfxDevice::new().expect("Could not access graphics device");
    let mut gfx_window = gfx_device
        .create_gfx_window(&game_window, 160, 144)
        .expect("Could not attach graphics device to game window");

    // Clear first frame to black (screen off)
    {
        let mut frame = gfx_window.next_frame();
        frame.clear(&[0.0, 0.0, 0.0, 1.0]);
        frame.present(false).expect("Could not present frame");
    }

    let mut frame = gfx_window.next_frame();

    // When window messages for this thread were last polled and distributed to
    // all windows that were created on this thread.
    let mut last_window_msg_poll = Instant::now();

    os_timing.notify_frame_start().unwrap();

    loop {
        emu.emulate_step();

        match emu.query_video_frame_status() {
            VideoFrameStatus::NotReady => (),
            VideoFrameStatus::Ready(frame_data) => {
                frame.copy_from_slice(frame_data);

                os_timing.wait_frame_remaining().unwrap();
                os_timing.notify_frame_start().unwrap();

                frame.present(false).expect("Could not present frame");
                frame = gfx_window.next_frame();
            }
            VideoFrameStatus::LcdTurnedOff => {
                frame.clear(&[0.0, 0.0, 0.0, 1.0]);

                os_timing.wait_frame_remaining().unwrap();
                os_timing.notify_frame_start().unwrap();

                frame.present(false).expect("Could not present frame");
                frame = gfx_window.next_frame();
            }
        }

        // TODO: Think about the timing of this
        if last_window_msg_poll.elapsed() > Duration::from_millis(16) {
            window_factory.dispatch_window_msgs();

            let button_states =
                window_input
                    .borrow()
                    .depressed_keys()
                    .fold(Buttons::empty(), |mut acc, key| {
                        match key {
                            A_BUTTON_KEY => acc.insert(Buttons::A),
                            B_BUTTON_KEY => acc.insert(Buttons::B),
                            START_BUTTON_KEY => acc.insert(Buttons::START),
                            SELECT_BUTTON_KEY => acc.insert(Buttons::SELECT),
                            UP_BUTTON_KEY => acc.insert(Buttons::UP),
                            RIGHT_BUTTON_KEY => acc.insert(Buttons::RIGHT),
                            DOWN_BUTTON_KEY => acc.insert(Buttons::DOWN),
                            LEFT_BUTTON_KEY => acc.insert(Buttons::LEFT),
                            _ => (),
                        }
                        acc
                    });

            emu.notify_buttons_state(button_states);

            last_window_msg_poll = Instant::now();
        }
    }
}
