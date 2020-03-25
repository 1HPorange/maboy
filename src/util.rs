use std::ffi::*;
use std::iter::*;
use std::os::windows::prelude::*;

/// Very often, we need to encode strings as weird pseudo UTF-16 with a null terminator
/// for Win32 interop. This trait provides an easy extension method for this purpose.
pub trait EncodeWideNulTerm: OsStrExt {
    fn encode_wide_nul_term(&self) -> Vec<u16> {
        self.encode_wide().chain(once(0)).collect()
    }
}

// TODO: Why does this not work???
// impl<T: OsStrExt> EncodeWithNulTerm for T {}

// ... but this does...
impl EncodeWideNulTerm for OsStr {}
