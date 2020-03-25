//! Useful structs and enums concerning interrupt handling on the CPU

use super::util::BitOps;

/// Provides storage for the two interrupt related registers (IF and IE)
/// as well as means to schedule and query outstanding interrupts.
pub struct InterruptSystem {
    if_reg: u8,
    ie_reg: u8,
}

/// All interrupts that can occur on the Game Boy system. The value of each
/// variant is a bitmask that can be used on IF/IE to set the corresponding
/// interrupt bit.
#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum Interrupt {
    VBlank = 1 << 0,
    LcdStat = 1 << 1,
    Timer = 1 << 2,
    Serial = 1 << 3,
    Joypad = 1 << 4,
}

/// The read-mask if the IF register
const IF_MASK: u8 = 0b_1110_0000;

impl InterruptSystem {
    pub fn new() -> InterruptSystem {
        InterruptSystem {
            if_reg: IF_MASK,
            ie_reg: 0x0,
        }
    }

    pub fn read_if(&self) -> u8 {
        self.if_reg
    }

    pub fn write_if(&mut self, val: u8) {
        self.if_reg = val | IF_MASK;
    }

    pub fn read_ie(&self) -> u8 {
        self.ie_reg
    }

    pub fn write_ie(&mut self, val: u8) {
        self.ie_reg = val;
    }

    /// If an interrupt is requested (IF) *and* enabled (IE), this function
    /// will return it. If multiple interrupts are scheduled, the one with
    /// the highest priority is returned.
    pub fn query_interrupt_request(&self) -> Option<Interrupt> {
        let request = self.if_reg & self.ie_reg & 0x1F;

        if request == 0 {
            return None;
        }

        unsafe {
            for bit in 0..5 {
                if request.bit(bit) {
                    return Some(std::mem::transmute(1u8 << bit));
                }
            }

            std::hint::unreachable_unchecked()
        }
    }

    /// Sets the bit in IF the corresponds to the given interrupt
    pub fn schedule_interrupt(&mut self, interrupt: Interrupt) {
        self.if_reg |= interrupt as u8
    }
}
