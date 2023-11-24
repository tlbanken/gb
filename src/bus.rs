//! Main Bus for the gameboy emulator. Handles sending reads and writes to the
//! appropriate location.

use std::{cell::RefCell, rc::Rc};

use log::{debug, info, trace, warn};

use crate::{
  cart::Cartridge,
  err::{GbError, GbErrorType, GbResult},
  gb_err,
  ram::Ram,
  util::LazyDref,
};

pub struct Bus {
  wram: Option<Rc<RefCell<Ram>>>,
  eram: Option<Rc<RefCell<Ram>>>,
  cart: Option<Rc<RefCell<Cartridge>>>,
}

impl Bus {
  pub fn new() -> Bus {
    Bus {
      wram: None,
      eram: None,
      cart: None,
    }
  }

  /// Adds a reference to the working ram to the bus
  pub fn connect_wram(&mut self, wram: Rc<RefCell<Ram>>) -> GbResult<()> {
    debug!("Connecting working ram to the bus");
    match self.wram {
      None => self.wram = Some(wram),
      Some(_) => return gb_err!(GbErrorType::AlreadyInitialized),
    }
    Ok(())
  }

  /// Adds a reference to the external ram to the bus
  pub fn connect_eram(&mut self, eram: Rc<RefCell<Ram>>) -> GbResult<()> {
    debug!("Connecting external ram to the bus");
    match self.eram {
      None => self.eram = Some(eram),
      Some(_) => return gb_err!(GbErrorType::AlreadyInitialized),
    }
    Ok(())
  }

  /// Adds a reference to the cartridge to the bus
  pub fn connect_cartridge(&mut self, cart: Rc<RefCell<Cartridge>>) -> GbResult<()> {
    debug!("Connecting cartridge to the bus");
    match self.cart {
      None => self.cart = Some(cart),
      Some(_) => return gb_err!(GbErrorType::AlreadyInitialized),
    }
    Ok(())
  }

  pub fn read(&self, addr: u16) -> GbResult<u8> {
    #[cfg(debug_assertions)]
    trace!("READ ${:04x}", addr);

    match addr {
      // cartridge banks
      0x0000..=0x7fff => self.cart.lazy_dref().read(addr),
      // external ram
      0xa000..=0xbfff => self.eram.lazy_dref().read(addr),
      // working ram
      0xc000..=0xdfff => self.wram.lazy_dref().read(addr),
      // unsupported
      _ => {
        warn!("Unsupported read address: [0x{:04x}]", addr);
        Ok(0)
      }
    }
  }

  pub fn write(&mut self, addr: u16, val: u8) -> GbResult<()> {
    #[cfg(debug_assertions)]
    trace!("WRITE 0x{:02x} ({}) to ${:04X}", val, val, addr);

    match addr {
      // cartridge banks
      0x0000..=0x7fff => self.cart.lazy_dref_mut().write(addr, val),
      // external ram
      0xa000..=0xbfff => self.eram.lazy_dref_mut().write(addr, val),
      // working ram
      0xc000..=0xdfff => self.wram.lazy_dref_mut().write(addr, val),
      // unsupported
      _ => {
        warn!("Unsupported write address: [0x{:04x}]", addr);
        Ok(())
      }
    }
  }
}
