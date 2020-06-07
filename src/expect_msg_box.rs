use super::util::EncodeWideNulTerm;
use std::fmt::Debug;
use std::{ffi::OsString, ptr, sync::Once};
use winapi::um::winuser::MessageBoxW;
use winapi::um::winuser::MB_OK;

static mut MSG_BOX_TITLE: Vec<u16> = Vec::new();
static MSG_BOX_TITLE_INIT: Once = Once::new();

pub trait ExpectMsgBox<T> {
    fn expect_msg_box(self, msg: &str) -> T;
}

impl<T, E: Debug> ExpectMsgBox<T> for Result<T, E> {
    fn expect_msg_box(self, msg: &str) -> T {
        let title = unsafe {
            MSG_BOX_TITLE_INIT.call_once(|| {
                MSG_BOX_TITLE = OsString::from("MaBoy GameBoy Emulator").encode_wide_nul_term()
            });
            &MSG_BOX_TITLE
        };

        match self {
            Ok(val) => val,
            Err(err) => unsafe {
                let err_str =
                    OsString::from(&format!("{} ({:?})", msg, err)).encode_wide_nul_term();
                MessageBoxW(ptr::null_mut(), err_str.as_ptr(), title.as_ptr(), MB_OK);
                panic!("{:?}", err);
            },
        }
    }
}

impl<T> ExpectMsgBox<T> for Option<T> {
    fn expect_msg_box(self, msg: &str) -> T {
        let title = unsafe {
            MSG_BOX_TITLE_INIT.call_once(|| {
                MSG_BOX_TITLE = OsString::from("MaBoy GameBoy Emulator").encode_wide_nul_term()
            });
            &MSG_BOX_TITLE
        };

        match self {
            Some(val) => val,
            None => unsafe {
                let err_str = OsString::from(msg).encode_wide_nul_term();
                MessageBoxW(ptr::null_mut(), err_str.as_ptr(), title.as_ptr(), MB_OK);
                panic!("Unwrapped empty option");
            },
        }
    }
}
