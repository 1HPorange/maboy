use super::address::{IOReg, ReadAddr, WriteAddr};
use super::interrupt_system::{Interrupt, InterruptSystem};
use super::memory::{cartridge_mem::CartridgeRam, Memory};
use super::ppu::PPU;
use super::util::BitOps;
pub struct Board<CRAM: CartridgeRam> {
    mem: Memory<CRAM>,
    ppu: PPU,
    ir_system: InterruptSystem,
}

impl<CRAM: CartridgeRam> Board<CRAM> {
    pub fn new(mem: Memory<CRAM>) -> Board<CRAM> {
        Board {
            mem,
            ppu: PPU::new(),
            ir_system: InterruptSystem::new(),
        }
    }

    pub fn advance_mcycle(&mut self) {
        self.ppu.advance_mcycle();
    }

    pub fn read8(&mut self, addr: u16) -> u8 {
        use ReadAddr::*;

        let result = match ReadAddr::from(addr) {
            Mem(mem_addr) => self.mem.read8(mem_addr),
            VideoMem(vid_mem_addr) => self.ppu.read_video_mem(vid_mem_addr),
            Unusable => unimplemented!(),
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
            Unusable => unimplemented!(),
            IO(IOReg::Ppu(ppu_reg)) => self.ppu.write_reg(ppu_reg, val),
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

    // This method has to sit on Board because it doesn't consume cycles,
    // unlike other memory access operations. We don't want to give the
    // CPU the ability to accidentally forget to advance cycles, so we just
    // put the check for interrupts in here.
    pub fn query_interrupt_request(&self) -> Option<Interrupt> {
        self.ir_system.query_interrupt_request()
    }
}
