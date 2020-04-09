use super::mem_frame::*;
use crate::maboy::clock;
use crate::maboy::cpu::Interrupt;
use crate::maboy::mmu::MMU;
use crate::maboy::util::Bit;
use crate::maboy::windows::gfx::GfxWindow;
use std::cell::UnsafeCell;

pub struct PPU {
    frame: MemFrame,
}

impl PPU {
    pub fn new() -> PPU {
        PPU {
            frame: MemFrame::new(),
        }
    }

    // TODO: Make this cycle-accurate
    // TODO: Prevent writing to write-protected addresses (e.g. in LCD flags register)
    // TODO: Return 0xFF on illegal reads instead of doing nothing/panicking
    // TODO: Disabling the display grants free access to both OAM and VRAM
    // TODO: Implement Display=off routine
    pub async fn step(&mut self, mmu: &UnsafeCell<MMU<'_>>, mut gfx_window: GfxWindow<'_, '_>) {
        unsafe {
            let mmu = mmu.get();
            let mmu_r = &*mmu;

            'frame: loop {
                let mut frame = gfx_window.next_frame();

                // Clearing here is unneccesary, since we never really "render" anything
                // and instead just perform a copy from CPU
                // frame.clear(&[1.0, 0.0, 1.0, 1.0]);

                for ly in 0..144 {
                    let lcd_ctrl = LCDControl::read(mmu_r);

                    // TODO: Figure out where exactly this goes, and the present timing of it
                    if !lcd_ctrl.lcd_enabled() {
                        clock::ticks(4).await;
                        // Do NOT present here. It incurs a massive performance penalty, even if not blocking
                        continue 'frame;
                    }

                    let mut lcd_stat = LCDStat::read(&mut *mmu);
                    // let other = self.read_other(mmu);

                    // if other.dma_request > 0 {
                    //     // TODO: DMA!
                    //     // TODO: DMA apparently ignores the mode flag and can be done at any time... this is hard!
                    // }

                    // Update LY value in memory
                    (&mut *mmu).write8(0xFF44, ly);

                    let ly_lyc_equal = ly == ly_compare(mmu_r);
                    lcd_stat.set_ly_lyc_equal_flag(ly_lyc_equal);

                    if ly_lyc_equal && lcd_stat.lyc_interrupt_enabled() {
                        (&mut *mmu).request_interrupt(Interrupt::LCD_Stat);
                    }

                    // OAM access
                    lcd_stat.set_mode(LCDMode::OAMSearch);
                    if lcd_stat.oam_interrupt_enabled() {
                        (&mut *mmu).request_interrupt(Interrupt::LCD_Stat);
                    }
                    clock::ticks(80).await;

                    // OAM + VRAM access
                    lcd_stat.set_mode(LCDMode::LCDTransfer);
                    clock::ticks(172).await;

                    // TODO: Use replica of algorithm described in The Ultimate GameBoy Talk
                    self.render_line(mmu_r, ly, lcd_ctrl);

                    // HBlank
                    lcd_stat.set_mode(LCDMode::HBlank);
                    if lcd_stat.h_blank_interrupt_enabled() {
                        (&mut *mmu).request_interrupt(Interrupt::LCD_Stat);
                    }
                    clock::ticks(204).await;
                }

                // VBlank
                frame.copy_from_slice(self.frame.data());
                frame.present(false).expect("Lost graphics device");

                let mut lcd_stat = LCDStat::read(&mut *mmu);
                lcd_stat.set_mode(LCDMode::VBlank);

                if lcd_stat.v_blank_interrupt_enabled() {
                    (&mut *mmu).request_interrupt(Interrupt::LCD_Stat);
                }

                (&mut *mmu).request_interrupt(Interrupt::VBlank);

                for ly in 144..154 {
                    if mmu_r.read8(0xFF46) != 0 {
                        unimplemented!("OAM DMA requested, but not implemented");
                    }

                    // Update LY value in memory
                    (&mut *mmu).write8(0xFF44, ly);
                    lcd_stat.set_ly_lyc_equal_flag(ly == ly_compare(mmu_r));

                    clock::ticks(456).await;
                }
            }
        }
    }

    // TODO: Right now this function is weird and slow. Make it nice and fast.
    // TODO: Think about overflows
    fn render_line(&mut self, mmu: &MMU<'_>, ly: u8, lcd_ctrl: LCDControl) {
        // TODO:
        // 1 tile == 16 bytes, 8by8 px
        // Palette - Bits from highest to lowest: Col for 11, 10, 01, 00 (w 11 being black, 00 being white)
        // Render sprite when obj enable  (lcdc) true
        // 10 sprites MAX PER LINE (40 per screen, but can be hacked via smart DMA!)
        // TODO: 8 by 16 mode rendering

        // Scrolling values
        let scx = scx(mmu);
        let scy = scy(mmu);

        // Screen y
        let y = ly.wrapping_add(scy);

        let line = self.frame.line(ly);

        // Tile map index y
        let ty = y / 8;

        // Tile subindex y
        let tsidx_y = y % 8;

        let bg_palette = GreyscalePalette::dmg_bg(mmu);
        let bg_tile_map_addr = lcd_ctrl.bg_tile_map_addr();
        let bg_wnd_tiles_addr = lcd_ctrl.bg_wnd_tiles_addr();
        let tile_idx_shift = if bg_wnd_tiles_addr == 0x8800 { 128 } else { 0 };

        if lcd_ctrl.bg_enabled() {
            for lx in 0..160 {
                // Screen x
                let x = (lx as u8).wrapping_add(scx);

                // Tile map index x
                let tx = x / 8;

                // Figure out tile index by looking at tile map
                let tidx = mmu
                    .read8(bg_tile_map_addr + 32 * (ty as u16) + (tx as u16))
                    .wrapping_add(tile_idx_shift);

                // Fetch the tile
                let tile =
                    mmu.read16(bg_wnd_tiles_addr + 16 * (tidx as u16) + 2 * (tsidx_y as u16));

                // Tile sub-index (offset of the pixel within the sprite line)
                let tsidx_x = x % 8;

                let col = (((tile >> (7-tsidx_x)) & 0b1) << 1) + // The upper bit of the color;
                    ((tile >> (15-tsidx_x)) & 0b1); // and the lower bit... JESUS CHRIST

                // Transform the color through the palette
                let col = bg_palette.transform_2bit(col as u8);

                // TODO: Investigate a more efficient way of writing memory so we can
                // be more performant on reads. There is a whole lot of stuff we can
                // do with the memory layout / caching.

                line[x as usize] = unsafe { Pixel::from_2bit(col) };
            }
        }
    }
}

