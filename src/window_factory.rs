use super::util::EncodeWideNulTerm;
use super::window::{MsgHandler, MsgHandlerResult, Window};
use std::cell::RefCell;
use std::ffi::OsString;
use std::mem;
use std::pin::Pin;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::{HWND, RECT};
use winapi::um::{errhandlingapi::GetLastError, libloaderapi::GetModuleHandleW, winuser::*};

pub struct WindowFactory {
    active_windows: RefCell<Vec<HWND>>,
}

#[derive(Debug)]
pub enum WindowCreateError {
    CouldNotRegisterWindowClass(u32),
    CouldNotDetermineWindowSize,
    CouldNotCreateWindow(u32),
    CouldNotAttachWindowInstance(u32),
}

static WND_CLASS_CREATED: AtomicBool = AtomicBool::new(false);

impl WindowFactory {
    pub fn new() -> WindowFactory {
        WindowFactory {
            active_windows: RefCell::new(Vec::with_capacity(8)),
        }
    }

    pub fn create_window<'a>(
        &self,
        title: &str,
        width: u16,
        height: u16,
        msg_handler: MsgHandler,
    ) -> Result<Pin<Box<Window>>, WindowCreateError> {
        unsafe {
            let wnd_class_name = OsString::from("MaBoy_Game_Window").encode_wide_nul_term();
            let wnd_name = OsString::from(title).encode_wide_nul_term();

            let hinstance = GetModuleHandleW(ptr::null());

            if !WND_CLASS_CREATED.compare_and_swap(false, true, Ordering::SeqCst) {
                let mut wnd_class: WNDCLASSEXW = mem::zeroed();
                wnd_class.cbSize = mem::size_of::<WNDCLASSEXW>() as u32;
                wnd_class.lpfnWndProc = Some(wnd_proc_dispatch);
                wnd_class.hInstance = hinstance;
                wnd_class.lpszClassName = wnd_class_name.as_ptr();

                if RegisterClassExW(&wnd_class) == 0 {
                    return Err(WindowCreateError::CouldNotRegisterWindowClass(
                        GetLastError(),
                    ));
                }
            }

            let mut desired_client_area = RECT {
                left: 0,
                top: 0,
                right: width as i32,
                bottom: height as i32,
            };

            if AdjustWindowRect(&mut desired_client_area, WS_OVERLAPPEDWINDOW, 0) == 0 {
                return Err(WindowCreateError::CouldNotDetermineWindowSize);
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
                return Err(WindowCreateError::CouldNotCreateWindow(GetLastError()));
            }

            let mut window = Box::pin(Window::new(self, hwnd, msg_handler));

            // Clears any error that might have been set by something we called before.
            SetLastErrorEx(0, 0);

            if SetWindowLongPtrW(
                hwnd,
                GWLP_USERDATA,
                Pin::get_unchecked_mut(window.as_mut()) as *mut _ as isize,
            ) == 0
            {
                let last_error = GetLastError();
                if last_error != 0 {
                    // TODO: Destroy window
                    return Err(WindowCreateError::CouldNotAttachWindowInstance(
                        GetLastError(),
                    ));
                }
            }

            self.active_windows.borrow_mut().push(hwnd);

            Ok(window)
        }
    }

    pub fn dispatch_window_msgs(&self) -> bool {
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
    if let Some(window) = (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Window).as_mut() {
        if msg == WM_DESTROY {
            let mut active_windows = window.factory.active_windows.borrow_mut();
            let hwnd_index = active_windows
                .iter()
                .position(|other_hwnd| hwnd == *other_hwnd)
                .expect("Destroyed window that wasn't created via WindowFactory");

            active_windows.remove(hwnd_index);

            if active_windows.is_empty() {
                PostQuitMessage(0);
                return 0;
            } else {
                return DefWindowProcW(hwnd, msg, w_param, l_param);
            }
        }

        match window.handle_msg(msg, w_param, l_param) {
            MsgHandlerResult::RunDefaultMsgHandler => (),
            MsgHandlerResult::DoNotRunDefaultMsgHandler(result) => return result,
        }
    }

    DefWindowProcW(hwnd, msg, w_param, l_param)
}
