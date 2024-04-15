//! PPU for the Gameboy emulator.

use crate::err::{GbError, GbErrorType, GbResult};
use crate::int::{Interrupt, Interrupts};
use crate::screen::{Pos, Screen};
use crate::util::LazyDref;
use crate::{
  bus::{self, OAM_END, OAM_START, PPU_END, PPU_START},
  gb_err, screen,
};
use bit_field::BitField;
use log::{trace, warn};
use std::cell::RefCell;
use std::mem::swap;
use std::rc::Rc;

const LCDC_ADDR: u16 = 0xff40;
const STAT_ADDR: u16 = 0xff41;
const SCY_ADDR: u16 = 0xff42;
const SCX_ADDR: u16 = 0xff43;
const LY_ADDR: u16 = 0xff44;
const LYC_ADDR: u16 = 0xff45;
const BGP_ADDR: u16 = 0xff47;
const OBP0_ADDR: u16 = 0xff48;
const OBP1_ADDR: u16 = 0xff49;

// addresses for vram
const VRAM_SIZE: usize = 8 * 1024;
pub const OAM_SIZE: usize = 160;
const TILE_MAP_START_LO: u16 = 0x9800 - bus::PPU_START;
const TILE_MAP_START_HI: u16 = 0x9C00 - bus::PPU_START;
const TILE_DATA_START_LO: u16 = 0x8000 - bus::PPU_START;
const TILE_DATA_START_HI: u16 = 0x9000 - bus::PPU_START;
const TILE_DATA_SIZE: u8 = 16;

// Color Palettes
pub const PALETTE_GRAY: [screen::Color; 4] = [
  screen::Color::new(1.0, 1.0, 1.0), // white
  screen::Color::new(0.7, 0.7, 0.7), // light gray
  screen::Color::new(0.3, 0.3, 0.3), // dark gray
  screen::Color::new(0.0, 0.0, 0.0), // black
];

pub const PALETTE_GREEN: [screen::Color; 4] = [
  screen::Color::new(155.0 / 255.0, 188.0 / 255.0, 15.0 / 255.0), // white
  screen::Color::new(139.0 / 255.0, 172.0 / 255.0, 15.0 / 255.0), // light gray
  screen::Color::new(48.0 / 255.0, 98.0 / 255.0, 48.0 / 255.0),   // dark gray
  screen::Color::new(15.0 / 255.0, 56.0 / 255.0, 15.0 / 255.0),   // black
];

// Custom color for fun
pub const PALETTE_BLUE: [screen::Color; 4] = [
  screen::Color::new(52.0 / 255.0, 204.0 / 255.0, 235.0 / 255.0), // white
  screen::Color::new(52.0 / 255.0, 137.0 / 255.0, 225.0 / 255.0), // light gray
  screen::Color::new(48.0 / 255.0, 48.0 / 255.0, 98.0 / 255.0),   // dark gray
  screen::Color::new(15.0 / 255.0, 15.0 / 255.0, 55.0 / 255.0),   // black
];

#[derive(PartialEq, Copy, Clone)]
enum PpuMode {
  HBlank = 0,
  VBlank = 1,
  OamScan = 2,
  Rendering = 3,
}

impl From<u8> for PpuMode {
  fn from(value: u8) -> Self {
    match value {
      mode if mode == PpuMode::Rendering as u8 => PpuMode::Rendering,
      mode if mode == PpuMode::VBlank as u8 => PpuMode::VBlank,
      mode if mode == PpuMode::HBlank as u8 => PpuMode::HBlank,
      mode if mode == PpuMode::OamScan as u8 => PpuMode::OamScan,
      // shouldn't be possible
      _ => panic!("Unknown Ppu Mode!"),
    }
  }
}

