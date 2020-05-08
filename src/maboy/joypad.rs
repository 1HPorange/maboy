use super::interrupt_system::{Interrupt, InterruptSystem};
use bitflags::bitflags;

pub struct JoyPad {
    /// aka JOYP
    p1_reg: u8,
    /// Bitflags of all buttons pressed, with 0 = pressed and 1 = released
    pressed: Buttons,
    active_buttons: ActiveButtonGroup,
}

enum ActiveButtonGroup {
    Neither,
    Directional,
    General, // TODO: Think of a better name
    Both,    // TODO: Investigate if this state is even possible
}

bitflags! {
    pub struct Buttons: u8 {
        const Right = 0b_0000_0001;
        const Left = 0b_0000_0010;
        const Up = 0b_0000_0100;
        const Down = 0b_0000_1000;
        const A = 0b_0001_0000;
        const B = 0b_0010_0000;
        const Select = 0b_0100_0000;
        const Start = 0b_1000_0000;
    }
}

/// The write-mask of the P1 register
const P1_MASK: u8 = 0b_0011_0000;

impl JoyPad {
    pub fn new() -> JoyPad {
        JoyPad {
            p1_reg: 0xff,
            pressed: Buttons::all(),
            active_buttons: ActiveButtonGroup::Neither,
        }
    }

    pub fn read_p1(&self) -> u8 {
        (self.p1_reg & 0xf0)
            | match self.active_buttons {
                ActiveButtonGroup::Neither => 0,
                ActiveButtonGroup::Directional => self.pressed.bits() & 0x0f,
                ActiveButtonGroup::General => self.pressed.bits() >> 4,
                ActiveButtonGroup::Both => {
                    (self.pressed.bits() & 0x0f) | (self.pressed.bits() >> 4)
                }
            }
    }

    pub fn write_p1(&mut self, val: u8) {
        self.p1_reg = (self.p1_reg & (!P1_MASK)) | (val & P1_MASK);

        self.active_buttons = match self.p1_reg & 0b_0011_0000 {
            0b_0000_0000 => ActiveButtonGroup::Both,
            0b_0001_0000 => ActiveButtonGroup::Directional,
            0b_0010_0000 => ActiveButtonGroup::General,
            0b_0011_0000 => ActiveButtonGroup::Neither,
            _ => unsafe { std::hint::unreachable_unchecked() },
        }
    }

    pub fn notify_buttons_pressed(&mut self, ir_system: &mut InterruptSystem, buttons: Buttons) {
        if self.pressed.bits() & buttons.bits() != 0 {
            ir_system.schedule_interrupt(Interrupt::Joypad);
        }

        self.pressed.remove(buttons);
    }

    pub fn notify_buttons_released(&mut self, buttons: Buttons) {
        self.pressed.insert(buttons);
    }

    pub fn notify_buttons_state(&mut self, ir_system: &mut InterruptSystem, buttons: Buttons) {
        if self.pressed.bits() & buttons.bits() != 0 {
            ir_system.schedule_interrupt(Interrupt::Joypad);
        }

        self.pressed = unsafe { Buttons::from_bits_unchecked(!buttons.bits()) };
    }
}
