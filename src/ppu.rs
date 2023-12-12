//! PPU for the Gameboy emulator.

use crate::err::{GbError, GbErrorType, GbResult};
use crate::screen::{Pos, Screen};
use crate::util::LazyDref;
use crate::{bus, gb_err, screen};
use log::{trace, warn};
use std::cell::RefCell;
use std::rc::Rc;

const LCDC_ADDR: u16 = 0xff40;
const STAT_ADDR: u16 = 0xff41;
const SCY_ADDR: u16 = 0xff42;
const SCX_ADDR: u16 = 0xff43;
const LY_ADDR: u16 = 0xff44;
const LYC_ADDR: u16 = 0xff45;
const BGP_ADDR: u16 = 0xff47;

// screen constants
const TRUE_NUM_ROWS: u16 = 256;
const TRUE_NUM_COLS: u16 = 256;

// addresses for vram
const VRAM_SIZE: usize = 8 * 1024;
const TILE_MAP_START_LO: u16 = 0x9800 - bus::PPU_START;
const TILE_MAP_START_HI: u16 = 0x9C00 - bus::PPU_START;
const TILE_DATA_START_LO: u16 = 0x8000 - bus::PPU_START;
const TILE_DATA_START_HI: u16 = 0x8800 - bus::PPU_START;
const TILE_DATA_SIZE: u8 = 16;
const SCREEN_WIDTH: u8 = screen::GB_RESOLUTION.width as u8;
const SCREEN_HEIGHT: u8 = screen::GB_RESOLUTION.height as u8;

// Color Palettes
const PALETTE_GRAY: [screen::Color; 4] = [
  screen::Color::new(1.0, 1.0, 1.0), // white
  screen::Color::new(0.7, 0.7, 0.7), // light gray
  screen::Color::new(0.3, 0.3, 0.3), // dark gray
  screen::Color::new(0.0, 0.0, 0.0), // black
];

const PALETTE_GREEN: [screen::Color; 4] = [
  screen::Color::new(155.0 / 255.0, 188.0 / 255.0, 15.0 / 255.0), // white
  screen::Color::new(139.0 / 255.0, 172.0 / 255.0, 15.0 / 255.0), // light gray
  screen::Color::new(48.0 / 255.0, 98.0 / 255.0, 48.0 / 255.0),   // dark gray
  screen::Color::new(15.0 / 255.0, 56.0 / 255.0, 15.0 / 255.0),   // black
];

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

enum PpuMode {
  HBlank = 0,
  VBlank = 1,
  OamScan = 2,
  Rendering = 3,
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
      ppu_mode: PpuMode::Rendering as u8,
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
  /// Background palette index mapping
  pub bgp: u8,
  /// Scroll X
  pub scx: u8,
  /// Scroll Y
  pub scy: u8,

  // palette
  palette: [screen::Color; 4],

  // timing helpers
  vblank_left: u32,
  hblank_left: u32,

  // Screen to draw to
  screen: Option<Rc<RefCell<Screen>>>,

  // current screen position we are drawing
  pos: screen::Pos,
}

impl Ppu {
  pub fn new() -> Ppu {
    Ppu {
      vram: vec![0; VRAM_SIZE],
      lcdc: LcdControl::new(),
      stat: LcdStatus::new(),
      ly: 0,
      lyc: 0,
      bgp: 0,
      scx: 0,
      scy: 0,
      palette: PALETTE_GRAY,
      vblank_left: 0,
      hblank_left: 0,
      screen: None,
      pos: Pos { x: 0, y: 0 },
    }
  }

  pub fn connect_screen(&mut self, screen: Rc<RefCell<Screen>>) -> GbResult<()> {
    match self.screen {
      None => self.screen = Some(screen),
      Some(_) => return gb_err!(GbErrorType::AlreadyInitialized),
    }
    Ok(())
  }

  pub fn step(&mut self) -> GbResult<()> {
    // TODO: if we are in VBLANK or HBLANK, skip
    // TODO: update mode

    if self.stat.ppu_mode == PpuMode::Rendering as u8 {
      // our pixel coordinate needs to be adjusted for scrolling
      let pos = self.pos_with_scroll();
      trace!("Adjusted Pos: {:?}", pos);

      // TODO: Render background
      // figure out the tile map entry we are on in the tile map table
      // use the tile map entry to read the tile data in the tile data table
      // use the tile data entry to figure out the color of the pixel
      let tile_data_index = self.get_tile_map_entry(pos);
      trace!("Tile Data Index: {}", tile_data_index);
      // next we get the tile data info
      let bg_color = self.get_color_from_tile_data(self.get_tile_data_location(tile_data_index));
      trace!("BG Color: {:?}", bg_color);

      // TODO: Render Objects
      // TODO: Render Window

      // TODO: This should check priorities
      let final_color = bg_color;

      // draw pixel
      self.screen.lazy_dref_mut().set_pixel(self.pos, final_color);
    }

    // update position
    self.update_pos();
    Ok(())
  }

