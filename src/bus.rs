//! Main Bus for the gameboy emulator. Handles sending reads and writes to the
//! appropriate location.

use std::{cell::RefCell, rc::Rc};

use log::{debug, info, trace, warn};

use crate::{
  cart::Cartridge,
  err::{GbError, GbErrorType, GbResult},
  gb_err,
  ppu::Ppu,
  ram::Ram,
  util::LazyDref,
};

const CART_START: u16 = 0x0000;
const CART_END: u16 = 0x7fff;
const GPU_START: u16 = 0x8000;
const GPU_END: u16 = 0x9fff;
const ERAM_START: u16 = 0xa000;
const ERAM_END: u16 = 0xbfff;
const WRAM_START: u16 = 0xc000;
const WRAM_END: u16 = 0xdfff;

pub struct Bus {
  wram: Option<Rc<RefCell<Ram>>>,
  eram: Option<Rc<RefCell<Ram>>>,
  cart: Option<Rc<RefCell<Cartridge>>>,
  gpu: Option<Rc<RefCell<Ppu>>>,
}

impl Bus {
  pub fn new() -> Bus {
    Bus {
      wram: None,
      eram: None,
      cart: None,
      gpu: None,
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

  /// Adds a reference to the gpu to the bus
  pub fn connect_gpu(&mut self, gpu: Rc<RefCell<Ppu>>) -> GbResult<()> {
    debug!("Connecting gpu to the bus");
    match self.gpu {
      None => self.gpu = Some(gpu),
      Some(_) => return gb_err!(GbErrorType::AlreadyInitialized),
    }
    Ok(())
  }

  pub fn read8(&self, addr: u16) -> GbResult<u8> {
    #[cfg(debug_assertions)]
    trace!("READ8 ${:04x}", addr);

    // read with relative addressing
    match addr {
      CART_START..=CART_END => self.cart.lazy_dref().read(addr - CART_START),
      GPU_START..=GPU_END => self.gpu.lazy_dref().read(addr - GPU_START),
      ERAM_START..=ERAM_END => self.eram.lazy_dref().read(addr - ERAM_START),
      WRAM_START..=WRAM_END => self.wram.lazy_dref().read(addr - WRAM_START),
      // unsupported
      _ => {
        warn!("Unsupported read8 address: [0x{:04x}]", addr);
        Ok((0))
      }
    }
  }

  pub fn read16(&self, addr: u16) -> GbResult<u16> {
    #[cfg(debug_assertions)]
    trace!("READ8 ${:04x}", addr);

    // read with relative addressing
    Ok(match addr {
      CART_START..=CART_END => u16::from_le_bytes([
        self.cart.lazy_dref().read(addr - CART_START)?,
        self.cart.lazy_dref().read(addr - CART_START + 1)?,
      ]),
      GPU_START..=GPU_END => u16::from_le_bytes([
        self.gpu.lazy_dref().read(addr - GPU_START)?,
        self.gpu.lazy_dref().read(addr - GPU_START + 1)?,
      ]),
      ERAM_START..=ERAM_END => u16::from_le_bytes([
        self.eram.lazy_dref().read(addr - ERAM_START)?,
        self.eram.lazy_dref().read(addr - ERAM_START + 1)?,
      ]),
      WRAM_START..=WRAM_END => u16::from_le_bytes([
        self.wram.lazy_dref().read(addr - WRAM_START)?,
        self.wram.lazy_dref().read(addr - WRAM_START + 1)?,
      ]),

      // unsupported
      _ => {
        warn!("Unsupported read16 address: [0x{:04x}]", addr);
        0
      }
    })
  }

  pub fn write8(&mut self, addr: u16, val: u8) -> GbResult<()> {
    #[cfg(debug_assertions)]
    trace!("WRITE 0x{:02x} ({}) to ${:04X}", val, val, addr);

    // write with relative addressing
    match addr {
      CART_START..=CART_END => self.cart.lazy_dref_mut().write(addr - CART_START, val),
      GPU_START..=GPU_END => self.gpu.lazy_dref_mut().write(addr - GPU_START, val),
      ERAM_START..=ERAM_END => self.eram.lazy_dref_mut().write(addr - ERAM_START, val),
      WRAM_START..=WRAM_END => self.wram.lazy_dref_mut().write(addr - WRAM_START, val),
      // unsupported
      _ => {
        warn!("Unsupported write8 address: [0x{:04x}]", addr);
        Ok(())
      }
    }
  }

  pub fn write16(&mut self, addr: u16, val: u16) -> GbResult<()> {
    #[cfg(debug_assertions)]
    trace!("WRITE 0x{:02x} ({}) to ${:04X}", val, val, addr);

    // write with relative addressing
    let bytes = val.to_le_bytes();
    Ok(match addr {
      CART_START..=CART_END => {
        self
          .cart
          .lazy_dref_mut()
          .write(addr - CART_START, bytes[0])?;
        self
          .cart
          .lazy_dref_mut()
          .write(addr - CART_START, bytes[1])?;
      }
      GPU_START..=GPU_END => {
        self
          .cart
          .lazy_dref_mut()
          .write(addr - GPU_START, bytes[0])?;
        self
          .cart
          .lazy_dref_mut()
          .write(addr - GPU_START, bytes[1])?;
      }
      ERAM_START..=ERAM_END => {
        self
          .eram
          .lazy_dref_mut()
          .write(addr - ERAM_START, bytes[0])?;
        self
          .eram
          .lazy_dref_mut()
          .write(addr - ERAM_START, bytes[1])?;
      }
      WRAM_START..=WRAM_END => {
        self
          .wram
          .lazy_dref_mut()
          .write(addr - WRAM_START, bytes[0])?;
        self
          .wram
          .lazy_dref_mut()
          .write(addr - WRAM_START, bytes[1])?;
      }
      // unsupported
      _ => {
        warn!("Unsupported write16 address: [0x{:04x}]", addr);
      }
    })
  }
}
