use super::util::EncodeWithNulTerm;
use crate::maboy::input::KeyboardKey;
use std::convert::TryFrom;
use std::ffi::OsString;
use std::marker::PhantomPinned;
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
    pub keys_down: [bool; KeyboardKey::_LEN as usize], // TODO: Get rid of this horrible thing
    _pin: PhantomPinned,
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

            let mut window = Box::pin(Window {
                hwnd,
                keys_down: [false; KeyboardKey::_LEN as usize],
                _pin: PhantomPinned,
            });

            // TODO: Remove all unneccesary winapi reference (e.g. those i just need for type re-definitions)
            SetLastErrorEx(0, 0);
            if SetWindowLongPtrW(
                hwnd,
                GWLP_USERDATA,
                Pin::get_unchecked_mut(window.as_mut()) as *mut _ as isize,
            ) == 0
            {
                // TODO: Destroy window
                let last_error = GetLastError();
                if last_error != 0 {
                    return Err(WindowError::CouldNotAttachWindowInstance(GetLastError()));
                }
            }

            Ok(window)
        }
    }

    pub fn show(&self) {
        unsafe {
            ShowWindow(self.hwnd, SW_SHOW);
        }
    }

    // TODO: This should go on the factory, not the window!
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

    pub fn is_key_down(&self, key: KeyboardKey) -> bool {
        self.keys_down[key as usize]
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

    if let Some(window) = (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Window).as_mut() {
        match msg {
            WM_KEYDOWN => {
                if let Ok(key) = KeyboardKey::try_from(w_param) {
                    window.keys_down[key as usize] = true;
                }
            }
            WM_KEYUP => {
                if let Ok(key) = KeyboardKey::try_from(w_param) {
                    window.keys_down[key as usize] = false;
                }
            }
            _ => (),
        }
    }

    DefWindowProcW(hwnd, msg, w_param, l_param)
}

#[derive(Debug)]
pub enum WindowError {
    CouldNotRegisterWindowClass(DWORD),
    CouldNotDetermineWindowSize,
    CouldNotCreateWindow(DWORD),
    CouldNotAttachWindowInstance(DWORD),
}

impl TryFrom<WPARAM> for KeyboardKey {
    type Error = ();

    fn try_from(w_param: WPARAM) -> Result<Self, Self::Error> {
        match w_param {
            0x57 => Ok(KeyboardKey::W),
            0x41 => Ok(KeyboardKey::A),
            0x53 => Ok(KeyboardKey::S),
            0x44 => Ok(KeyboardKey::D),
            0x49 => Ok(KeyboardKey::I),
            0x4F => Ok(KeyboardKey::O),
            0x4B => Ok(KeyboardKey::K),
            0x4C => Ok(KeyboardKey::L),
            _ => Err(()),
        }
    }
}
