//! Cartridge logic for the gb emulator.

mod header;
mod mapper;
mod mbc1;
mod mbc3;
mod no_mbc;

use crate::cart::mapper::{Mapper, MapperType};
use crate::cart::mbc1::Mbc1;
use crate::cart::mbc3::Mbc3;
use crate::cart::no_mbc::NoMbc;
use crate::err::{GbError, GbErrorType, GbResult};
use crate::gb_err;
use header::*;
use log::{error, info};
use std::fs;
use std::path::PathBuf;

// raw dump of the DMG boot rom. This is loaded into addresses 0x00..=0xff until
// the rom writes to the BANK register at 0xff50
const BOOT_ROM: [u8; 256] = [
  0x31, 0xfe, 0xff, 0xaf, 0x21, 0xff, 0x9f, 0x32, 0xcb, 0x7c, 0x20, 0xfb, 0x21, 0x26, 0xff, 0x0e,
  0x11, 0x3e, 0x80, 0x32, 0xe2, 0x0c, 0x3e, 0xf3, 0xe2, 0x32, 0x3e, 0x77, 0x77, 0x3e, 0xfc, 0xe0,
  0x47, 0x11, 0x04, 0x01, 0x21, 0x10, 0x80, 0x1a, 0xcd, 0x95, 0x00, 0xcd, 0x96, 0x00, 0x13, 0x7b,
  0xfe, 0x34, 0x20, 0xf3, 0x11, 0xd8, 0x00, 0x06, 0x08, 0x1a, 0x13, 0x22, 0x23, 0x05, 0x20, 0xf9,
  0x3e, 0x19, 0xea, 0x10, 0x99, 0x21, 0x2f, 0x99, 0x0e, 0x0c, 0x3d, 0x28, 0x08, 0x32, 0x0d, 0x20,
  0xf9, 0x2e, 0x0f, 0x18, 0xf3, 0x67, 0x3e, 0x64, 0x57, 0xe0, 0x42, 0x3e, 0x91, 0xe0, 0x40, 0x04,
  0x1e, 0x02, 0x0e, 0x0c, 0xf0, 0x44, 0xfe, 0x90, 0x20, 0xfa, 0x0d, 0x20, 0xf7, 0x1d, 0x20, 0xf2,
  0x0e, 0x13, 0x24, 0x7c, 0x1e, 0x83, 0xfe, 0x62, 0x28, 0x06, 0x1e, 0xc1, 0xfe, 0x64, 0x20, 0x06,
  0x7b, 0xe2, 0x0c, 0x3e, 0x87, 0xe2, 0xf0, 0x42, 0x90, 0xe0, 0x42, 0x15, 0x20, 0xd2, 0x05, 0x20,
  0x4f, 0x16, 0x20, 0x18, 0xcb, 0x4f, 0x06, 0x04, 0xc5, 0xcb, 0x11, 0x17, 0xc1, 0xcb, 0x11, 0x17,
  0x05, 0x20, 0xf5, 0x22, 0x23, 0x22, 0x23, 0xc9, 0xce, 0xed, 0x66, 0x66, 0xcc, 0x0d, 0x00, 0x0b,
  0x03, 0x73, 0x00, 0x83, 0x00, 0x0c, 0x00, 0x0d, 0x00, 0x08, 0x11, 0x1f, 0x88, 0x89, 0x00, 0x0e,
  0xdc, 0xcc, 0x6e, 0xe6, 0xdd, 0xdd, 0xd9, 0x99, 0xbb, 0xbb, 0x67, 0x63, 0x6e, 0x0e, 0xec, 0xcc,
  0xdd, 0xdc, 0x99, 0x9f, 0xbb, 0xb9, 0x33, 0x3e, 0x3c, 0x42, 0xb9, 0xa5, 0xb9, 0xa5, 0x42, 0x3c,
  0x21, 0x04, 0x01, 0x11, 0xa8, 0x00, 0x1a, 0x13, 0xbe, 0x20, 0xfe, 0x23, 0x7d, 0xfe, 0x34, 0x20,
  0xf5, 0x06, 0x19, 0x78, 0x86, 0x23, 0x05, 0x20, 0xfb, 0x86, 0x20, 0xfe, 0x3e, 0x01, 0xe0, 0x50,
];

