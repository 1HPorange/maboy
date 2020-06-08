use crate::address::CRomAddr;
use std::pin::Pin;

// TODO: Think about if we can reuse this for banked CRAM
pub struct BankedRom {
    rom: Pin<Box<[u8]>>,
    mapped_bank: Option<&'static [u8]>,
}

impl BankedRom {
    pub fn new(rom: Box<[u8]>) -> Self {
        let rom = Pin::new(rom);

        // Forgets about the lifetime of our slice
        let mapped_bank = Some(unsafe { std::mem::transmute(&rom[0x4000..]) });

        Self { rom, mapped_bank }
    }

    // TODO: Be more consistent where warn!, debug!, error! are used
    pub fn select_bank(&mut self, bank: u8) {
        let bank_idx = bank as usize * 0x4000;

        self.mapped_bank = if self.rom.len() >= bank_idx + 0x4000 {
            log::debug!("Switched to ROM bank {}", bank);
            Some(unsafe { std::mem::transmute(&self.rom[bank_idx..]) })
        } else {
            log::warn!("Attempted to switch to non-existent ROM bank {}", bank);
            None
        }
    }

    pub fn read(&self, addr: CRomAddr) -> u8 {
        match addr {
            CRomAddr::CROM0(addr) => self.rom[addr as usize],
            CRomAddr::CROMn(addr) => self
                .mapped_bank
                .map(|bank| bank[addr as usize])
                .unwrap_or(0xff),
        }
    }
}