fn scy(mmu: &MMU<'_>) -> u8 {
    mmu.read8(0xFF42)
}

fn scx(mmu: &MMU<'_>) -> u8 {
    mmu.read8(0xFF43)
}

fn ly_compare(mmu: &MMU<'_>) -> u8 {
    mmu.read8(0xFF45)
}

fn dma_request(mmu: &MMU<'_>) -> u8 {
    mmu.read8(0xFF46)
}

fn wnd_y(mmu: &MMU<'_>) -> u8 {
    mmu.read8(0xFF4A)
}

fn wnd_x(mmu: &MMU<'_>) -> u8 {
    mmu.read8(0xFF4B) - 7
}

#[repr(transparent)]
struct LCDControl(u8);

impl LCDControl {
    fn read(mmu: &MMU<'_>) -> LCDControl {
        LCDControl(mmu.read8(0xFF40))
    }

    fn lcd_enabled(&self) -> bool {
        self.0.bit(7)
    }

    fn wnd_tile_map_addr(&self) -> u16 {
        if self.0.bit(6) {
            0x9C00
        } else {
            0x9800
        }
    }

    fn window_enabled(&self) -> bool {
        self.0.bit(5)
    }

    fn bg_wnd_tiles_addr(&self) -> u16 {
        if self.0.bit(4) {
            0x8000
        } else {
            0x8800
        }
    }

    fn bg_tile_map_addr(&self) -> u16 {
        if self.0.bit(3) {
            0x9C00
        } else {
            0x9800
        }
    }

    fn large_sprites(&self) -> bool {
        self.0.bit(2)
    }

    fn sprites_enabled(&self) -> bool {
        self.0.bit(1)
    }

    /// TODO: This Flag has a different meaning for CGB
    fn bg_enabled(&self) -> bool {
        self.0.bit(0)
    }
}

#[repr(transparent)]
struct LCDStat<'a>(&'a mut u8);

// TODO: Interrupts!
impl<'a> LCDStat<'a> {
    fn read(mmu: &'a mut MMU<'_>) -> LCDStat<'a> {
        LCDStat(mmu.mut8(0xFF41).unwrap())
    }

    fn lyc_interrupt_enabled(&self) -> bool {
        self.0.bit(6)
    }

    /// aka mode 2 interrupt
    fn oam_interrupt_enabled(&self) -> bool {
        self.0.bit(5)
    }

    /// aka mode 1 interrupt
    fn v_blank_interrupt_enabled(&self) -> bool {
        self.0.bit(4)
    }

    /// aka mode 0 interrupt
    fn h_blank_interrupt_enabled(&self) -> bool {
        self.0.bit(3)
    }

    fn set_ly_lyc_equal_flag(&mut self, eq: bool) {
        *self.0 &= !0b100;
        if eq {
            *self.0 += 0b100;
        }
    }

    fn set_mode(&mut self, mode: LCDMode) {
        *self.0 &= !0b11;
        *self.0 += mode as u8;
    }
}

#[repr(u8)]
enum LCDMode {
    HBlank = 0,
    VBlank = 1,
    OAMSearch = 2,
    LCDTransfer = 3,
}

#[repr(transparent)]
struct OAMEntry([u8; 4]);

impl OAMEntry {
    fn pos_y(&self) -> u8 {
        self.0[0]
    }

    fn pos_x(&self) -> u8 {
        self.0[1]
    }

    fn tile_num(&self) -> u8 {
        self.0[2]
    }

    fn draw_behind_bg(&self) -> bool {
        self.0[3].bit(7)
    }

    fn flip_y(&self) -> bool {
        self.0[3].bit(6)
    }

    fn flip_x(&self) -> bool {
        self.0[3].bit(5)
    }

    /// Respected only in Non-GBC mode
    fn use_secondary_palette(&self) -> bool {
        self.0[3].bit(4)
    }

    /// Respected only in GBC mode
    fn use_secondary_tile_ram(&self) -> bool {
        self.0[3].bit(3)
    }

    /// Respected only in GBC mode
    fn palette_num(&self) -> u8 {
        self.0[3] & 0b11
    }
}

struct GreyscalePalette(u8);

impl GreyscalePalette {
    fn dmg_bg(mmu: &MMU<'_>) -> GreyscalePalette {
        GreyscalePalette(mmu.read8(0xFF47))
    }

    fn dmg_sprite_0(mmu: &MMU<'_>) -> GreyscalePalette {
        GreyscalePalette(mmu.read8(0xFF48))
    }

    fn dmg_sprite_1(mmu: &MMU<'_>) -> GreyscalePalette {
        GreyscalePalette(mmu.read8(0xFF49))
    }

    // TODO: Think about what to do about overflowing shifts... do we really want to pay every time???
    fn transform_2bit(&self, raw_col: u8) -> u8 {
        (self.0 >> (2 * raw_col)) & 0b11
    }
}