const BOOT_ROM_START: u16 = 0x0000;
const BOOT_ROM_END: u16 = 0x00ff;

// 8 KB ram banks
pub const RAM_BANK_SIZE: usize = 8 * 1024;
// 16 KB rom banks
pub const ROM_BANK_SIZE: usize = 16 * 1024;
// External Ram start
pub const ERAM_START: u16 = 0xa000;
pub const ERAM_END: u16 = 0xbfff;
// ROM Banks Addresses
pub const ROM0_START: u16 = 0x0000;
pub const ROM0_END: u16 = 0x3fff;
pub const ROM1_START: u16 = 0x4000;
pub const ROM1_END: u16 = 0x7fff;

pub struct Cartridge {
  pub path: PathBuf,
  pub mbc: Option<Box<dyn Mapper>>,
  pub header: Header,
  pub loaded: bool,
  pub boot_mode: bool,
}

impl Cartridge {
  pub fn new() -> Cartridge {
    Cartridge {
      mbc: None,
      path: PathBuf::new(),
      header: Header::new(),
      loaded: false,
      boot_mode: true,
    }
  }

  pub fn load(&mut self, path: PathBuf) -> GbResult<()> {
    self.loaded = true;
    let rom = match fs::read(path.clone()) {
      Ok(data) => data,
      Err(why) => {
        error!("Failed to load {}: {}", path.display(), why);
        return gb_err!(GbErrorType::FileError);
      }
    };
    self.path = path.clone();
    info!("Loaded {}", self.path.display());
    self.header.read_header(&Vec::from(&rom[0x100..]))?;
    info!("------- HEADER --------");
    info!("{:?}", self.header);
    info!("----- HEADER END ------");
    match self.header.mapper {
      MapperType::None => self.mbc = Some(Box::new(NoMbc::new(rom, self.header.ram_banks))),
      MapperType::Mbc1 => {
        self.mbc = Some(Box::new(Mbc1::new(
          rom,
          self.header.rom_banks,
          self.header.ram_banks,
        )))
      }
      MapperType::Mbc3 => {
        self.mbc = Some(Box::new(Mbc3::new(
          rom,
          self.header.rom_banks,
          self.header.ram_banks,
        )))
      }
      _ => {
        error!("Unsupported Mapper!");
        return gb_err!(GbErrorType::Unsupported);
      }
    }
    Ok(())
  }

  pub fn cart_path(&self) -> Option<PathBuf> {
    if self.loaded {
      Some(self.path.clone())
    } else {
      None
    }
  }

  pub fn read(&self, addr: u16) -> GbResult<u8> {
    Ok(match addr {
      BOOT_ROM_START..=BOOT_ROM_END => {
        if self.boot_mode {
          BOOT_ROM[addr as usize]
        } else {
          self.mbc.as_ref().unwrap().read(addr)?
        }
      }
      _ => {
        if self.loaded {
          self.mbc.as_ref().unwrap().read(addr)?
        } else {
          // when no cartridge loaded, returns 0xff
          0xff
        }
      }
    })
  }

  pub fn write(&mut self, addr: u16, val: u8) -> GbResult<()> {
    match addr {
      BOOT_ROM_START..=BOOT_ROM_END => {
        if self.boot_mode {
          panic!("Writing to BOOT ROM")
        } else {
          self.mbc.as_mut().unwrap().write(addr, val)?
        }
      }
      _ => {
        if self.loaded {
          self.mbc.as_mut().unwrap().write(addr, val)?
        } else {
          panic!("Writing with no cartrige loaded")
        }
      }
    }
    Ok(())
  }

  pub fn io_read(&self, addr: u16) -> GbResult<u8> {
    match addr {
      0xff50 => Ok(self.boot_mode as u8),
      _ => gb_err!(GbErrorType::OutOfBounds),
    }
  }

  pub fn io_write(&mut self, addr: u16, data: u8) -> GbResult<()> {
    match addr {
      0xff50 => self.boot_mode = data == 0,
      _ => return gb_err!(GbErrorType::OutOfBounds),
    }
    Ok(())
  }
}
