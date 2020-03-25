//! A delta-based implementation of the real-time clock found in MBC3 cartridges.
//! Delta-based means that it doesn't *constantly* recalculate the current time, but
//! only when it is queried. [`Rtc`] also provides methods to serialize/deserialize
//! it to/from a raw byte vector, which is useful for storing the state of the RTC
//! on disk.
//!
//! The MBC3 RTC is not very straight-forward. I would recommend reading up on it
//! somewhere first before diving into this code.

use crate::{util::BitOps, CartridgeParseError};
use bitflags::bitflags;
use num_enum::TryFromPrimitive;
use std::convert::TryFrom;
use std::{
    mem::size_of,
    time::{Duration, SystemTime},
};

// TODO: Figure out if my understadning of latching is correct
// TODO: Also figure out what fields to serialize (Basically: What
// is powered by the gameboy, and what is powered by the battery?)

pub struct Rtc {
    /// The system time when this RTC was last written to (changed)
    base: SystemTime,
    /// The values of all RTC registers at the system time [`base`]
    base_reg: RtcReg,
    /// The system time when the RTC was latched (frozen) to allow safe reading
    latched: Option<SystemTime>,
    /// The register of the RTC that is currently selected for reading/writing
    selected_reg: RtcRegAddr,
}

impl Rtc {
    pub fn new() -> Self {
        Self {
            base: SystemTime::now(),
            base_reg: RtcReg::default(),
            latched: None,
            selected_reg: RtcRegAddr::Seconds,
        }
    }

    /// Attempt to deserialize the state of this struct from a byte vector that was previously
    /// exported via [`Self::export_metadata`]
    pub fn apply_metadata(&mut self, metadata: Vec<u8>) -> Result<(), CartridgeParseError> {
        if metadata.len() != size_of::<u64>() + 5 {
            return Err(CartridgeParseError::InvalidRtcMetadata);
        }

        let duration_since_epoch = Duration::from_millis(u64::from_le_bytes(
            <[u8; size_of::<u64>()]>::try_from(&metadata[..size_of::<u64>()])
                .map_err(|_| CartridgeParseError::InvalidRtcMetadata)?,
        ));

        let base = SystemTime::UNIX_EPOCH
            .checked_add(duration_since_epoch)
            .ok_or(CartridgeParseError::InvalidRtcMetadata)?;

        let base_reg = RtcReg {
            seconds: metadata[size_of::<u64>() + 0],
            minutes: metadata[size_of::<u64>() + 1],
            hours: metadata[size_of::<u64>() + 2],
            days_lower: metadata[size_of::<u64>() + 3],
            flags: RtcFlags::from_bits(metadata[size_of::<u64>() + 4])
                .ok_or(CartridgeParseError::InvalidRtcMetadata)?,
        };

        self.base = base;
        self.base_reg = base_reg;

        Ok(())
    }

    /// Serializes the current state of the struct to store it on disk
    pub fn export_metadata(&self) -> Vec<u8> {
        let time_since_epoch = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_millis() as u64;

        let mut data = Vec::with_capacity(size_of::<u64>() + 5);

        data.extend_from_slice(&time_since_epoch.to_le_bytes());

        data.push(self.base_reg.seconds);
        data.push(self.base_reg.minutes);
        data.push(self.base_reg.hours);
        data.push(self.base_reg.days_lower);
        data.push(self.base_reg.flags.bits);

        data
    }

    /// If unlatched, latches the current time into the RTC registers. Otherwise, the
    /// latched time is unlatched. Latched registers are only relevant for reading,
    /// writing is not affected.
    pub fn toggle_latched(&mut self) {
        if self.latched.is_some() {
            self.latched = None;
        } else {
            self.latched = Some(SystemTime::now());
        }
    }

    /// Attempts to set currently selected register. If no corresponding
    /// register can be found, returns false, otherwise returns true.
    pub fn try_select_reg(&mut self, val: u8) -> bool {
        if let Ok(reg) = RtcRegAddr::try_from(val) {
            self.selected_reg = reg;
            true
        } else {
            false
        }
    }

    /// Reads the currently mapped register, respecting latched registers
    pub fn read_reg(&self) -> u8 {
        if let Some(latched_at) = self.latched {
            self.calc_reg(
                self.selected_reg,
                latched_at
                    .duration_since(self.base)
                    .unwrap_or(Duration::from_secs(0)),
            )
        } else {
            self.calc_reg(
                self.selected_reg,
                self.base.elapsed().unwrap_or(Duration::from_secs(0)),
            )
        }
    }

