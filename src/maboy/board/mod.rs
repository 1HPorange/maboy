mod oam_dma;

use super::address::{Addr, IOReg, VideoMemAddr};
use super::cartridge::CartridgeMem;
use super::interrupt_system::{Interrupt, InterruptSystem};
use super::joypad::{Buttons, JoyPad};
use super::memory::Memory;
use super::ppu::{VideoFrameStatus, PPU};
use super::serial_port::SerialPort;
use super::timer::Timer;
use oam_dma::OamDma;

pub struct Board<C> {
    mem: Memory<C>,
    ppu: PPU,
    ir_system: InterruptSystem,
    pub joypad: JoyPad,
    oam_dma: OamDma,
    timer: Timer,
    serial_port: SerialPort,
}

impl<C: CartridgeMem> Board<C> {
    pub fn new(mem: Memory<C>) -> Board<C> {
        Board {
            mem,
            ppu: PPU::new(),
            ir_system: InterruptSystem::new(),
            joypad: JoyPad::new(),
            oam_dma: OamDma::new(),
            timer: Timer::new(),
            serial_port: SerialPort::new(),
        }
    }

    pub fn advance_mcycle(&mut self) {
        self.timer.advance_mcycle(&mut self.ir_system);
        self.ppu.advance_mcycle(&mut self.ir_system);
        OamDma::advance_mcycle(self);
    }

    /// Necessary for implementing OAM DMA. Doesn't consume any cycles.
    /// This method should never be public to avoid other components
    /// accidentally reading memory "for free".
    fn read8_instant(&self, addr: Addr) -> u8 {
        use Addr::*;

        match addr {
            Mem(mem_addr) => self.mem.read8(mem_addr),
            // OAM is unavailable during OAM DMA
            VideoMem(VideoMemAddr::OAM(_)) if self.oam_dma.is_active() => 0xff,
            VideoMem(vid_mem_addr) => self.ppu.read_video_mem(vid_mem_addr),
            // TODO: Research if read of Unusable always return 0 even in different PPU modes
            Unusable => 0, // Reads from here curiously return 0 on DMG systems
            IO(IOReg::P1) => self.joypad.read_p1(),
            IO(IOReg::Serial(serial_reg)) => self.serial_port.read_reg(serial_reg),
            IO(IOReg::Timer(timer_reg)) => self.timer.read_reg(timer_reg),
            IO(IOReg::Ppu(ppu_reg)) => self.ppu.read_reg(ppu_reg),
            IO(IOReg::OamDma) => self.oam_dma.read_ff46(),
            IO(IOReg::IF) => self.ir_system.read_if(),
            IO(IOReg::Unimplemented(addr)) => {
                log::warn!("Unimplemented IO register read: {:#06X}", addr);
                0xff // TODO: Implement!
            }
            IO(reg) => {
                log::warn!("Unimplemented IO register read: {:?}", reg);
                0xff // TODO: Implement!
            }
            IE => self.ir_system.read_ie(),
        }
    }

    pub fn read8(&mut self, addr: u16) -> u8 {
        let addr = Addr::from(addr);

        self.advance_mcycle();

        self.read8_instant(addr)
    }

    pub fn write8(&mut self, addr: u16, val: u8) {
        use Addr::*;

        let addr = Addr::from(addr);

        self.advance_mcycle();

        match addr {
            Mem(mem_addr) => self.mem.write8(mem_addr, val),
            // OAM is unavailable during OAM DMA
            VideoMem(VideoMemAddr::OAM(_)) if self.oam_dma.is_active() => (),
            VideoMem(vid_mem_addr) => self.ppu.write_video_mem(vid_mem_addr, val),
            Unusable => (), // Writes to here are ignored by DMG systems
            IO(IOReg::P1) => self.joypad.write_p1(val),
            IO(IOReg::Serial(serial_reg)) => self.serial_port.write_reg(serial_reg, val),
            IO(IOReg::Timer(timer_reg)) => self.timer.write_reg(timer_reg, val),
            IO(IOReg::Ppu(ppu_reg)) => self.ppu.write_reg(&mut self.ir_system, ppu_reg, val),
            IO(IOReg::OamDma) => self.oam_dma.write_ff46(val),
            IO(IOReg::BootRomDisable) => self.mem.write_ff50(val),
            IO(IOReg::IF) => self.ir_system.write_if(val),
            IO(IOReg::Unimplemented(addr)) => log::warn!("Unimplemented IO write: {:#06X}", addr),
            IO(reg) => log::warn!("Unimplemented IO write: {:?}", reg),
            IE => self.ir_system.write_ie(val),
        }
    }

    pub fn read16(&mut self, addr: u16) -> u16 {
        u16::from_le_bytes([self.read8(addr), self.read8(addr.wrapping_add(1))])
    }

    pub fn write16(&mut self, addr: u16, val: u16) {
        self.write8(addr, (val & 0xff) as u8);
        self.write8(addr.wrapping_add(1), (val >> 8) as u8);
    }

    pub fn query_video_frame_status(&mut self) -> VideoFrameStatus {
        self.ppu.query_frame_status()
    }

    pub fn notify_buttons_pressed(&mut self, buttons: Buttons) {
        self.joypad
            .notify_buttons_pressed(&mut self.ir_system, buttons);
    }

    pub fn notify_buttons_released(&mut self, buttons: Buttons) {
        self.joypad.notify_buttons_released(buttons);
    }

    pub fn notify_buttons_state(&mut self, buttons: Buttons) {
        self.joypad
            .notify_buttons_state(&mut self.ir_system, buttons);
    }

    // The following methods have to sit on Board because they don't consume
    // cycles, unlike other memory access operations. The postfix "_instant"
    // denotes this behaviour.

    pub fn query_interrupt_request_instant(&self) -> Option<Interrupt> {
        self.ir_system.query_interrupt_request()
    }

    pub fn read_if_instant(&self) -> u8 {
        self.ir_system.read_if()
    }

    pub fn write_if_instant(&mut self, val: u8) {
        self.ir_system.write_if(val);
    }
}
