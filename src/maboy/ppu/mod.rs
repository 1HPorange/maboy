pub struct PPU {
    ly_TEMP: u8, // TODO: Maybe move into state enum
}

impl PPU {
    pub fn new() -> PPU {
        PPU { ly_TEMP: 0 }
    }

    pub fn read_ly(&self) -> u8 {
        self.ly_TEMP
    }

    pub fn write_ly(&mut self) {
        self.ly_TEMP = 0;
    }
}
