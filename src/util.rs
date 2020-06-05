use std::ffi::*;
use std::iter::*;
use std::os::windows::prelude::*;

pub trait EncodeWideNulTerm: OsStrExt {
    fn encode_wide_nul_term(&self) -> Vec<u16> {
        self.encode_wide().chain(once(0)).collect()
    }
}

// Why does this not work???
// impl<T: OsStrExt> EncodeWithNulTerm for T {}

// ... but this does...
impl EncodeWideNulTerm for OsStr {}
