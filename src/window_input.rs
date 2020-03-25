//! Utilities for reading keyboard input from an active window

use std::collections::HashMap;
use winapi::um::winuser::*;

/// Used to query which keys are currently pressed by the user. If you have multiple
/// window that capture input, you need multiple instances of this struct.
///
/// An instance of this struct will not watch every key, but only a subset that is
/// specified during the call to [`WindowInput::from_watched_keys`]. You can have multiple instances
/// for a single window.
///
/// You *must* call ['WindowInput::update'] from the message handler routine of your window.
/// An example can be found in the [crate root documentation].
pub struct WindowInput {
    watched_keys: HashMap<i32, bool>,
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[repr(i32)]
pub enum KeyboardKey {
    A = 'A' as i32,
    B = 'B' as i32,
    C = 'C' as i32,
    D = 'D' as i32,
    E = 'E' as i32,
    F = 'F' as i32,
    G = 'G' as i32,
    H = 'H' as i32,
    I = 'I' as i32,
    J = 'J' as i32,
    K = 'K' as i32,
    L = 'L' as i32,
    M = 'M' as i32,
    N = 'N' as i32,
    O = 'O' as i32,
    P = 'P' as i32,
    R = 'R' as i32,
    S = 'S' as i32,
    T = 'T' as i32,
    U = 'U' as i32,
    V = 'V' as i32,
    W = 'W' as i32,
    X = 'X' as i32,
    Y = 'Y' as i32,
    Z = 'Z' as i32,
    Space = VK_SPACE,
    Return = VK_RETURN,
    Backspace = VK_BACK,
    UpArrow = VK_UP,
    RightArrow = VK_RIGHT,
    DownArrow = VK_DOWN,
    LeftArrow = VK_LEFT,
    ControlLeft = VK_CONTROL,
    ControlRight = VK_RCONTROL,
}

impl WindowInput {
    /// Creates and instance that tracks the specified keys
    pub fn from_watched_keys(watched_keys: &[KeyboardKey]) -> WindowInput {
        WindowInput {
            watched_keys: watched_keys
                .iter()
                .copied()
                .map(|key| key as i32)
                .zip(std::iter::repeat(false))
                .collect(),
        }
    }

    /// Updates the internal state of all watched keys according to the window message.
    /// This function must be called from the message handler routine of your window.
    pub fn update(&mut self, msg: u32, w_param: usize) {
        match msg {
            WM_KEYDOWN => {
                if let Some(pressed) = self.watched_keys.get_mut(&(w_param as i32)) {
                    *pressed = true;
                }
            }
            WM_KEYUP => {
                if let Some(pressed) = self.watched_keys.get_mut(&(w_param as i32)) {
                    *pressed = false;
                }
            }
            _ => (),
        }
    }

    /// Returns an iterator over all *watched* keys that are currently pressed
    pub fn depressed_keys<'a>(&'a self) -> impl 'a + Iterator<Item = KeyboardKey> {
        self.watched_keys
            .iter()
            .filter(|&(_, v)| *v)
            // Safe because `watched_keys` only contains `KeyboardKey` variants
            .map(|(k, _)| unsafe { std::mem::transmute(*k) })
    }

    /// Returns the state of any *watched* key. Returns false for any key that is
    /// not watched.
    pub fn is_pressed(&self, key: KeyboardKey) -> bool {
        *self.watched_keys.get(&(key as i32)).unwrap_or(&false)
    }
}
