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
  rom: Vec<[u8; ROM_BANK_SIZE]>,
  ram: Vec<[u8; RAM_BANK_SIZE]>,
  ram_enabled: bool,
  rom_bank: usize,
  // either ram bank or upper 2 bits of rom bank
  secondary_bank: usize,
  simple_bank_mode: bool,
  num_rom_banks: usize,
}

impl Mbc1 {
  pub fn new(rom: Vec<u8>, num_rom_banks: usize, num_ram_banks: usize) -> Self {
    // set up rom
    let mut rom_banks: Vec<[u8; ROM_BANK_SIZE]> = Vec::new();
    for bank in 0..num_rom_banks {
      let bank_offset = bank * ROM_BANK_SIZE;
      let bank_range = bank_offset..(bank_offset + ROM_BANK_SIZE);
      rom_banks.push([0u8; ROM_BANK_SIZE]);
      rom_banks[bank].copy_from_slice(&rom[bank_range]);
    }

    // set up ram
    let mut ram_banks: Vec<[u8; RAM_BANK_SIZE]> = Vec::new();
    for _bank in 0..num_ram_banks {
      ram_banks.push([0u8; RAM_BANK_SIZE]);
    }

    Self {
      rom: rom_banks,
      ram: ram_banks,
      ram_enabled: false,
      rom_bank: 1,
      secondary_bank: 0,
      simple_bank_mode: false,
      num_rom_banks,
    }
  }

  fn get_mapped_rom_bank0(&self) -> usize {
    if self.simple_bank_mode {
      // simple mode has no mapping for bank 0
      0
    } else {
      // use upper bits from secondary bank
      self.secondary_bank << 5
    }
  }

  fn get_mapped_rom_bank1(&self) -> usize {
    (self.secondary_bank << 5) | self.rom_bank
  }

  fn get_mapped_ram_bank(&self) -> usize {
    self.secondary_bank
  }
}

impl Mapper for Mbc1 {
  fn read(&self, addr: u16) -> GbResult<u8> {
    let rel_rom_addr = addr as usize % ROM_BANK_SIZE;
    let rel_ram_addr = addr as usize % RAM_BANK_SIZE;
    match addr {
      ROM0_START..=ROM0_END => Ok(self.rom[self.get_mapped_rom_bank0()][rel_rom_addr]),
      ROM1_START..=ROM1_END => Ok(self.rom[self.get_mapped_rom_bank1()][rel_rom_addr]),
      ERAM_START..=ERAM_END => {
        if self.ram_enabled {
          Ok(self.ram[self.get_mapped_ram_bank()][rel_ram_addr])
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
    let rel_ram_addr = addr as usize % RAM_BANK_SIZE;
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
          self.rom_bank = val as usize % self.num_rom_banks;
        }
      }
      RAM_BANK_NUM_START..=RAM_BANK_NUM_END => {
        self.secondary_bank = val as usize & 0x3;
      }
      BANK_MODE_START..=BANK_MODE_END => self.simple_bank_mode = val & 0x1 > 0,
      ERAM_START..=ERAM_END => {
        if self.ram_enabled {
          let bank = self.get_mapped_ram_bank();
          self.ram[bank][rel_ram_addr] = val
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
