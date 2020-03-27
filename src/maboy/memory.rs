use super::error::Error;
use std::ops::{Index, IndexMut};

#[derive(Debug)]
pub enum MemoryAccessError {}

pub struct Memory;

impl Memory {
    pub fn TEMP_NEW() -> Memory {
        Memory
    }

    pub fn get8(&self, addr: u16) -> Result<u8, MemoryAccessError> {
        unimplemented!()
    }

    pub fn get8_mut(&self, addr: u16) -> Result<&mut u8, MemoryAccessError> {
        unimplemented!()
    }

    pub fn get16(&self, addr: u16) -> Result<u16, MemoryAccessError> {
        unimplemented!()
    }

    pub fn get16_mut(&self, addr: u16) -> Result<&mut u16, MemoryAccessError> {
        unimplemented!()
    }
}

// impl Index<u16> for RAM {
//     type Output = u8;

//     fn index(&self, index: u16) -> &Self::Output {
//         unimplemented!()
//     }
// }

// impl IndexMut<u16> for RAM {
//     fn index_mut(&mut self, index: u16) -> &mut Self::Output {
//         unimplemented!()
//     }
// }
