use super::address::TimerReg;
use super::interrupt_system::{Interrupt, InterruptSystem};
use super::util::BitOps;

/// The timer is a really screwed up thing with lots of oddities.
/// This implementation should be close enough without introducing
/// unneccessary complexity.
pub struct Timer {
    div_reg: u16,
    tima_reg: u8,
    tma_reg: u8,
    tac_reg: u8,
    tima_freq: TimaFrequency,
    /// 0 when off, 0xffff when on
    tima_enabled: u16,
}

const TAC_WRITE_MASK: u8 = 0b111;

/// Enum values are the bitmask for DIV that triggers an increase in TIMA on falling edges
#[derive(Copy, Clone)]
#[repr(u16)]
enum TimaFrequency {
    F00 = 0b10_0000_0000,
    F01 = 0b00_0000_1000,
    F10 = 0b00_0010_0000,
    F11 = 0b00_1000_0000,
}

impl Timer {
    pub fn new() -> Timer {
        Timer {
            div_reg: 0,
            tima_reg: 0,
            tma_reg: 0,
            tac_reg: !TAC_WRITE_MASK,
            tima_freq: TimaFrequency::F00,
            tima_enabled: 0,
        }
    }

    pub fn advance_mcycle(&mut self, ir_system: &mut InterruptSystem) {
        let old_div = self.div_reg;
        self.div_reg = self.div_reg.wrapping_add(4);

        self.update_tima(ir_system, old_div, self.div_reg);
    }

    pub fn read_reg(&self, reg: TimerReg) -> u8 {
        match reg {
            TimerReg::DIV => (self.div_reg >> 8) as u8,
            TimerReg::TIMA => self.tima_reg,
            TimerReg::TMA => self.tma_reg,
            TimerReg::TAC => self.tac_reg,
        }
    }

    pub fn write_reg(&mut self, reg: TimerReg, val: u8) {
        match reg {
            TimerReg::DIV => self.div_reg = 0,
            TimerReg::TIMA => self.tima_reg = val, // Investigate
            TimerReg::TMA => self.tma_reg = val,
            TimerReg::TAC => self.write_tac(val),
        }
    }

    fn update_tima(&mut self, ir_system: &mut InterruptSystem, old_div: u16, new_div: u16) {
        // TIMA is increased when a falling edge is detected from a certain bit in
        // DIV, with the index of the bit depending on the frequence setting in TAC

        let freq_mask = self.tima_freq as u16 & self.tima_enabled;
        if old_div & freq_mask > new_div & freq_mask {
            if let Some(tima) = self.tima_reg.checked_add(1) {
                self.tima_reg = tima;
            } else {
                self.tima_reg = self.tma_reg;
                ir_system.schedule_interrupt(Interrupt::Timer);
            }
        }
    }

    fn write_tac(&mut self, val: u8) {
        if val.bit(2) {
            self.tima_enabled = 0xffff;
        }

        self.tima_freq = match val & 0b11 {
            0b00 => TimaFrequency::F00,
            0b01 => TimaFrequency::F01,
            0b10 => TimaFrequency::F10,
            0b11 => TimaFrequency::F11,
            _ => unsafe { std::hint::unreachable_unchecked() },
        };

        self.tac_reg = (self.tac_reg & (!TAC_WRITE_MASK)) | (val & TAC_WRITE_MASK);
    }
}
