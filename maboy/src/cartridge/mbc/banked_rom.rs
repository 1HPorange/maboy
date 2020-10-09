use crate::address::CRomAddr;
use std::pin::Pin;

// TODO: Be more consistent where warn!, debug!, error! are used

/// Helper struct for all MBCs that allow ROM banking. Allows for efficient bank switching
/// by keeping an internal pointer to the currently active ROM bank offset.
pub struct BankedRom {
    rom: Pin<Box<[u8]>>,
    // TODO: Figure out exact behaviour when a non-existent bank is selected
    mapped_bank: Option<&'static [u8]>,
}

impl BankedRom {
    pub fn new(rom: Box<[u8]>) -> Self {
        let rom = Pin::new(rom);

        // Forgets about the lifetime of our slice. This is safe because it is pinned and also
        // lives inside of self
        let mapped_bank = Some(unsafe { std::mem::transmute(&rom[0x4000..]) });

        Self { rom, mapped_bank }
    }

    /// If the ROM bank does not exist, this activates a "fake" ROM bank which will
    /// only ever return `0xFF` on reads
    pub fn select_bank(&mut self, bank: u8) {
        let bank_idx = bank as usize * 0x4000;

        self.mapped_bank = if self.rom.len() >= bank_idx + 0x4000 {
            log::debug!("Switched to ROM bank {}", bank);
            // Forgets the lifetime of the slice. Safe because we the referenced memory
            // is pinned and lives inside self
            Some(unsafe { std::mem::transmute(&self.rom[bank_idx..]) })
        } else {
            log::warn!("Attempted to switch to non-existent ROM bank {}", bank);
            None
        }
    }

    /// Reads a byte from ROM (bank 0 or the currently active switchable bank)
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
