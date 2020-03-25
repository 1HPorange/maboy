//! This crate contains the Windows (DirectX 11) frontend for the 
//! [Maboy Gameboy Emulator](https://github.com/1HPorange/maboy).
//! It handles window management, input and graphics for the emulator backend.

mod expect_msg_box;
mod gamepad_input;
mod gfx;
mod hresult_error;
mod open_file_dialog;
mod os_timing;
mod util;
mod window;
mod window_factory;
mod window_input;

pub use expect_msg_box::ExpectMsgBox;
pub use gamepad_input::GamePadInput;
pub use gfx::{GfxDevice, GfxFrame, GfxWindow};
pub use open_file_dialog::{open_file_dialog, FileFilter};
pub use os_timing::OsTiming;
pub use window::{MsgHandler, MsgHandlerResult, Window};
pub use window_factory::WindowFactory;
pub use window_input::{KeyboardKey, WindowInput};
