//! PPU for the Gameboy emulator.

use crate::err::{GbError, GbErrorType, GbResult};
use crate::gb_err;
use crate::screen::Screen;
use log::warn;
use std::cell::RefCell;
use std::rc::Rc;

// 8Kb of video memory
const VRAM_SIZE: usize = 8 * 1024;
const LCDC_ADDR: u16 = 0xff40;
const STAT_ADDR: u16 = 0xff41;
const LY_ADDR: u16 = 0xff44;
const LYC_ADDR: u16 = 0xff45;

/// LCD Control register
struct LcdControl {
  /// raw byte for faster reads
  pub raw: u8,

  /// bit 0: 0 = Off; 1 = On
  pub bg_win_enabled: bool,
  /// bit 1: 0 = Off; 1 = On
  pub obj_enabled: bool,
  /// bit 2: 0 = 8×8; 1 = 8×16
  pub large_obj_size: bool,
  /// bit 3: 0 = 9800–9BFF; 1 = 9C00–9FFF
  pub bg_tile_map_hi: bool,
  /// bit 4: 0 = 8800–97FF; 1 = 8000–8FFF
  pub win_bg_data_map_lo: bool,
  /// bit 5: 0 = Off; 1 = On
  pub window_enabled: bool,
  /// bit 6: 0 = 9800–9BFF; 1 = 9C00–9FFF
  pub window_tile_map_hi: bool,
  /// bit 7: 0 = Off; 1 = On
  pub lcd_ppu_enabled: bool,
}

impl LcdControl {
  pub fn new() -> LcdControl {
    LcdControl {
      raw: 0,
      bg_win_enabled: false,
      obj_enabled: false,
      large_obj_size: false,
      bg_tile_map_hi: false,
      win_bg_data_map_lo: false,
      window_enabled: false,
      window_tile_map_hi: false,
      lcd_ppu_enabled: false,
    }
  }

  pub fn write(&mut self, byte: u8) {
    self.raw = byte;
    self.bg_win_enabled = (byte >> 0) & 0x1 > 0;
    self.obj_enabled = (byte >> 1) & 0x1 > 0;
    self.large_obj_size = (byte >> 2) & 0x1 > 0;
    self.bg_tile_map_hi = (byte >> 3) & 0x1 > 0;
    self.win_bg_data_map_lo = (byte >> 4) & 0x1 > 0;
    self.window_enabled = (byte >> 5) & 0x1 > 0;
    self.window_tile_map_hi = (byte >> 6) & 0x1 > 0;
    self.lcd_ppu_enabled = (byte >> 7) & 0x1 > 0;
  }
}

/// LCD Status Register
struct LcdStatus {
  /// Raw Byte for faster reads
  pub raw: u8,

  #[rustfmt::skip]
  /// Bit 0-1: PPU mode (Read-only)
  /// 
  /// Mode | Action                                     | Duration                             | Accessible video memory
  /// -----|--------------------------------------------|--------------------------------------|-------------------------
  ///   2  | Searching for OBJs which overlap this line | 80 dots                              | VRAM, CGB palettes
  ///   3  | Sending pixels to the LCD                  | Between 172 and 289 dots, see below  | None
  ///   0  | Waiting until the end of the scanline      | 376 - mode 3's duration              | VRAM, OAM, CGB palettes
  ///   1  | Waiting until the next frame               | 4560 dots (10 scanlines)             | VRAM, OAM, CGB palettes
  pub ppu_mode: u8,

  /// Bit 2: LYC == LY (Read-only): Set when LY contains the same value as LYC;
  /// it is constantly updated
  pub lyc_eq_ly: bool,

  /// Bit 3: Mode 0 int select (Read/Write): If set, selects the Mode 0
  /// condition
  pub mode0_int_select: bool,

  /// Bit 4: Mode 1 int select (Read/Write): If set, selects the Mode 1
  /// condition
  pub mode1_int_select: bool,

  /// Bit 5: Mode 2 int select (Read/Write): If set, selects the Mode 2
  /// condition
  pub mode2_int_select: bool,

  /// Bit 6: LYC int select (Read/Write): If set, selects the LYC == LY
  /// condition
  pub lyc_int_select: bool,
}

impl LcdStatus {
  pub fn new() -> LcdStatus {
    LcdStatus {
      raw: 0,
      ppu_mode: 0,
      lyc_eq_ly: false,
      mode0_int_select: false,
      mode1_int_select: false,
      mode2_int_select: false,
      lyc_int_select: false,
    }
  }

  pub fn write(&mut self, byte: u8) {
    // mask ppu_mode and lyc_eq_ly since these are read-only
    self.raw = byte & 0xfc;
    self.mode0_int_select = (byte >> 3) & 0x1 > 0;
    self.mode1_int_select = (byte >> 4) & 0x1 > 0;
    self.mode2_int_select = (byte >> 5) & 0x1 > 0;
    self.lyc_int_select = (byte >> 6) & 0x1 > 0;
  }
}

pub struct Ppu {
  pub vram: Vec<u8>,
  /// lcd control register
  pub lcdc: LcdControl,
  /// LCD y coord ranged from 0-153 (read-only)
  pub ly: u8,
  /// LCD Y value to compare against
  pub lyc: u8,
  /// LCD Status register
  pub stat: LcdStatus,

  // Screen to draw to
  screen: Option<Rc<RefCell<Screen>>>,
}

impl Ppu {
  pub fn new() -> Ppu {
    Ppu {
      vram: vec![0; VRAM_SIZE],
      lcdc: LcdControl::new(),
      stat: LcdStatus::new(),
      ly: 0,
      lyc: 0,
      screen: None,
    }
  }

  pub fn connect_screen(&mut self, screen: Rc<RefCell<Screen>>) -> GbResult<()> {
    match self.screen {
      None => self.screen = Some(screen),
      Some(_) => return gb_err!(GbErrorType::AlreadyInitialized),
    }
    Ok(())
  }

  pub fn step(&mut self) -> GbResult<u8> {
    // TODO: this should probably render a row?

    // TODO: set the mode based on the row we are on

    // TODO: Render background
    // TODO: Render Objects
    // TODO: Render Window

    // TODO: Increment ly and compare with lyc

    todo!()
  }

  pub fn read(&self, addr: u16) -> GbResult<u8> {
    Ok(self.vram[addr as usize])
  }

  pub fn write(&mut self, addr: u16, data: u8) -> GbResult<()> {
    self.vram[addr as usize] = data;
    Ok(())
  }

  pub fn io_read(&self, addr: u16) -> GbResult<u8> {
    match addr {
      LCDC_ADDR => Ok(self.lcdc.raw),
      STAT_ADDR => Ok(self.stat.raw),
      LY_ADDR => Ok(self.ly),
      LYC_ADDR => Ok(self.lyc),
      _ => {
        warn!("Read from unsupported IO Reg: ${:04X}. Returning 0", addr);
        Ok(0)
      }
    }
  }

  pub fn io_write(&mut self, addr: u16, data: u8) -> GbResult<()> {
    match addr {
      LCDC_ADDR => self.lcdc.write(data),
      STAT_ADDR => self.stat.write(data),
      LYC_ADDR => self.lyc = data,
      _ => warn!(
        "Write to unsupported IO Reg: [{:02X}] -> ${:04X}",
        data, addr
      ),
    }
    Ok(())
  }
}
