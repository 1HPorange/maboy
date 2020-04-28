use super::cpu::interrupt::Interrupt;
use super::memory::mem_addr::{IOAddr, MemAddr};
use super::memory::{cartridge_mem::CartridgeRam, Memory};
use super::ppu::PPU;
use super::util::BitOps;
pub struct Board<CRAM: CartridgeRam> {
    mem: Memory<CRAM>,
    ppu: PPU,
}

impl<CRAM: CartridgeRam> Board<CRAM> {
    pub fn new(mem: Memory<CRAM>) -> Board<CRAM> {
        Board {
            mem,
            ppu: PPU::new(),
        }
    }

    pub fn advance_mcycle(&mut self) {}

    pub fn read8(&mut self, addr: u16) -> u8 {
        let result = self.mem.read8(MemAddr::from(addr));
        self.advance_mcycle();
        result
    }

    pub fn write8(&mut self, addr: u16, val: u8) {
        let addr = MemAddr::from(addr);

        self.mem.write8(addr, val);

        // TODO: Do special shit after special writes

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
        let if_reg = self.mem.read8(MemAddr::IO(IOAddr::IF));
        let ie_reg = self.mem.read8(MemAddr::IE);
        let request = if_reg & ie_reg & 0x1F;

        if request == 0 {
            return None;
        }

        unsafe {
            for bit in 0..5 {
                if request.bit(bit) {
                    return Some(std::mem::transmute(1u8 << bit));
                }
            }

            std::hint::unreachable_unchecked()
        }
    }
}
