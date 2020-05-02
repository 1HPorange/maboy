use super::address::{IOReg, ReadAddr, WriteAddr};
use super::interrupt_system::{Interrupt, InterruptSystem};
use super::memory::{cartridge_mem::CartridgeRam, Memory};
use super::ppu::PPU;
use super::serial_port::SerialPort;
pub struct Board<CRAM> {
    mem: Memory<CRAM>,
    ppu: PPU,
    ir_system: InterruptSystem,
    serial_port: SerialPort,
}

impl<CRAM: CartridgeRam> Board<CRAM> {
    pub fn new(mem: Memory<CRAM>) -> Board<CRAM> {
        Board {
            mem,
            ppu: PPU::new(),
            ir_system: InterruptSystem::new(),
            serial_port: SerialPort::new(),
        }
    }

    pub fn advance_mcycle(&mut self) {
        self.ppu.advance_mcycle(&mut self.ir_system);
    }

    pub fn read8(&mut self, addr: u16) -> u8 {
        use ReadAddr::*;

        let result = match ReadAddr::from(addr) {
            Mem(mem_addr) => self.mem.read8(mem_addr),
            VideoMem(vid_mem_addr) => self.ppu.read_video_mem(vid_mem_addr),
            Unusable => unimplemented!(),
            IO(IOReg::Serial(serial_reg)) => self.serial_port.read_reg(serial_reg),
            IO(IOReg::Ppu(ppu_reg)) => self.ppu.read_reg(ppu_reg),
            IO(IOReg::IF) => self.ir_system.read_if(),
            IO(IOReg::Unimplemented(addr)) => {
                unimplemented!("Unimplemented IO read: {:#06X}", addr)
            }
            IO(reg) => {
                println!("Unimplemented IO register read: {:?}", reg);
                0x0 // TODO: FIX!
            }
            IE => self.ir_system.read_ie(),
        };

        self.advance_mcycle();

        result
    }

    pub fn write8(&mut self, addr: u16, val: u8) {
        use WriteAddr::*;

        match WriteAddr::from(addr) {
            ROM(addr) => println!("Unimplemented MBC stuff"),
            Mem(mem_addr) => self.mem.write8(mem_addr, val),
            VideoMem(vid_mem_addr) => self.ppu.write_video_mem(vid_mem_addr, val),
            Unusable => println!("Unimplemented write to unusable memory"),
            IO(IOReg::Serial(serial_reg)) => self.serial_port.write_reg(serial_reg, val),
            IO(IOReg::Ppu(ppu_reg)) => self.ppu.write_reg(&mut self.ir_system, ppu_reg, val),
            IO(IOReg::BOOT_ROM_DISABLE) => self.mem.write_ff50(val),
            IO(IOReg::IF) => self.ir_system.write_if(val),
            IO(IOReg::Unimplemented(addr)) => println!("Unimplemented IO write: {:#06X}", addr),
            IO(reg) => println!("Unimplemented IO write: {:?}", reg),
            IE => self.ir_system.write_ie(val),
        }

        self.advance_mcycle();
    }

    pub fn read16(&mut self, addr: u16) -> u16 {
        u16::from_le_bytes([self.read8(addr), self.read8(addr.wrapping_add(1))])
    }

    pub fn write16(&mut self, addr: u16, val: u16) {
        self.write8(addr, (val & 0xff) as u8);
        self.write8(addr.wrapping_add(1), (val >> 8) as u8);
    }

    pub fn query_video_frame_ready(&self) -> Option<&[super::ppu::mem_frame::MemPixel]> {
        self.ppu.query_video_frame_ready()
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
