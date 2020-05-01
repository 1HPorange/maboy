use super::util::EncodeWithNulTerm;
use super::window::Window;
use std::ffi::OsString;
use std::pin::Pin;
use winapi::um::libloaderapi::GetModuleHandleW;

pub struct WindowFactory(());

impl WindowFactory {
    pub fn new() -> WindowFactory {
        WindowFactory(())
    }

    pub fn create_window() -> Result<Pin<Box<Window>>, CreateWindowError> {
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
}

pub enum CreateWindowError {
    CouldNotRegisterWindowClass(DWORD),
    CouldNotDetermineWindowSize,
    CouldNotCreateWindow(DWORD),
    CouldNotAttachWindowInstance(DWORD),
}
