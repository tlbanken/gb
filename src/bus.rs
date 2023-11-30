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

const CART_START: u16 = 0x0000;
const CART_END: u16 = 0x7fff;
const ERAM_START: u16 = 0xa000;
const ERAM_END: u16 = 0xbfff;
const WRAM_START: u16 = 0xc000;
const WRAM_END: u16 = 0xdfff;

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

  pub fn read8(&self, addr: u16) -> GbResult<u8> {
    #[cfg(debug_assertions)]
    trace!("READ8 ${:04x}", addr);

    match addr {
      CART_START..=CART_END => self.cart.lazy_dref().read(addr),
      ERAM_START..=ERAM_END => self.eram.lazy_dref().read(addr),
      WRAM_START..=WRAM_END => self.wram.lazy_dref().read(addr),
      // unsupported
      _ => {
        warn!("Unsupported read address: [0x{:04x}]", addr);
        Ok((0))
      }
    }
  }

  pub fn read16(&self, addr: u16) -> GbResult<u16> {
    #[cfg(debug_assertions)]
    trace!("READ8 ${:04x}", addr);

    Ok(match addr {
      CART_START..=CART_END => u16::from_le_bytes([
        self.cart.lazy_dref().read(addr)?,
        self.cart.lazy_dref().read(addr + 1)?,
      ]),
      ERAM_START..=ERAM_END => u16::from_le_bytes([
        self.eram.lazy_dref().read(addr)?,
        self.eram.lazy_dref().read(addr + 1)?,
      ]),
      WRAM_START..=WRAM_END => u16::from_le_bytes([
        self.wram.lazy_dref().read(addr)?,
        self.wram.lazy_dref().read(addr + 1)?,
      ]),

      // unsupported
      _ => {
        warn!("Unsupported read address: [0x{:04x}]", addr);
        0
      }
    })
  }

  pub fn write8(&mut self, addr: u16, val: u8) -> GbResult<()> {
    #[cfg(debug_assertions)]
    trace!("WRITE 0x{:02x} ({}) to ${:04X}", val, val, addr);

    match addr {
      // cartridge banks
      CART_START..=CART_END => self.cart.lazy_dref_mut().write(addr, val),
      // external ram
      ERAM_START..=ERAM_END => self.eram.lazy_dref_mut().write(addr, val),
      // working ram
      WRAM_START..=WRAM_END => self.wram.lazy_dref_mut().write(addr, val),
      // unsupported
      _ => {
        warn!("Unsupported write address: [0x{:04x}]", addr);
        Ok(())
      }
    }
  }

  pub fn write16(&mut self, addr: u16, val: u16) -> GbResult<()> {
    #[cfg(debug_assertions)]
    trace!("WRITE 0x{:02x} ({}) to ${:04X}", val, val, addr);

    let bytes = val.to_le_bytes();
    Ok(match addr {
      // cartridge banks
      CART_START..=CART_END => {
        self.cart.lazy_dref_mut().write(addr, bytes[0])?;
        self.cart.lazy_dref_mut().write(addr, bytes[1])?;
      }
      // external ram
      ERAM_START..=ERAM_END => {
        self.eram.lazy_dref_mut().write(addr, bytes[0])?;
        self.eram.lazy_dref_mut().write(addr, bytes[1])?;
      }
      // working ram
      WRAM_START..=WRAM_END => {
        self.wram.lazy_dref_mut().write(addr, bytes[0])?;
        self.wram.lazy_dref_mut().write(addr, bytes[1])?;
      }
      // unsupported
      _ => {
        warn!("Unsupported write address: [0x{:04x}]", addr);
      }
    })
  }
}
