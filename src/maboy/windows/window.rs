use super::util::EncodeWithNulTerm;
use std::ffi::OsString;
use std::mem;
use std::os::windows::prelude::*;
use std::pin::Pin;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use winapi::ctypes::*;
use winapi::shared::{minwindef::*, windef::*};
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::winuser::*;

// TODO: Rewrite to use WindowManager

// Note: These are the dimensions of the *client area* of the window, not the window itself
const WND_WIDTH: c_int = 256; //160 * 4;
const WND_HEIGHT: c_int = 256; //144 * 4;

static WND_CLASS_CREATED: AtomicBool = AtomicBool::new(false);

pub struct Window {
    pub(super) hwnd: HWND,
}

impl Window {
    pub fn new() -> Result<Pin<Box<Window>>, WindowError> {
        unsafe {
            let wnd_class_name = OsString::from("Peter").encode_wide_with_term();
            let wnd_name = OsString::from("MaBoy GameBoy Emulator").encode_wide_with_term();

            let hinstance = GetModuleHandleW(ptr::null());

            if !WND_CLASS_CREATED.compare_and_swap(false, true, Ordering::SeqCst) {
                let mut wnd_class: WNDCLASSEXW = mem::zeroed();
                wnd_class.cbSize = mem::size_of::<WNDCLASSEXW>() as u32;
                wnd_class.lpfnWndProc = Some(wnd_proc_dispatch);
                wnd_class.hInstance = hinstance;
                wnd_class.lpszClassName = wnd_class_name.as_ptr();

                if RegisterClassExW(&wnd_class) == 0 {
                    return Err(WindowError::CouldNotRegisterWindowClass(GetLastError()));
                }
            }

            let mut desired_client_area = RECT {
                left: 0,
                top: 0,
                right: WND_WIDTH,
                bottom: WND_HEIGHT,
            };

            if AdjustWindowRect(&mut desired_client_area, WS_OVERLAPPEDWINDOW, 0) == 0 {
                return Err(WindowError::CouldNotDetermineWindowSize);
            }

            let hwnd = CreateWindowExW(
                0,
                wnd_class_name.as_ptr(),
                wnd_name.as_ptr(),
                WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
                CW_USEDEFAULT,
                CW_USEDEFAULT,
                desired_client_area.right - desired_client_area.left,
                desired_client_area.bottom - desired_client_area.top,
                ptr::null_mut(),
                ptr::null_mut(),
                hinstance,
                ptr::null_mut(),
            );

            if hwnd.is_null() {
                return Err(WindowError::CouldNotCreateWindow(GetLastError()));
            }

            Ok(Box::pin(Window { hwnd }))
        }
    }

    pub fn show(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_SHOW);
        }
    }

    // TODO: You should go on the factory, not the window!
    pub fn handle_msgs(&self) -> bool {
        unsafe {
            let mut msg: MSG = mem::MaybeUninit::uninit().assume_init();

            while PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    return false;
                }

                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            true
        }
    }
}

unsafe extern "system" fn wnd_proc_dispatch(
    hwnd: HWND,
    msg: UINT,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if msg == WM_DESTROY {
        PostQuitMessage(0);
        return 0;
    }

    DefWindowProcW(hwnd, msg, w_param, l_param)
}

#[derive(Debug)]
pub enum WindowError {
    CouldNotRegisterWindowClass(DWORD),
    CouldNotDetermineWindowSize,
    CouldNotCreateWindow(DWORD),
}
