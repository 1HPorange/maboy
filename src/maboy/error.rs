use super::memory::MemoryAccessError;

#[derive(Debug)]
pub enum Error {
    MemoryAccessError(MemoryAccessError),
}

impl From<MemoryAccessError> for Error {
    fn from(e: MemoryAccessError) -> Self {
        Error::MemoryAccessError(e)
    }
}
