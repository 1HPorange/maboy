use winapi::shared::winerror::*;

#[derive(Debug)]
pub struct HResultError(pub HRESULT);

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
