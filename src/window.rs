//! Utilities for creating native windows on Win32. For now, there is
//! no support for any UI besides the window frame. All drawing for
//! the emulator is done through DirectX.

use super::window_factory::WindowFactory;
use std::marker::PhantomPinned;
use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::HWND;
use winapi::um::winuser::{ShowWindow, SW_SHOW};

// TODO: Impl drop closing the window properly
/// A native window with its own message handler routine. Don't forget
/// to display the window after creating it by calling [`Window::show`].
pub struct Window<'f> {
    hwnd: HWND,
    pub(super) factory: &'f WindowFactory,
    msg_handler: MsgHandler,
    /// We pin this struct since the window factory has to dereference
    /// a raw pointer into it, meaning that it *must not* be moved.
    _pin: PhantomPinned,
}

/// A Win32 message handler routine. Parameters are (msg, w_param, l_param).
pub type MsgHandler = Box<dyn Fn(u32, usize, isize) -> MsgHandlerResult>;

pub enum MsgHandlerResult {
    RunDefaultMsgHandler,
    DoNotRunDefaultMsgHandler(LRESULT),
}

impl<'f> Window<'f> {
    pub(super) fn new(factory: &WindowFactory, hwnd: HWND, msg_handler: MsgHandler) -> Window {
        Window {
            hwnd,
            factory,
            msg_handler,
            _pin: PhantomPinned,
        }
    }

    pub(super) fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// Calls the stored internal message handler routine
    pub(super) fn handle_msg(
        &mut self,
        msg: UINT,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> MsgHandlerResult {
        (self.msg_handler)(msg, w_param, l_param)
    }

    /// Actually displays the window
    pub fn show(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_SHOW);
        }
    }
}
