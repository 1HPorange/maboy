use super::util::EncodeWithNulTerm;
use super::window::{MsgHandler, MsgHandlerResult, Window};
use std::ffi::OsString;
use std::mem;
use std::pin::Pin;
use std::ptr;
use std::sync::atomic::{AtomicBool, Ordering};
use winapi::shared::minwindef::{LPARAM, LRESULT, UINT, WPARAM};
use winapi::shared::windef::{HWND, RECT};
use winapi::um::{errhandlingapi::GetLastError, libloaderapi::GetModuleHandleW, winuser::*};

pub struct WindowFactory(());

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
        WindowFactory(())
    }

    pub fn create_window<'a>(
        &mut self,
        title: &str,
        width: u16,
        height: u16,
        msg_handler: MsgHandler,
    ) -> Result<Pin<Box<Window>>, WindowCreateError> {
        unsafe {
            let wnd_class_name = OsString::from("MaBoy_Game_Window").encode_wide_with_term();
            let wnd_name = OsString::from("MaBoy Emulatin'").encode_wide_with_term();

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
                    return Err(WindowCreateError::CouldNotAttachWindowInstance(
                        GetLastError(),
                    ));
                }
            }

            Ok(window)
        }
    }

    pub fn dispatch_window_msgs(&self) {
        unsafe {
            let mut msg: MSG = mem::MaybeUninit::uninit().assume_init();

            while PeekMessageW(&mut msg, ptr::null_mut(), 0, 0, PM_REMOVE) != 0 {
                if msg.message == WM_QUIT {
                    // return false;
                    // TODO: Some kind of mechanism to tell the window factory that no window is alive
                }

                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
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
        panic!("TODO: Handle window close nicely"); // TODO: Some kind of mechanism to tell the window factory that no window is alive
        return 0;
    }

    if let Some(window) = (GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut Window).as_mut() {
        match window.handle_msg(msg, w_param, l_param) {
            MsgHandlerResult::RunDefaultMsgHandler => (),
            MsgHandlerResult::DoNotRunDefaultMsgHandler(result) => return result,
        }
    }

    DefWindowProcW(hwnd, msg, w_param, l_param)
}
