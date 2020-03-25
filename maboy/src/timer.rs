//! The Game Boy timer has too many edge cases to explain it here properly.
//! Please consult an external source (like TCAGBD) to learn about it.

use super::address::TimerReg;
use super::interrupt_system::{Interrupt, InterruptSystem};
use super::util::BitOps;

// TODO:  If register IF is written during TimaReloadState::RightAfterReload,
// the written value will overwrite the automatically set interrupt requset... I think?!

/// The timer is a really screwed up thing with lots of oddities.
/// This implementation should be close enough without introducing
/// unneccessary complexity.
pub struct Timer {
    div_reg: u16,
    tima_reg: u8,
    tma_reg: u8,
    tac_reg: u8,
    tima_freq: TimaFrequency,
    /// None when timer disabled, Some otherwise
    tima_enabled: Option<()>,
    tima_reload_state: TimaReloadState,
}

const TAC_WRITE_MASK: u8 = 0b111;

/// Enum values are the bitmask for DIV that triggers an increase in TIMA on falling edges.
/// That was poorly explained... So basically when the bit that is 1 in Fxx goes from 1
/// to 0 in the DIV register, TIMA is increased.
#[derive(Copy, Clone)]
#[repr(u16)]
enum TimaFrequency {
    F00 = 0b10_0000_0000,
    F01 = 0b00_0000_1000,
    F10 = 0b00_0010_0000,
    F11 = 0b00_1000_0000,
}

/// The timer has some behaviour with VERY tight timing. This enum is used
/// to keep track of the exact internal state at all times, even the one that
/// cannot be expressed via register values alone.
enum TimaReloadState {
    NotReloading,
    /// Contains a new value for TIMA or `None` if TIMA should be set to TMA
    InReload(Option<u8>),
    RightAfterReload,
}

impl Timer {
    pub fn new() -> Timer {
        Timer {
            div_reg: 0,
            tima_reg: 0,
            tma_reg: 0,
            tac_reg: !TAC_WRITE_MASK,
            tima_freq: TimaFrequency::F00,
            tima_enabled: None,
            tima_reload_state: TimaReloadState::NotReloading,
        }
    }

    pub fn advance_mcycle(&mut self, ir_system: &mut InterruptSystem) {
        let old_div = self.div_reg;
        self.div_reg = self.div_reg.wrapping_add(4);

        if let TimaReloadState::InReload(new_tima) = self.tima_reload_state {
            self.tima_reg = new_tima.unwrap_or(self.tma_reg);
            ir_system.schedule_interrupt(Interrupt::Timer);
            self.tima_reload_state = TimaReloadState::RightAfterReload;
        } else {
            self.tima_reload_state = TimaReloadState::NotReloading;
        }

        self.update_tima(old_div, self.div_reg);
    }

    pub fn read_reg(&self, reg: TimerReg) -> u8 {
        match reg {
            TimerReg::DIV => (self.div_reg >> 8) as u8,
            TimerReg::TIMA => self.tima_reg,
            TimerReg::TMA => self.tma_reg,
            TimerReg::TAC => self.tac_reg,
        }
    }

    pub fn write_reg(&mut self, ir_system: &mut InterruptSystem, reg: TimerReg, val: u8) {
        match reg {
            TimerReg::DIV => {
                if self.div_reg & self.tima_freq as u16 != 0 {
                    if self.incr_tima() {
                        self.tima_reload_state = TimaReloadState::InReload(None);
                    }
                }

                self.div_reg = 0;
            }
            TimerReg::TIMA => {
                if let TimaReloadState::RightAfterReload = self.tima_reload_state {
                    self.tima_reg = self.tma_reg;
                } else {
                    self.tima_reg = val;

                    if let TimaReloadState::InReload(_) = self.tima_reload_state {
                        self.tima_reload_state = TimaReloadState::InReload(Some(val));
                    }
                }
            }
            TimerReg::TMA => {
                self.tma_reg = val;

                if let TimaReloadState::RightAfterReload = self.tima_reload_state {
                    self.tima_reg = val;
                }
            }
            TimerReg::TAC => self.write_tac(ir_system, val),
        }
    }

    fn update_tima(&mut self, old_div: u16, new_div: u16) {
        // TIMA is increased when a falling edge is detected from a certain bit in
        // DIV, with the index of the bit depending on the frequence setting in TAC

        let freq_mask = self.tima_freq as u16 & self.tima_enabled.map(|_| 0xFFFF).unwrap_or(0);
        if old_div & freq_mask > new_div & freq_mask {
            if self.incr_tima() {
                self.tima_reload_state = TimaReloadState::InReload(None);
            }
        }
    }

    /// Returns true if TIMA overflowed
    #[must_use]
    fn incr_tima(&mut self) -> bool {
        if let Some(tima) = self.tima_reg.checked_add(1) {
            self.tima_reg = tima;
            false
        } else {
            self.tima_reg = 0;
            true
        }
    }

    fn write_tac(&mut self, ir_system: &mut InterruptSystem, val: u8) {
        // Writing to TAC can lead to some unexpected increases in TIMA

        let new_freq = match val & 0b11 {
            0b00 => TimaFrequency::F00,
            0b01 => TimaFrequency::F01,
            0b10 => TimaFrequency::F10,
            0b11 => TimaFrequency::F11,
            _ => unsafe { std::hint::unreachable_unchecked() },
        };

        if val.bit(2) {
            self.tima_enabled = Some(());

            // This is pure black magic, but is documented in TCAGBD
            if self.div_reg & self.tima_freq as u16 == 0 && self.div_reg & new_freq as u16 != 0 {
                if self.incr_tima() {
                    ir_system.schedule_interrupt(Interrupt::Timer);
                }
            }
        } else {
            self.tima_enabled = None;

            // Leads to falling edge => increases tima
            if self.tac_reg.bit(2) && self.div_reg & self.tima_freq as u16 != 0 {
                if self.incr_tima() {
                    ir_system.schedule_interrupt(Interrupt::Timer);
                }
            }
        }

        self.tima_freq = new_freq;

        self.tac_reg = (self.tac_reg & (!TAC_WRITE_MASK)) | (val & TAC_WRITE_MASK);
    }
}
