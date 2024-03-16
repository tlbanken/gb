//! Mbc1 mapper

use crate::cart::mapper::Mapper;
use crate::cart::{
  ERAM_END, ERAM_START, RAM_BANK_SIZE, ROM0_END, ROM0_START, ROM1_END, ROM1_START, ROM_BANK_SIZE,
};
use crate::err::{GbError, GbErrorType, GbResult};
use crate::gb_err;
use log::{error, warn};

const RAM_ENABLE_START: u16 = 0x0000;
const RAM_ENABLE_END: u16 = 0x1fff;
const ROM_BANK_NUM_START: u16 = 0x2000;
const ROM_BANK_NUM_END: u16 = 0x3fff;
const RAM_BANK_NUM_START: u16 = 0x4000;
const RAM_BANK_NUM_END: u16 = 0x5fff;
const BANK_MODE_START: u16 = 0x6000;
const BANK_MODE_END: u16 = 0x7fff;

pub struct Mbc1 {
  rom: Vec<u8>,
  ram: Vec<u8>,
  ram_enabled: bool,
  rom_bank: u32,
  secondary_bank: u32,
  simple_bank_mode: bool,
}

impl Mbc1 {
  pub fn new(rom: Vec<u8>, ram_banks: u32) -> Self {
    Self {
      rom,
      ram: vec![0; (ram_banks * RAM_BANK_SIZE as u32) as usize],
      ram_enabled: false,
      rom_bank: 1,
      secondary_bank: 0,
      simple_bank_mode: false,
    }
  }

  fn map_ram_addr(&self, addr: u16) -> usize {
    let mut mapped_addr = (addr - ERAM_START) as u32;
    if !self.simple_bank_mode {
      mapped_addr += RAM_BANK_SIZE as u32 * self.secondary_bank;
    }
    mapped_addr as usize
  }

  fn map_rom0_addr(&self, addr: u16) -> usize {
    let mut mapped_addr = (addr - ROM0_START) as u32;
    if !self.simple_bank_mode {
      mapped_addr += ROM_BANK_SIZE as u32 * (self.secondary_bank << 5);
    }
    mapped_addr as usize
  }

  fn map_rom1_addr(&self, addr: u16) -> usize {
    let mut mapped_addr = (addr - ROM1_START) as u32;
    mapped_addr += ROM_BANK_SIZE as u32 * ((self.secondary_bank << 5) + self.rom_bank);
    mapped_addr as usize
  }
}

impl Mapper for Mbc1 {
  fn read(&self, addr: u16) -> GbResult<u8> {
    match addr {
      ROM0_START..=ROM0_END => Ok(self.rom[self.map_rom0_addr(addr)]),
      ROM1_START..=ROM1_END => Ok(self.rom[self.map_rom1_addr(addr)]),
      ERAM_START..=ERAM_END => {
        if self.ram_enabled {
          Ok(self.ram[self.map_ram_addr(addr)])
        } else {
          warn!(
            "Reading ERAM @0x{:04x} while disabled! Returning 0xff...",
            addr
          );
          Ok(0xff)
        }
      }
      _ => {
        error!("Invalid Read ${:04X}", addr);
        gb_err!(GbErrorType::OutOfBounds)
      }
    }
  }

  fn write(&mut self, addr: u16, val: u8) -> GbResult<()> {
    match addr {
      RAM_ENABLE_START..=RAM_ENABLE_END => {
        // write $XA to enable ram
        self.ram_enabled = val & 0x0f == 0xa;
      }
      ROM_BANK_NUM_START..=ROM_BANK_NUM_END => {
        // setting to 0 acts as setting to 1
        if val == 0 {
          self.rom_bank = 0x01;
        } else {
          self.rom_bank = val as u32 & 0x1f;
        }
      }
      RAM_BANK_NUM_START..=RAM_BANK_NUM_END => {
        self.secondary_bank = val as u32 & 0x3;
      }
      BANK_MODE_START..=BANK_MODE_END => self.simple_bank_mode = val & 0x1 > 0,
      ERAM_START..=ERAM_END => {
        if self.ram_enabled {
          let mapped_addr = self.map_ram_addr(addr);
          self.ram[mapped_addr] = val
        }
      }
      _ => {
        error!("Invalid Write [{:02X}] -> ${:04X}", val, addr);
        return gb_err!(GbErrorType::OutOfBounds);
      }
    }
    Ok(())
  }
}
