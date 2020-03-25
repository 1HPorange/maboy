use winapi::shared::winerror::{HRESULT, SUCCEEDED};

/// Wrapper around HRESULT for future proofing; We might
/// want to extend this error type with useful info.
#[derive(Debug)]
pub struct HResultError(pub(super) HRESULT);

pub trait IntoResult: Sized {
    fn into_result(self) -> Result<(), HResultError>;
}

impl IntoResult for HRESULT {
    fn into_result(self) -> Result<(), HResultError> {
        if SUCCEEDED(self) {
            Ok(())
        } else {
            Err(HResultError(self))
        }
    }
}
