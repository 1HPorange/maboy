use super::mmu::MMU;
use super::util::Bit;
use super::windows::window::Window;

// TODO: Comment
// Pressed is indicated by bits being ZERO

pub struct Input {
    // 0 = pressed, dpad keys in lower 4 bits
    pressed: u8,
    selected: SelectedButtons,
}

impl Input {
    pub fn new() -> Input {
        Input {
            pressed: 0xff,
            selected: SelectedButtons::General, // TODO: Investigate initial state
        }
    }

    // TODO: Take proper key mapping instead of window
    pub fn update(&mut self, window: &Window) {
        self.pressed = 0xff;
        // TODO: Massively unsafe and stupid, but should work for now
        for bit in 0..KeyboardKey::_LEN as u8 {
            if window.is_key_down(unsafe { std::mem::transmute(bit) }) {
                self.pressed &= !(1 << bit);
            }
        }
    }

    pub(super) fn write_ff00(&mut self, reg: &mut u8, val: u8) {
        // TODO: Figure out what the top two bits do when written to
        *reg = val & 0b_0011_0000;

        // TODO: Figure out what happens if special programmers write 1 to both lines
        if val.bit(4) {
            self.selected = SelectedButtons::Directional;
        }

        // For now, we just forget about the directional keys
        if val.bit(5) {
            self.selected = SelectedButtons::General;
        }
    }

    pub(super) fn read_ff00(&self, reg: u8) -> u8 {
        // TODO: Figure out what the top two bits do when read
        reg | 0b_1100_1111
            | match self.selected {
                SelectedButtons::Directional => self.pressed & 0x0f,
                SelectedButtons::General => self.pressed >> 4,
            }
    }
}

#[repr(u8)]
enum SelectedButtons {
    Directional,
    General,
}

#[repr(u8)]
pub enum Button {
    Right = 0,
    Left = 1,
    Up = 2,
    Down = 3,
    A = 4,
    B = 5,
    Select = 6,
    Start = 7,
}

#[repr(u8)]
pub enum KeyboardKey {
    D = 0,
    A = 1,
    W = 2,
    S = 3,
    L = 4,
    K = 5,
    I = 6,
    O = 7,
    _LEN,
}
