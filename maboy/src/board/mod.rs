//! Everything inside the Game Boy (except the CPU), packaged under one
//! roof (the [`Board`] trait) to avoid passing a million arguments to
//! every function. This is a trait for the simple reason that we can use
//! annotated types to hide the generic parameters, which would be very
//! annoying to carry with us everywhere.

mod oam_dma;

use super::address::{Addr, IOReg, VideoMemAddr};
use super::cartridge::Cartridge;
use super::debug::{CpuEvt, DbgEvtSrc, PpuEvt};
use super::interrupt_system::InterruptSystem;
use super::joypad::{Buttons, JoyPad};
use super::memory::Memory;
use super::ppu::{VideoFrameStatus, PPU};
use super::serial_port::SerialPort;
use super::timer::Timer;
use oam_dma::OamDma;

/// See the [module documentation](super::board)
pub trait Board {
    /// The type of Cartridge that this Game Boy can handle.
    type CMem: Cartridge;

    /// Cpu event logger for debugging purposes
    type CpuDbgEvtSrc: DbgEvtSrc<CpuEvt>;

    /// Ppu event logger for debugging purposes
    type PpuDbgEvtSrc: DbgEvtSrc<PpuEvt>;

    /// Advances all components of the gameboy by 1 machine cycle (4 clock cycles).
    ///
    /// To be called only from the CPU, whenever a cycle is advanced *without*
    /// any memory reads/writes. This method is automatically called when
    /// `read8`, `read16`, `write8`, `write16` are called.
    fn advance_mcycle(&mut self);

    /// Reads a byte from memory *without* consuming a cycle. Should not be called
    /// from the CPU unless for very special cases (like IR handling). This method
    /// is also necessary to handle OAM DMA.
    fn read8_instant(&self, addr: Addr) -> u8;

    /// Reads a byte from memory, consuming a cycle before doing so.
    fn read8(&mut self, addr: u16) -> u8;

    /// Writes a byte to memory, consuming a cycle before doing so.
    fn write8(&mut self, addr: u16, val: u8);

    /// Reads a little-endian 16bit integer from memory *without* consuming a cycle.
    /// Should not be called from the CPU unless for very special cases (like IR handling).
    fn read16_instant(&self, addr: u16) -> u16;

    /// Reads a 16bit little-endian integer from memory, consuming two cycles in the process.
    fn read16(&mut self, addr: u16) -> u16;

    /// Writes a 16bit little-endian integer to memory, consuming two cycles in the process.
    fn write16(&mut self, addr: u16, val: u16);

    /// Provides access to the internal interrupt system to allow the CPU to handle interrupts
    /// correctly.
    fn ir_system(&mut self) -> &mut InterruptSystem;

    /// Push an event to the [`CpuDbgEvtSrc`] implementation
    fn push_cpu_evt(&mut self, evt: CpuEvt);

    /// Push an event to the [`PpuDbgEvtSrc`] implementation
    fn push_ppu_evt(&mut self, evt: PpuEvt);
}

/// The one and only implementation of [`Board`]
pub struct BoardImpl<CMem, CpuDbg, PpuDbg> {
    pub mem: Memory<CMem>,
    pub ppu: PPU,
    pub ir_system: InterruptSystem,
    pub joypad: JoyPad,
    pub oam_dma: OamDma,
    pub timer: Timer,
    pub serial_port: SerialPort,
    pub cpu_evt_src: CpuDbg,
    pub ppu_evt_src: PpuDbg,
}

