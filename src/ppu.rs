//! PPU for the Gameboy emulator.

use crate::err::{GbError, GbErrorType, GbResult};
use crate::screen::{Pos, Screen};
use crate::util::LazyDref;
use crate::{bus, gb_err, screen};
use bit_field::BitField;
use log::{trace, warn};
use std::cell::RefCell;
use std::ops::Range;
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

// LCD Control register
/// bit 0: 0 = Off; 1 = On
const LCDC_BG_WIN_ENABLE: u8 = 0;
/// bit 1: 0 = Off; 1 = On
const LCDC_OBJ_ENABLED: u8 = 1;
/// bit 2: 0 = 8×8; 1 = 8×16
const LCDC_OBJ_SIZE_LARGE: u8 = 2;
/// bit 3: 0 = 9800–9BFF; 1 = 9C00–9FFF
const LCDC_TILE_MAP_HI: u8 = 3;
/// bit 4: 0 = 8800–97FF; 1 = 8000–8FFF
const LCDC_WIN_BG_DATA_MAP_LO: u8 = 4;
/// bit 5: 0 = Off; 1 = On
const LCDC_WIN_ENABLED: u8 = 5;
/// bit 6: 0 = 9800–9BFF; 1 = 9C00–9FFF
const LCDC_WIN_TILE_MAP_HI: u8 = 6;
/// bit 7: 0 = Off; 1 = On
const LCDC_PPU_ENABLED: u8 = 7;

// LCD Status Register
#[rustfmt::skip]
/// Bit 0-1: PPU mode (Read-only)
///
/// Mode | Action                                     | Duration                             | Accessible video memory
/// -----|--------------------------------------------|--------------------------------------|-------------------------
///   2  | Searching for OBJs which overlap this line | 80 dots                              | VRAM, CGB palettes
///   3  | Sending pixels to the LCD                  | Between 172 and 289 dots, see below  | None
///   0  | Waiting until the end of the scanline      | 376 - mode 3's duration              | VRAM, OAM, CGB palettes
///   1  | Waiting until the next frame               | 4560 dots (10 scanlines)             | VRAM, OAM, CGB palettes
const STAT_PPU_MODE: Range<usize> = 0..2;
/// Bit 2: LYC == LY (Read-only): Set when LY contains the same value as LYC;
/// it is constantly updated
const STAT_LYC_EQ_LY: u8 = 2;
/// Bit 3: Mode 0 int select (Read/Write): If set, selects the Mode 0
/// condition
const STAT_MODE0_INT_SELECT: u8 = 3;
/// Bit 4: Mode 1 int select (Read/Write): If set, selects the Mode 1
/// condition
const STAT_MODE1_INT_SELECT: u8 = 4;
/// Bit 5: Mode 2 int select (Read/Write): If set, selects the Mode 2
/// condition
const STAT_MODE2_INT_SELECT: u8 = 5;
/// Bit 6: LYC int select (Read/Write): If set, selects the LYC == LY
/// condition
const STAT_LYC_INT_SELECT: u8 = 6;

#[derive(PartialEq)]
enum PpuMode {
  HBlank = 0,
  VBlank = 1,
  OamScan = 2,
  Rendering = 3,
}

pub struct Ppu {
  pub vram: Vec<u8>,
  /// lcd control register
  pub lcdc: u8,
  /// LCD y coord ranged from 0-153 (read-only)
  pub ly: u8,
  /// LCD Y value to compare against
  pub lyc: u8,
  /// LCD Status register
  pub stat: u8,
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
    // start in rendering mode
    let mut stat = 0;
    stat.set_bits(STAT_PPU_MODE, PpuMode::Rendering as u8);

    Ppu {
      vram: vec![0; VRAM_SIZE],
      lcdc: 0,
      stat,
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

  pub fn step(&mut self, cycle_budget: u32) -> GbResult<()> {
    for _ in 0..cycle_budget {
      self.step_one()?;
    }
    Ok(())
  }

  fn step_one(&mut self) -> GbResult<()> {
    // only draw when we need to
    if self.ppu_mode() == PpuMode::Rendering {
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
      LCDC_ADDR => Ok(self.lcdc),
      STAT_ADDR => Ok(self.stat),
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
      LCDC_ADDR => self.lcdc = data,
      STAT_ADDR => self.stat = data,
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
    // self.pos
    Pos {
      x: (self.pos.x + self.scx as u32) % 256,
      y: (self.pos.y + self.scy as u32) % 256,
    }
  }

  fn update_pos(&mut self) {
    const HBLANK_START: u32 = 160;
    const HBLANK_END: u32 = 360; // TODO what is the correct value here?
    const VBLANK_START: u32 = 144;
    const VBLANK_END: u32 = 154 + 1;

    // always advance x
    self.pos.x += 1;

    if self.pos.x == HBLANK_START {
      if self.ppu_mode() != PpuMode::VBlank {
        self.set_ppu_mode(PpuMode::HBlank);
      }
    }
    if self.pos.x == HBLANK_END {
      // reset x position and start rendering again if not in vblank
      self.pos.x = 0;
      if self.ppu_mode() != PpuMode::VBlank {
        self.set_ppu_mode(PpuMode::Rendering);
      }
    }
    if self.pos.x == 0 {
      // new row
      self.pos.y += 1;
    }
    if self.pos.y == VBLANK_START {
      self.set_ppu_mode(PpuMode::VBlank);
      // TODO: raise interrupt
    }
    if self.pos.y == VBLANK_END {
      self.pos.y = 0;
      self.set_ppu_mode(PpuMode::Rendering);
    }
    self.ly = self.pos.y as u8;
    // TODO: Trigger interrupt on lyc compare
  }

  fn ppu_mode(&self) -> PpuMode {
    match self.stat.get_bits(STAT_PPU_MODE) {
      mode if mode == PpuMode::Rendering as u8 => PpuMode::Rendering,
      mode if mode == PpuMode::VBlank as u8 => PpuMode::VBlank,
      mode if mode == PpuMode::HBlank as u8 => PpuMode::HBlank,
      mode if mode == PpuMode::OamScan as u8 => PpuMode::OamScan,
      // shouldn't be possible
      _ => panic!("Unknown Ppu Mode!"),
    }
  }

  fn set_ppu_mode(&mut self, mode: PpuMode) {
    self.stat.set_bits(STAT_PPU_MODE, mode as u8);
  }
}