    /// Writes to the currently mapped register
    pub fn write_reg(&mut self, val: u8) {
        if let RtcRegAddr::Flags = self.selected_reg {
            // We unforunately have to recalculate all base registers here, since
            // the DAY_MSB and DAY_CARRY bits can't be fooled by any trickery

            let elapsed = self.base.elapsed().unwrap_or(Duration::from_secs(0));

            self.base_reg.seconds = self.calc_reg(RtcRegAddr::Seconds, elapsed);
            self.base_reg.minutes = self.calc_reg(RtcRegAddr::Minutes, elapsed);
            self.base_reg.hours = self.calc_reg(RtcRegAddr::Hours, elapsed);
            self.base_reg.days_lower = self.calc_reg(RtcRegAddr::DaysLower, elapsed);
            self.base_reg.flags = RtcFlags::from_bits_truncate(val);

            // Eliminate drift by subtracting the fractional second that we "forgot about"
            self.base = SystemTime::now() - Duration::from_nanos(elapsed.subsec_nanos() as u64);
        } else {
            // We use a trick here: To avoid recalculating all registers and
            // setting a new self.base, we propagate the relative register
            // difference back to correpsponding register in base_reg.

            let target = self.selected_reg.constrain_value(val);
            let current = self.calc_reg(
                self.selected_reg,
                self.base.elapsed().unwrap_or(Duration::from_secs(0)),
            );

            if target > current {
                *self.base_reg.get_mut(self.selected_reg) += target - current;
            } else if target < current {
                *self.base_reg.get_mut(self.selected_reg) +=
                    self.selected_reg.limit().unwrap_or(u8::MAX) - current - target;
            }

            if let Some(limit) = self.selected_reg.limit() {
                *self.base_reg.get_mut(self.selected_reg) %= limit;
            }
        }
    }

    /// Calculates the current value of a register based on the duration that has elpased since
    /// [`self.base`]
    fn calc_reg(&self, reg: RtcRegAddr, elapsed: Duration) -> u8 {
        match reg {
            RtcRegAddr::Seconds => ((elapsed.as_secs() + self.base_reg.seconds as u64) % 60) as u8,
            RtcRegAddr::Minutes => {
                (((elapsed.as_secs() / 60) + self.base_reg.minutes as u64) % 60) as u8
            }
            RtcRegAddr::Hours => {
                (((elapsed.as_secs() / 3600) + self.base_reg.hours as u64) % 24) as u8
            }
            RtcRegAddr::DaysLower => self
                .base_reg
                .days_lower
                .wrapping_add((elapsed.as_secs() / 86400) as u8),
            RtcRegAddr::Flags => {
                // Note: This cast to u16 will fail if you don't play for around 184 years. Make
                // sure to pass this knowledge to your grandkids.
                let days_raw = ((elapsed.as_secs() / 86400) as u16)
                    + (((self.base_reg.flags.bits & 1) as u16) << 8)
                    + (self.base_reg.days_lower as u16);

                let mut flags = RtcFlags::empty();
                flags.set(RtcFlags::DAY_MSB, days_raw.bit(8));
                flags.set(RtcFlags::DAY_CARRY, days_raw > 0x1FF);
                flags.set(
                    RtcFlags::HALTED,
                    self.base_reg.flags.contains(RtcFlags::HALTED),
                );

                flags.bits
            }
        }
    }
}

#[derive(TryFromPrimitive, Copy, Clone, Debug)]
#[repr(u8)]
enum RtcRegAddr {
    Seconds = 0x8,
    Minutes = 0x9,
    Hours = 0xA,
    DaysLower = 0xB,
    Flags = 0xC,
}

impl RtcRegAddr {
    // TODO: Investigate how this behaves in real hardware
    fn constrain_value(&self, val: u8) -> u8 {
        match self {
            RtcRegAddr::Seconds => val % 60,
            RtcRegAddr::Minutes => val % 60,
            RtcRegAddr::Hours => val % 24,
            RtcRegAddr::DaysLower => val,
            RtcRegAddr::Flags => val,
        }
    }

    fn limit(&self) -> Option<u8> {
        match self {
            RtcRegAddr::Seconds => Some(60),
            RtcRegAddr::Minutes => Some(60),
            RtcRegAddr::Hours => Some(24),
            RtcRegAddr::DaysLower => None,
            RtcRegAddr::Flags => None,
        }
    }
}

#[derive(Default)]
struct RtcReg {
    seconds: u8,
    minutes: u8,
    hours: u8,
    days_lower: u8,
    flags: RtcFlags,
}

impl RtcReg {
    fn get_mut(&mut self, addr: RtcRegAddr) -> &mut u8 {
        match addr {
            RtcRegAddr::Seconds => &mut self.seconds,
            RtcRegAddr::Minutes => &mut self.minutes,
            RtcRegAddr::Hours => &mut self.hours,
            RtcRegAddr::DaysLower => &mut self.days_lower,
            RtcRegAddr::Flags => &mut self.flags.bits,
        }
    }
}

bitflags! {
    #[derive(Default)]
    pub struct RtcFlags: u8 {
        const DAY_MSB = 0b_0000_0001;
        const HALTED = 0b_0100_0000;
        const DAY_CARRY = 0b_1000_0000;
    }
}
