use super::window_factory::WindowFactory;
use std::cell::RefCell;
use std::marker::PhantomPinned;
use std::rc::Rc;
use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::HWND;
use winapi::um::winuser::{ShowWindow, SW_SHOW};

// TODO: Impl drop closing the window properly
pub struct Window<'f> {
    hwnd: HWND,
    pub(super) factory: &'f WindowFactory,
    msg_handler: MsgHandler,
    _pin: PhantomPinned,
}

/// (msg, w_param, l_param)
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