#[derive(Copy, Clone, Debug)]
pub struct LcdControl {
  /// bit 0: 0 = Off; 1 = On
  pub bg_win_enable: bool,
  /// bit 1: 0 = Off; 1 = On
  pub obj_enabled: bool,
  /// bit 2: 0 = 8×8; 1 = 8×16
  pub obj_size_large: bool,
  /// bit 3: 0 = 9800–9BFF; 1 = 9C00–9FFF
  pub bg_tile_map_hi: bool,
  /// bit 4: 0 = 8800–97FF; 1 = 8000–8FFF
  pub win_and_bg_data_map_lo: bool,
  /// bit 5: 0 = Off; 1 = On
  pub win_enabled: bool,
  /// bit 6: 0 = 9800–9BFF; 1 = 9C00–9FFF
  pub win_tile_map_hi: bool,
  /// bit 7: 0 = Off; 1 = On
  pub ppu_enabled: bool,
}

impl From<u8> for LcdControl {
  fn from(value: u8) -> Self {
    Self {
      bg_win_enable: value.get_bit(0),
      obj_enabled: value.get_bit(1),
      obj_size_large: value.get_bit(2),
      bg_tile_map_hi: value.get_bit(3),
      win_and_bg_data_map_lo: value.get_bit(4),
      win_enabled: value.get_bit(5),
      win_tile_map_hi: value.get_bit(6),
      ppu_enabled: value.get_bit(7),
    }
  }
}

impl From<LcdControl> for u8 {
  fn from(value: LcdControl) -> Self {
    let mut val_u8 = 0;
    val_u8.set_bit(0, value.bg_win_enable);
    val_u8.set_bit(1, value.obj_enabled);
    val_u8.set_bit(2, value.obj_size_large);
    val_u8.set_bit(3, value.bg_tile_map_hi);
    val_u8.set_bit(4, value.win_and_bg_data_map_lo);
    val_u8.set_bit(5, value.win_enabled);
    val_u8.set_bit(6, value.win_tile_map_hi);
    val_u8.set_bit(7, value.ppu_enabled);
    val_u8
  }
}

#[derive(Copy, Clone)]
struct Status {
  #[rustfmt::skip]
  /// Bit 0-1: PPU mode (Read-only)
  ///
  /// Mode | Action                                     | Duration                             | Accessible video memory
  /// -----|--------------------------------------------|--------------------------------------|-------------------------
  ///   2  | Searching for OBJs which overlap this line | 80 dots                              | VRAM, CGB palettes
  ///   3  | Sending pixels to the LCD                  | Between 172 and 289 dots, see below  | None
  ///   0  | Waiting until the end of the scanline      | 376 - mode 3's duration              | VRAM, OAM, CGB palettes
  ///   1  | Waiting until the next frame               | 4560 dots (10 scanlines)             | VRAM, OAM, CGB palettes
  pub ppu_mode: PpuMode,
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

impl From<u8> for Status {
  fn from(value: u8) -> Self {
    Self {
      ppu_mode: value.get_bits(0..2).into(),
      lyc_eq_ly: value.get_bit(2),
      mode0_int_select: value.get_bit(3),
      mode1_int_select: value.get_bit(4),
      mode2_int_select: value.get_bit(5),
      lyc_int_select: value.get_bit(6),
    }
  }
}

impl From<Status> for u8 {
  fn from(value: Status) -> Self {
    let mut val_u8 = 0;
    val_u8.set_bits(0..2, value.ppu_mode as u8);
    val_u8.set_bit(2, value.lyc_eq_ly);
    val_u8.set_bit(3, value.mode0_int_select);
    val_u8.set_bit(4, value.mode1_int_select);
    val_u8.set_bit(5, value.mode2_int_select);
    val_u8.set_bit(6, value.lyc_int_select);
    val_u8
  }
}

#[derive(Copy, Clone)]
pub struct ObjAttrFlags {
  pub low_priority: bool,
  pub flip_y: bool,
  pub flip_x: bool,
  pub palette_idx: u8,
  // CGB attributes not included
}

impl From<u8> for ObjAttrFlags {
  fn from(value: u8) -> Self {
    Self {
      low_priority: value.get_bit(7),
      flip_y: value.get_bit(6),
      flip_x: value.get_bit(5),
      palette_idx: value.get_bit(4) as u8,
    }
  }
}

#[derive(Copy, Clone)]
pub struct ObjectAttribute {
  pub y_pos: u8,
  pub x_pos: u8,
  pub tile_idx: u8,
  pub flags: ObjAttrFlags,
}

impl From<[u8; 4]> for ObjectAttribute {
  fn from(value: [u8; 4]) -> Self {
    Self {
      y_pos: value[0],
      x_pos: value[1],
      tile_idx: value[2],
      flags: ObjAttrFlags::from(value[3]),
    }
  }
}

pub struct Ppu {
  pub vram: Vec<u8>,
  pub oam: Vec<u8>,
  /// lcd control register
  pub lcdc: LcdControl,
  /// LCD y coord ranged from 0-153 (read-only)
  pub ly: u8,
  /// LCD Y value to compare against
  pub lyc: u8,
  /// LCD Status register
  pub stat: Status,
  /// Background palette index mapping
  pub bgp: u8,
  /// Scroll X
  pub scx: u8,
  /// Scroll Y
  pub scy: u8,
  /// OAM Cache (max 10 items)
  pub oam_cache: Vec<ObjectAttribute>,
  /// object palette mapping
  pub obp: [u8; 2],