  pub fn read(&self, addr: u16) -> GbResult<u8> {
    Ok(self.vram[addr as usize])
  }

  pub fn write(&mut self, addr: u16, data: u8) -> GbResult<()> {
    // TODO: ignore writes in certain modes
    self.vram[addr as usize] = data;
    Ok(())
  }

  pub fn io_read(&self, addr: u16) -> GbResult<u8> {
    match addr {
      LCDC_ADDR => Ok(self.lcdc.raw),
      STAT_ADDR => Ok(self.stat.raw),
      LY_ADDR => Ok(self.ly),
      LYC_ADDR => Ok(self.lyc),
      BGP_ADDR => Ok(self.bgp),
      SCY_ADDR => Ok(self.scy),
      SCX_ADDR => Ok(self.scx),
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
      BGP_ADDR => self.bgp = data,
      SCY_ADDR => self.scy = data,
      SCX_ADDR => self.scx = data,
      _ => warn!(
        "Write to unsupported IO Reg: [{:02X}] -> ${:04X}",
        data, addr
      ),
    }
    Ok(())
  }

  /// Get's the tile map entry using the current pixel positioning we are
  /// rendering
  fn get_tile_map_entry(&self, pos: screen::Pos) -> u8 {
    // a tile map is a table of 32x32 of tile entries
    // a tile entry is a 1 byte index into the tile data table
    let y_byte = (pos.y / 8) as u16;
    let x_byte = (pos.x / 8) as u16;
    let map_index = y_byte * 32 + x_byte;
    // TODO: for now we will read from $9800, but we should really check LCDC.3
    let map_start = TILE_MAP_START_LO;
    self.vram[(map_start + map_index) as usize]
  }

  /// Get the vram offset for the tile that matches the given `index`
  fn get_tile_data_location(&self, index: u8) -> u16 {
    // TODO: this should be determined by LCDC.4
    let tile_data_start = TILE_DATA_START_LO;
    let location_start = tile_data_start + (index as u16 * TILE_DATA_SIZE as u16);
    // use the y position to figure out which row of the tile we are on
    let fine_y = self.pos.y as u16 % 8;
    // a row is 2 bytes
    location_start + (2 * fine_y)
  }

  /// Given a tile, construct the tile
  fn get_color_from_tile_data(&self, tile_data_location: u16) -> screen::Color {
    let bit_x = 7 - self.pos.x % 8;
    let lo_byte = self.vram[tile_data_location as usize];
    let hi_byte = self.vram[tile_data_location as usize + 1];
    let col_index = ((lo_byte >> bit_x) & 0x1) | (((hi_byte >> bit_x) & 0x1) << 1);
    let palette_index = (self.bgp >> (col_index * 2)) & 0x3;
    self.palette[palette_index as usize]
  }

  fn pos_with_scroll(&self) -> screen::Pos {
    // TODO: offset with scroll x and scroll y
    // self.pos
    Pos {
      x: (self.pos.x + self.scx as u32) % 256,
      y: (self.pos.y + self.scy as u32) % 256,
    }
  }

  fn update_pos(&mut self) {
    // TODO: Redo all this logic
    self.pos.x = (self.pos.x + 1) % SCREEN_WIDTH as u32;
    if self.stat.ppu_mode != PpuMode::VBlank as u8 {
      self.pos.y = if self.pos.x == 0 {
        self.pos.y + 1
      } else {
        self.pos.y
      };
      self.ly = self.pos.y as u8;
      if self.ly == SCREEN_HEIGHT {
        self.stat.ppu_mode = PpuMode::VBlank as u8;
      }
    } else if self.stat.ppu_mode == PpuMode::VBlank as u8 {
      // TODO: Set VBLANK Status
      // we entered the vblank
      self.ly = if self.pos.x == 0 {
        self.ly + 1
      } else {
        self.ly
      };
      if self.ly == 154 {
        // done with vblank
        // TODO: Set correct mode
        self.stat.ppu_mode = PpuMode::Rendering as u8;
        // reset positions
        self.ly = 0;
        self.pos.x = 0;
        self.pos.y = 0;
      }
    }
    // TODO: Set hblank and OAM Scan
    // TODO: Trigger interrupt on lyc compare
  }

  // fn should_render(&mut self) -> bool {
  //   if self.hblank_left > 0 {
  //     self.hblank_left -= 1;
  //     return false;
  //   }
  //   // TODO: OAM Scan
  //   // TODO: reset mode
  //   return true;
  // }
}
