//! Utilities for displaying a native open file dialog

use super::util::EncodeWideNulTerm;
use std::ffi::OsString;
use std::iter;
use std::os::windows::ffi::OsStringExt;
use std::ptr;
use winapi::shared::minwindef::TRUE;
use winapi::um::commdlg::{GetOpenFileNameW, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR, OPENFILENAMEW};

/// Filter for the files displayed in the dialog.
///
/// # Example
/// ```
/// let filter = FileFilter {
///     display_name: "Text file",
///     file_types: vec!["*.TXT", "*.DOC"],
/// };
/// ```
pub struct FileFilter {
    /// Filter name, displayed in the file-type dropdown
    pub display_name: &'static str,
    /// Accepted file names and extensions in upper-case letters with wildcards allowed.
    /// An example would be `*.JPG`
    pub file_types: Vec<&'static str>,
}

impl FileFilter {
    pub fn new(display_name: &'static str, file_types: Vec<&'static str>) -> FileFilter {
        FileFilter {
            display_name,
            file_types,
        }
    }
}

// TODO: Think about returning a `Result` from here instead of an `Option`
/// Displays a native open file dialog with the specified window title and file filter.
pub fn open_file_dialog(title: &str, filters: Vec<FileFilter>) -> Option<OsString> {
    const MAX_FILE_NAME_LEN: usize = 300;

    let mut title = OsString::from(title).encode_wide_nul_term();

    let mut filter_buf = filters
        .into_iter()
        .flat_map(|f| vec![f.display_name.to_owned(), f.file_types.join(";")])
        .flat_map(|s| OsString::from(s).encode_wide_nul_term())
        .chain(iter::once(0))
        .collect::<Vec<u16>>();

    let mut file_name_buffer = vec![0u16; MAX_FILE_NAME_LEN];

    let mut open_dialog_options = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: ptr::null_mut(),
        hInstance: ptr::null_mut(),
        lpstrFilter: filter_buf.as_mut_ptr(),
        lpstrCustomFilter: ptr::null_mut(),
        nMaxCustFilter: 0,
        nFilterIndex: 1,
        lpstrFile: file_name_buffer.as_mut_ptr(),
        nMaxFile: MAX_FILE_NAME_LEN as u32,
        lpstrFileTitle: ptr::null_mut(),
        nMaxFileTitle: 0,
        lpstrInitialDir: ptr::null_mut(),
        lpstrTitle: title.as_mut_ptr(),
        Flags: OFN_FILEMUSTEXIST | OFN_NOCHANGEDIR,
        nFileOffset: 0,
        nFileExtension: 0,
        lpstrDefExt: ptr::null_mut(),
        lCustData: 0,
        lpfnHook: None,
        lpTemplateName: ptr::null_mut(),
        pvReserved: ptr::null_mut(),
        dwReserved: 0,
        FlagsEx: 0,
    };

    if TRUE == unsafe { GetOpenFileNameW(&mut open_dialog_options) } {
        file_name_buffer.retain(|c| *c != 0);
        Some(OsString::from_wide(&file_name_buffer))
    } else {
        None
    }
}