  // palette
  pub palette: [screen::Color; 4],

  // Screen to draw to
  screen: Option<Rc<RefCell<Screen>>>,
  // interrupt controller handle
  ic: Option<Rc<RefCell<Interrupts>>>,

  // current screen position we are drawing
  pos: Pos,
}

impl Ppu {
  pub fn new() -> Ppu {
    // start in rendering mode
    let mut stat: Status = 0.into();
    stat.ppu_mode = PpuMode::Rendering;

    Ppu {
      vram: vec![0; VRAM_SIZE],
      oam: vec![0; OAM_SIZE],
      oam_cache: Vec::new(),
      lcdc: 0.into(),
      stat,
      ly: 0,
      lyc: 0,
      bgp: 0,
      obp: [0; 2],
      scx: 0,
      scy: 0,
      palette: PALETTE_GRAY,
      screen: None,
      ic: None,
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

  /// Adds a reference to the interrupt controller to the ppu
  pub fn connect_ic(&mut self, ic: Rc<RefCell<Interrupts>>) -> GbResult<()> {
    match self.ic {
      None => self.ic = Some(ic),
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
    if self.stat.ppu_mode == PpuMode::Rendering {
      // our pixel coordinate needs to be adjusted for scrolling
      let pos = self.pos_with_scroll();
      trace!("Adjusted Pos: {:?}", pos);

      // Render background
      // figure out the tile map entry we are on in the tile map table
      // use the tile map entry to read the tile data in the tile data table
      // use the tile data entry to figure out the color of the pixel
      let tile_data_index = self.get_tile_map_entry(pos);
      // next we get the tile data info
      let tile_data = self.get_tile_data_location(tile_data_index, pos);
      // now transform that tile data into a color
      let mut pixel_color = self.get_color_from_tile_data(tile_data);

      // TODO: Render Objects
      // find obj attributes from cache
      let objs = self.get_available_cached_objs();
      for attr in objs {
        // get object color
        let obj_color = self.get_color_from_attribute(&attr, pos);

        // check if object should be drawn over background
        if obj_color.is_some() && !attr.flags.low_priority {
          pixel_color = obj_color.unwrap();
        }
      }

      // TODO: Render Window

      // TODO: This should check priorities

      // draw pixel
      self.screen.lazy_dref_mut().set_pixel(self.pos, pixel_color);
    }

    // update position
    self.update_pos();
    Ok(())
  }

  pub fn read(&self, addr: u16) -> GbResult<u8> {
    if (PPU_START..=PPU_END).contains(&addr) {
      Ok(self.vram[(addr - PPU_START) as usize])
    } else if (OAM_START..=OAM_END).contains(&addr) {
      Ok(self.oam[(addr - OAM_START) as usize])
    } else {
      gb_err!(GbErrorType::BadValue)
    }
  }

  pub fn write(&mut self, addr: u16, data: u8) -> GbResult<()> {
    // TODO: ignore writes in certain modes

    if (PPU_START..=PPU_END).contains(&addr) {
      self.vram[(addr - PPU_START) as usize] = data;
    } else if (OAM_START..=OAM_END).contains(&addr) {
      self.oam[(addr - OAM_START) as usize] = data;
    } else {
      return gb_err!(GbErrorType::BadValue);
    }
    Ok(())
  }

  pub fn io_read(&self, addr: u16) -> GbResult<u8> {
    match addr {
      LCDC_ADDR => Ok(self.lcdc.into()),
      STAT_ADDR => Ok(self.stat.into()),
      LY_ADDR => Ok(self.ly),
      LYC_ADDR => Ok(self.lyc),
      BGP_ADDR => Ok(self.bgp),
      SCY_ADDR => Ok(self.scy),
      SCX_ADDR => Ok(self.scx),
      BGP_ADDR => Ok(self.bgp),
      OBP0_ADDR => Ok(self.obp[0]),
      OBP1_ADDR => Ok(self.obp[1]),
      _ => {
        warn!("Read from unsupported IO Reg: ${:04X}. Returning 0", addr);
        Ok(0)
      }
    }
  }

  pub fn io_write(&mut self, addr: u16, data: u8) -> GbResult<()> {
    match addr {
      LCDC_ADDR => self.lcdc = data.into(),
      STAT_ADDR => self.stat = data.into(),
      LYC_ADDR => self.lyc = data,
      BGP_ADDR => self.bgp = data,
      SCY_ADDR => self.scy = data,
      SCX_ADDR => self.scx = data,
      BGP_ADDR => self.bgp = data,
      OBP0_ADDR => self.obp[0] = data,
      OBP1_ADDR => self.obp[1] = data,
      _ => warn!(
        "Write to unsupported IO Reg: [{:02X}] -> ${:04X}",
        data, addr
      ),
    }
    Ok(())
  }

  /// Gets the tile map entry using the current pixel positioning we are
  /// rendering
  fn get_tile_map_entry(&self, pos: screen::Pos) -> u8 {
    // a tile map is a table of 32x32 of tile entries
    // a tile entry is a 1 byte index into the tile data table
    let y_byte = (pos.y / 8) as u16;
    let x_byte = (pos.x / 8) as u16;
    let map_index = y_byte * 32 + x_byte;
    let map_start = if self.lcdc.bg_tile_map_hi {
      TILE_MAP_START_HI
    } else {
      TILE_MAP_START_LO
    };
    self.vram[(map_start + map_index) as usize]
  }

  /// Get the vram offset for the tile that matches the given `index`
  fn get_tile_data_location(&self, index: u8, scrolled_pos: Pos) -> u16 {
    let location_start = if self.lcdc.win_and_bg_data_map_lo {
      TILE_DATA_START_LO + (index as u16 * TILE_DATA_SIZE as u16)
    } else {
      // indexing using this mode requires using a signed index since we can index
      // backwards
      let mut signed_index = index as i8;
      let signed_start = TILE_DATA_START_HI as i32 + (signed_index as i32 * TILE_DATA_SIZE as i32);
      assert!(signed_start >= 0);
      signed_start as u16
    };
    // use the y position to figure out which row of the tile we are on
    let fine_y = scrolled_pos.y as u16 % 8;
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

  /// Given some object attribute data, get the pixel's color.
  fn get_color_from_attribute(
    &self,
    attribute: &ObjectAttribute,
    _scrolled_pos: Pos,
  ) -> Option<screen::Color> {
    // TODO: Maybe need scrolled position?
    let x_rel = (self.pos.x + 8) - attribute.x_pos as u32;
    let bit_x = 7 - (x_rel % 8);
    let tile_size = if self.lcdc.obj_size_large {
      TILE_DATA_SIZE * 2
    } else {
      TILE_DATA_SIZE
    };
    let mut tile_data_location = attribute.tile_idx as usize * tile_size as usize;
    let fine_y = ((self.pos.y + 16) as u8 - attribute.y_pos) as usize;
    // let fine_y = ((scrolled_pos.y + 16) as u8 - attribute.y_pos) as usize;
    tile_data_location += 2 * fine_y;
    let col_index = if fine_y < 8 {
      // first block
      let lo_byte = self.vram[tile_data_location];
      let hi_byte = self.vram[tile_data_location + 1];
      ((lo_byte >> bit_x) & 0x1) | (((hi_byte >> bit_x) & 0x1) << 1)
    } else {
      // second block
      assert!(self.lcdc.obj_size_large);
      let lo_byte = self.vram[tile_data_location + 2];
      let hi_byte = self.vram[tile_data_location + 3];
      ((lo_byte >> bit_x) & 0x1) | (((hi_byte >> bit_x) & 0x1) << 1)
    };
    let palette_index = (self.obp[attribute.flags.palette_idx as usize] >> (col_index * 2)) & 0x3;
    // color index of 0 is transparent
    if col_index == 0 {
      None
    } else {
      Some(self.palette[palette_index as usize])
    }
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
      if self.stat.ppu_mode != PpuMode::VBlank {
        self.stat.ppu_mode = PpuMode::HBlank;
      }
    }
    if self.pos.x == HBLANK_END {
      // reset x position and start rendering again if not in vblank
      self.pos.x = 0;
      if self.stat.ppu_mode != PpuMode::VBlank {
        self.stat.ppu_mode = PpuMode::Rendering;
      }
    }
    if self.pos.x == 0 {
      // new row
      self.pos.y += 1;
      self.ly = self.pos.y as u8;

      // TODO: maybe this needs to happen during an OAM scan period
      if self.stat.ppu_mode != PpuMode::VBlank {
        self.fill_oam_cache();
      }

      // Update stat reg and trigger interrupt on lyc compare
      self.stat.lyc_eq_ly = if self.ly == self.lyc {
        if self.stat.lyc_int_select {
          self.ic.lazy_dref_mut().raise(Interrupt::Lcd);
        }
        true
      } else {
        false
      };
    }
    if self.pos.y == VBLANK_START {
      self.stat.ppu_mode = PpuMode::VBlank;
      self.ic.lazy_dref_mut().raise(Interrupt::Vblank);
    }
    if self.pos.y == VBLANK_END {
      self.pos.y = 0;
      self.stat.ppu_mode = PpuMode::Rendering;
    }
  }

  fn fill_oam_cache(&mut self) {
    // reset cache
    self.oam_cache.clear();

    let mut obj_idx = 0;
    let obj_height = if self.lcdc.obj_size_large { 16 } else { 8 };
    while obj_idx < OAM_SIZE && self.oam_cache.len() < 10 {
      // y position is index 0 so no need to add offsets
      let obj_y = self.oam[obj_idx];
      // object is hidden so no point to add to cache
      if obj_y <= 160 {
        if (obj_y..(obj_y + obj_height)).contains(&(self.ly + 16)) {
          let obj_bytes = [
            self.oam[obj_idx + 0],
            self.oam[obj_idx + 1],
            self.oam[obj_idx + 2],
            self.oam[obj_idx + 3],
          ];
          self.oam_cache.push(ObjectAttribute::from(obj_bytes));
        }
      }
      // obj attribute is 4 bytes
      obj_idx += 4;
      assert!(self.oam_cache.len() <= 10);
    }
  }

  // Gets all available cached objs which could be drawn at this x coord
  fn get_available_cached_objs(&self) -> Vec<ObjectAttribute> {
    let mut objs: Vec<ObjectAttribute> = Vec::new();
    for attribute in &self.oam_cache {
      if (attribute.x_pos..(attribute.x_pos + 8)).contains(&(self.pos.x as u8 + 8)) {
        objs.push(attribute.clone());
      }
    }
    Self::sort_obj_attributes_by_rev_render_order(&mut objs);
    objs
  }

  // Sort the object attrs by largest x coord. Larger X coord are lower priority
  // so iterating over in order will allow to overwrite the color.
  fn sort_obj_attributes_by_rev_render_order(objs: &mut Vec<ObjectAttribute>) {
    // simple insertion sort since objs will be 10 or less in size
    for min_start in 0..objs.len() {
      for i in min_start..objs.len() {
        if objs[i].x_pos < objs[min_start].x_pos {
          objs.swap(i, min_start);
        }
      }
    }
  }
}
