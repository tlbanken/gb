//! Main Bus for the gameboy emulator. Handles sending reads and writes to the
//! appropriate location.

use std::{cell::RefCell, rc::Rc};

use log::warn;

use crate::{
  err::{GbError, GbErrorType, GbResult},
  gb_err,
  ram::Ram,
};

pub struct Bus {
  wram: Option<Rc<RefCell<Ram>>>,
  eram: Option<Rc<RefCell<Ram>>>,
}

impl Bus {
  pub fn new() -> Bus {
    Bus {
      wram: None,
      eram: None,
    }
  }

  /// Adds a reference to the working ram to the bus
  pub fn connect_wram(&mut self, wram: Rc<RefCell<Ram>>) -> GbResult<()> {
    match self.wram {
      None => self.wram = Some(wram),
      Some(_) => return gb_err!(GbErrorType::AlreadyInitialized),
    }
    Ok(())
  }

  /// Adds a reference to the external ram to the bus
  pub fn connect_eram(&mut self, eram: Rc<RefCell<Ram>>) -> GbResult<()> {
    match self.eram {
      None => self.eram = Some(eram),
      Some(_) => return gb_err!(GbErrorType::AlreadyInitialized),
    }
    Ok(())
  }

  pub fn read(&self, addr: u16) -> GbResult<u8> {
    match addr {
      // external ram
      0xa000..=0xbfff => self.eram.as_ref().unwrap().borrow().read(addr),
      // working ram
      0xc000..=0xdfff => self.wram.as_ref().unwrap().borrow().read(addr),
      // unsupported
      _ => {
        warn!("Unsupported read address: [0x{:04x}]", addr);
        Ok(0)
      }
    }
  }

  pub fn write(&mut self, addr: u16, val: u8) -> GbResult<()> {
    match addr {
      // external ram
      0xa000..=0xbfff => self.eram.as_ref().unwrap().borrow_mut().write(addr, val),
      // working ram
      0xc000..=0xdfff => self.wram.as_ref().unwrap().borrow_mut().write(addr, val),
      // unsupported
      _ => {
        warn!("Unsupported write address: [0x{:04x}]", addr);
        Ok(())
      }
    }
  }
}
