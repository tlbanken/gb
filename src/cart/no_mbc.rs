//! No mapper. Entire rom fits within the 32Kb of space

use crate::cart::mapper::Mapper;
use crate::cart::{ERAM_END, ERAM_START, RAM_BANK_SIZE, ROM0_START, ROM1_END};
use crate::err::{GbError, GbErrorType, GbResult};
use crate::gb_err;
use log::error;

pub struct NoMbc {
  rom: Vec<u8>,
  ram: Vec<u8>,
}

impl NoMbc {
  pub fn new(rom: Vec<u8>, ram_banks: u32) -> Self {
    Self {
      rom,
      ram: vec![0; ram_banks as usize * RAM_BANK_SIZE],
    }
  }
}

impl Mapper for NoMbc {
  fn read(&self, addr: u16) -> GbResult<u8> {
    match addr {
      ROM0_START..=ROM1_END => Ok(self.rom[addr as usize]),
      ERAM_START..=ERAM_END => Ok(self.ram[addr as usize - ERAM_START as usize]),
      _ => {
        error!("Invalid Read ${:04X}", addr);
        gb_err!(GbErrorType::OutOfBounds)
      }
    }
  }

  fn write(&mut self, addr: u16, val: u8) -> GbResult<()> {
    match addr {
      // sometimes games write to rom for some reason, just ignore it :/
      ROM0_START..=ROM1_END => {}
      ERAM_START..=ERAM_END => self.ram[addr as usize - ERAM_START as usize] = val,
      _ => {
        error!("Invalid Write [{:02X}] -> ${:04X}", val, addr);
        return gb_err!(GbErrorType::OutOfBounds);
      }
    }
    Ok(())
  }
}