impl<CMem: Cartridge, CpuDbg: DbgEvtSrc<CpuEvt>, PpuDbg: DbgEvtSrc<PpuEvt>>
    BoardImpl<CMem, CpuDbg, PpuDbg>
{
    pub fn new(mem: Memory<CMem>, cpu_evt_src: CpuDbg, ppu_evt_src: PpuDbg) -> Self {
        Self {
            mem,
            ppu: PPU::new(),
            ir_system: InterruptSystem::new(),
            joypad: JoyPad::new(),
            oam_dma: OamDma::new(),
            timer: Timer::new(),
            serial_port: SerialPort::new(),
            cpu_evt_src,
            ppu_evt_src,
        }
    }

    // See documentaion of this method on [`crate::maboy::Emulator`]
    pub fn query_video_frame_status(&mut self) -> VideoFrameStatus {
        self.ppu.query_frame_status()
    }

    // See documentaion of this method on [`crate::maboy::Emulator`]
    pub fn notify_buttons_pressed(&mut self, buttons: Buttons) {
        self.joypad
            .notify_buttons_pressed(&mut self.ir_system, buttons);
    }

    // See documentaion of this method on [`crate::maboy::Emulator`]
    pub fn notify_buttons_released(&mut self, buttons: Buttons) {
        self.joypad.notify_buttons_released(buttons);
    }

    // See documentaion of this method on [`crate::maboy::Emulator`]
    pub fn notify_buttons_state(&mut self, buttons: Buttons) {
        self.joypad
            .notify_buttons_state(&mut self.ir_system, buttons);
    }
}

impl<CMem: Cartridge, CpuDbg: DbgEvtSrc<CpuEvt>, PpuDbg: DbgEvtSrc<PpuEvt>> Board
    for BoardImpl<CMem, CpuDbg, PpuDbg>
{
    type CMem = CMem;
    type CpuDbgEvtSrc = CpuDbg;
    type PpuDbgEvtSrc = PpuDbg;

    fn advance_mcycle(&mut self) {
        self.timer.advance_mcycle(&mut self.ir_system);
        self.ppu.advance_mcycle(&mut self.ir_system);
        OamDma::advance_mcycle(self);
    }

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

    fn read8(&mut self, addr: u16) -> u8 {
        self.advance_mcycle();

        let result = self.read8_instant(Addr::from(addr));
        self.push_cpu_evt(CpuEvt::ReadMem(addr, result));
        result
    }

    fn write8(&mut self, addr: u16, val: u8) {
        use Addr::*;

        self.advance_mcycle();

        match Addr::from(addr) {
            Mem(mem_addr) => self.mem.write8(mem_addr, val),
            // OAM is unavailable during OAM DMA
            VideoMem(VideoMemAddr::OAM(_)) if self.oam_dma.is_active() => (),
            VideoMem(vid_mem_addr) => self.ppu.write_video_mem(vid_mem_addr, val),
            Unusable => (), // Writes to here are ignored by DMG systems
            IO(IOReg::P1) => self.joypad.write_p1(val),
            IO(IOReg::Serial(serial_reg)) => self.serial_port.write_reg(serial_reg, val),
            IO(IOReg::Timer(timer_reg)) => {
                self.timer.write_reg(&mut self.ir_system, timer_reg, val)
            }
            IO(IOReg::Ppu(ppu_reg)) => self.ppu.write_reg(&mut self.ir_system, ppu_reg, val),
            IO(IOReg::OamDma) => self.oam_dma.write_ff46(val),
            IO(IOReg::BootRomDisable) => self.mem.write_ff50(val),
            IO(IOReg::IF) => self.ir_system.write_if(val),
            IO(IOReg::Unimplemented(addr)) => log::warn!("Unimplemented IO write: {:#06X}", addr),
            IO(reg) => log::warn!("Unimplemented IO write: {:?}", reg),
            IE => self.ir_system.write_ie(val),
        }

        self.push_cpu_evt(CpuEvt::WriteMem(addr, val));
    }

    fn read16_instant(&self, addr: u16) -> u16 {
        u16::from_le_bytes([
            self.read8_instant(Addr::from(addr)),
            self.read8_instant(Addr::from(addr.wrapping_add(1))),
        ])
    }

    fn read16(&mut self, addr: u16) -> u16 {
        u16::from_le_bytes([self.read8(addr), self.read8(addr.wrapping_add(1))])
    }

    fn write16(&mut self, addr: u16, val: u16) {
        self.write8(addr, (val & 0xff) as u8);
        self.write8(addr.wrapping_add(1), (val >> 8) as u8);
    }

    fn ir_system(&mut self) -> &mut InterruptSystem {
        &mut self.ir_system
    }

    fn push_cpu_evt(&mut self, evt: CpuEvt) {
        self.cpu_evt_src.push(evt);
    }

    fn push_ppu_evt(&mut self, evt: PpuEvt) {
        self.ppu_evt_src.push(evt);
    }
}
