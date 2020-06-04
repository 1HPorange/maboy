use maboy::debug::*;
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
const DEBUG_KEY: KeyboardKey = KeyboardKey::G;

fn main() {
    env_logger::init();

    let mut command_line_args = std::env::args();

    // Parse argument as path to ROM
    let rom_path = command_line_args
        .by_ref()
        .skip(1)
        .next()
        .expect("Please provide the path to a GameBoy ROM (.gb) as a command-line argument.");

    // Parse Cartridge
    let cartridge = CartridgeVariant::from_file(rom_path).expect("Could not open ROM file");

    match cartridge {
        CartridgeVariant::RomOnly(c) => run_emulation(c),
        CartridgeVariant::MBC1NoRam(c) => run_emulation(c),
        CartridgeVariant::MBC1UnbankedRamNoBat(c) => run_emulation(c),
        CartridgeVariant::MBC1UnbankedRamBat(c) => run_with_savegame(c, command_line_args.next())
            .expect("Failed to load or store savegame"),
    };
}

fn run_emulation<C: CartridgeMem>(cartridge: C) {
    let mut emu = Emulator::new(cartridge, cpu_logger(), NoDbgLogger);

    #[cfg(debug_assertions)]
    let mut cpu_debugger = CpuDebugger::new();

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
        DEBUG_KEY,
    ])));

    let gamepad_input = GamePadInput::find_gamepad();

    // Initialize Window
    let window_factory = WindowFactory::new();

    let game_window = {
        let window_input = Rc::clone(&window_input);
        window_factory
            .create_window(
                "MaBoy Emulatin'",
                160 * 5,
                144 * 5,
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

    let mut last_os_update = Instant::now();

    // Initialize throttle clock
    let mut os_timing = OsTiming::new(59.7)
        .expect("Could not create OS timer. This timer is used to throttle the game.");

    loop {
        #[cfg(debug_assertions)]
        cpu_debugger.try_run_blocking(&emu);

        emu.emulate_step();

        let perform_os_update = match emu.query_video_frame_status() {
            VideoFrameStatus::NotReady => last_os_update.elapsed() > Duration::from_millis(20),
            VideoFrameStatus::Ready(frame_data) => {
                frame.copy_from_slice(frame_data);
                present_frame(frame, &mut os_timing);
                frame = gfx_window.next_frame();

                true
            }
            VideoFrameStatus::LcdTurnedOff => {
                frame.clear(&[0.0, 0.0, 0.0, 1.0]);
                present_frame(frame, &mut os_timing);
                frame = gfx_window.next_frame();

                true
            }
        };

        if perform_os_update {
            if !os_update(&mut emu, &window_factory, &window_input, &gamepad_input) {
                break;
            }
            last_os_update = Instant::now();

            #[cfg(debug_assertions)]
            {
                if window_input.borrow().is_pressed(DEBUG_KEY) {
                    cpu_debugger.request_break();
                }
            }
        }
    }
}

fn run_with_savegame<C: CartridgeMem>(
    mut cartridge: C,
    savegame_path: Option<String>,
) -> Result<(), std::io::Error> {
    use std::fs::File;
    use std::io::Read;

    // Load savegame if path was provided
    if let Some(path) = &savegame_path {
        if let Ok(mut save_file) = File::open(path) {
            save_file.read_exact(cartridge.cram_mut())?;
        }
    }

    run_emulation(&mut cartridge);

    // Store savegame if path was provided
    if let Some(path) = &savegame_path {
        std::fs::write(path, cartridge.cram())?;
    }

    Ok(())
}

fn present_frame(frame: GfxFrame, os_timing: &mut OsTiming) {
    os_timing.wait_frame_remaining().unwrap();
    os_timing.notify_frame_start().unwrap();

    frame.present(false).expect("Could not present frame");
}

// TODO: Make this signature nice by lower trait requirements for Emulator function calls
fn os_update<CMem: CartridgeMem, CpuDbg: DbgEvtSrc<CpuEvt>, PpuDbg: DbgEvtSrc<PpuEvt>>(
    emu: &mut Emulator<CMem, CpuDbg, PpuDbg>,
    window_factory: &WindowFactory,
    window_input: &RefCell<WindowInput>,
    gamepad_input: &Option<GamePadInput>,
) -> bool {
    if !window_factory.dispatch_window_msgs() {
        return false;
    }

    let mut button_states =
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

    button_states |= gamepad_input
        .as_ref()
        .map(|gi| gi.button_state())
        .unwrap_or(Buttons::empty());

    emu.notify_buttons_state(button_states);

    true
}

#[cfg(debug_assertions)]
fn cpu_logger() -> DbgEvtLogger<CpuEvt> {
    DbgEvtLogger::new()
}

#[cfg(not(debug_assertions))]
fn cpu_logger() -> NoDbgLogger {
    NoDbgLogger
}
