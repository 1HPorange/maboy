use super::window_factory::WindowFactory;
use std::marker::{PhantomData, PhantomPinned};
use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::HWND;
use winapi::um::winuser::{ShowWindow, SW_SHOW};

// TODO: Impl drop closing the window properly
pub struct Window {
    hwnd: HWND,
    msg_handler: MsgHandler,
    _pin: PhantomPinned,
}

pub type MsgHandler = fn(UINT, WPARAM, LPARAM) -> MsgHandlerResult;

pub enum MsgHandlerResult {
    RunDefaultMsgHandler,
    DoNotRunDefaultMsgHandler(LRESULT),
}

impl Window {
    pub(super) fn new(factory: &WindowFactory, hwnd: HWND, msg_handler: MsgHandler) -> Window {
        Window {
            hwnd,
            msg_handler,
            _pin: PhantomPinned,
        }
    }

    pub(super) fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub(super) fn handle_msg(
        &mut self,
        msg: UINT,
        w_param: WPARAM,
        l_param: LPARAM,
    ) -> MsgHandlerResult {
        (self.msg_handler)(msg, w_param, l_param)
    }

    pub fn show(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_SHOW);
        }
    }
}