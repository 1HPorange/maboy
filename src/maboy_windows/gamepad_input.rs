use crate::maboy::Buttons;
use bitflags::bitflags;
use std::mem::MaybeUninit;
use winapi::shared::minwindef::DWORD;
use winapi::shared::winerror::ERROR_SUCCESS;
use winapi::um::xinput::{XInputGetState, XINPUT_STATE};

/// Supports only a single controller (since it's a fricking GameBoy... What more do you want?)
pub struct GamePadInput(DWORD);

impl GamePadInput {
    pub fn find_gamepad() -> Option<GamePadInput> {
        unsafe {
            let mut input_state: XINPUT_STATE = MaybeUninit::uninit().assume_init();

            for user_index in 0..4 {
                if ERROR_SUCCESS == XInputGetState(user_index, &mut input_state) {
                    return Some(GamePadInput(user_index));
                }
            }

            None
        }
    }

    pub fn button_state(&self) -> Buttons {
        let gamepad_buttons = unsafe {
            let mut input_state: XINPUT_STATE = MaybeUninit::uninit().assume_init();
            XInputGetState(self.0, &mut input_state);
            GamepadButtons::from_bits_unchecked(input_state.Gamepad.wButtons)
        };

        let mut emu_buttons = Buttons::empty();

        emu_buttons.set(
            Buttons::LEFT,
            gamepad_buttons.contains(GamepadButtons::DPAD_LEFT),
        );

        emu_buttons.set(
            Buttons::UP,
            gamepad_buttons.contains(GamepadButtons::DPAD_UP),
        );

        emu_buttons.set(
            Buttons::RIGHT,
            gamepad_buttons.contains(GamepadButtons::DPAD_RIGHT),
        );

        emu_buttons.set(
            Buttons::DOWN,
            gamepad_buttons.contains(GamepadButtons::DPAD_DOWN),
        );

        emu_buttons.set(Buttons::A, gamepad_buttons.contains(GamepadButtons::B));

        emu_buttons.set(Buttons::B, gamepad_buttons.contains(GamepadButtons::A));

        emu_buttons.set(
            Buttons::START,
            gamepad_buttons.contains(GamepadButtons::START),
        );

        emu_buttons.set(
            Buttons::SELECT,
            gamepad_buttons.contains(GamepadButtons::BACK),
        );

        emu_buttons
    }
}

bitflags! {
    struct GamepadButtons: u16 {
        const DPAD_UP = 0x0001;
        const DPAD_DOWN = 0x0002;
        const DPAD_LEFT = 0x0004;
        const DPAD_RIGHT = 0x0008;
        const START = 0x0010;
        const BACK = 0x0020;
        const LEFT_THUMB = 0x0040;
        const RIGHT_THUMB = 0x0080;
        const LEFT_SHOULDER = 0x0100;
        const RIGHT_SHOULDER = 0x0200;
        const A = 0x1000;
        const B = 0x2000;
        const X = 0x4000;
        const Y = 0x8000;
    }
}
