//! Timing is a surprisingly comlex matter, especially when you have such tight
//! timing constraints as a Game Boy emulator. To achieve 59.7 fps faithfully and
//! independently of display refresh rate is one thing, but not hogging the CPU
//! while we are waiting is an equal concern. Power saving is a priority of MaBoy.
//!
//! MaBoy uses the Win32 API `WaitableTimer` to achieve this. Although the resolution
//! is somewhat low, practical results show that it is sufficient for our purposes.

use std::mem::{self, MaybeUninit};
use std::ptr;
use std::time::Duration;
use winapi::shared::minwindef::{FALSE, TRUE};
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::handleapi::CloseHandle;
use winapi::um::profileapi::{QueryPerformanceCounter, QueryPerformanceFrequency};
use winapi::um::synchapi::{CreateWaitableTimerW, SetWaitableTimer, WaitForSingleObject};
use winapi::um::winbase::{INFINITE, WAIT_FAILED};
use winapi::um::winnt::HANDLE;
use winapi::um::winnt::LARGE_INTEGER;

/// Provides access to operating system level timing functionality
pub struct OsTiming {
    target_frame_duration: i64,
    waitable_timer: HANDLE,
    /// Frequency of the QueryPerformanceCounter
    qpc_freq: LARGE_INTEGER,
    last_frame_start: LARGE_INTEGER,
}

#[derive(Debug)]
pub enum TimerError {
    CouldNotCreate(u32),
    CouldNotDetermineTimerFrequency(u32),
    CouldNotDetermineTime(u32),
    CouldNotStartTimer(u32),
    FailedToWaitForFrame(u32),
}

impl OsTiming {
    /// Creation may fail if the operating system or hardware doesn't support to neccessary APIs.
    /// In practice, this has never happened, so daring programmers may unwrap this result without
    /// losing sleep.
    pub fn new(target_frame_rate: f64) -> Result<OsTiming, TimerError> {
        unsafe {
            let t_handle = CreateWaitableTimerW(ptr::null_mut(), TRUE, ptr::null_mut());

            if t_handle.is_null() {
                return Err(TimerError::CouldNotCreate(GetLastError()));
            }

            let mut qpc_freq = mem::zeroed();
            if FALSE == QueryPerformanceFrequency(&mut qpc_freq) {
                return Err(TimerError::CouldNotDetermineTimerFrequency(GetLastError()));
            }

            let mut os_timing = OsTiming {
                target_frame_duration: ((1.0 / target_frame_rate) * *qpc_freq.QuadPart() as f64)
                    as i64,
                waitable_timer: t_handle,
                qpc_freq,
                last_frame_start: MaybeUninit::uninit().assume_init(),
            };

            OsTiming::query_qpc(&mut os_timing.last_frame_start)?;

            Ok(os_timing)
        }
    }

    /// Also returns last frame duration for logging / debugging purposes.
    pub fn notify_frame_start(&mut self) -> Result<Duration, TimerError> {
        unsafe {
            let mut current_pc = MaybeUninit::uninit().assume_init();
            OsTiming::query_qpc(&mut current_pc)?;

            let mut frame_duration = current_pc.QuadPart() - self.last_frame_start.QuadPart();

            // Convert to MICROseconds
            frame_duration *= 1_000_000;
            frame_duration /= self.qpc_freq.QuadPart();

            self.last_frame_start = current_pc;

            Ok(Duration::from_micros(frame_duration as u64))
        }
    }

    /// Does not wait at all if you are already too slow
    pub fn wait_frame_remaining(&self) -> Result<(), TimerError> {
        unsafe {
            let mut current_pc = MaybeUninit::uninit().assume_init();
            OsTiming::query_qpc(&mut current_pc)?;

            let elapsed = current_pc.QuadPart() - self.last_frame_start.QuadPart();

            if elapsed > self.target_frame_duration {
                return Ok(());
            } else {
                // This seems to be the wrong way round, but it isn't, because
                // SetWaitableTimer needs the NEGATIVE duration if you want
                // it to wait for a relative period (not an absolute timestamp).
                let mut wait_time: LARGE_INTEGER = mem::zeroed();
                *wait_time.QuadPart_mut() = elapsed - self.target_frame_duration;

                if FALSE
                    == SetWaitableTimer(
                        self.waitable_timer,
                        &wait_time,
                        0,
                        None,
                        ptr::null_mut(),
                        TRUE,
                    )
                {
                    return Err(TimerError::CouldNotStartTimer(GetLastError()));
                }

                if WAIT_FAILED == WaitForSingleObject(self.waitable_timer, INFINITE) {
                    Err(TimerError::FailedToWaitForFrame(GetLastError()))
                } else {
                    Ok(())
                }
            }
        }
    }

    fn query_qpc(target: &mut LARGE_INTEGER) -> Result<(), TimerError> {
        unsafe {
            if FALSE == QueryPerformanceCounter(target) {
                Err(TimerError::CouldNotDetermineTime(GetLastError()))
            } else {
                Ok(())
            }
        }
    }
}

impl Drop for OsTiming {
    fn drop(&mut self) {
        unsafe {
            CloseHandle(self.waitable_timer);
        }
    }
}
