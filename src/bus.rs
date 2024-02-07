//! Main Bus for the gameboy emulator. Handles sending reads and writes to the
//! appropriate location.

use std::{cell::RefCell, rc::Rc};

use log::{debug, trace, warn};

use crate::int::Interrupts;
use crate::timer::Timer;
use crate::{
  cart::Cartridge,
  err::{GbError, GbErrorType, GbResult},
  gb_err,
  ppu::Ppu,
  ram::Ram,
  util::LazyDref,
};

pub const CART_START: u16 = 0x0000;
pub const CART_END: u16 = 0x7fff;
pub const CART_IO_START: u16 = 0xff50;
pub const CART_IO_END: u16 = 0xff50;
pub const PPU_START: u16 = 0x8000;
pub const PPU_END: u16 = 0x9fff;
pub const PPU_IO_START: u16 = 0xff40;
pub const PPU_IO_END: u16 = 0xff4b;
pub const ERAM_START: u16 = 0xa000;
pub const ERAM_END: u16 = 0xbfff;
pub const WRAM_START: u16 = 0xc000;
pub const WRAM_END: u16 = 0xdfff;
pub const TIMER_START: u16 = 0xff04;
pub const TIMER_END: u16 = 0xff07;
pub const JOYPAD_EXACT: u16 = 0xff00;
pub const SERIAL_START: u16 = 0xff01;
pub const SERIAL_END: u16 = 0xff02;
pub const AUDIO_START: u16 = 0xff10;
pub const AUDIO_END: u16 = 0xff3f;
pub const HRAM_START: u16 = 0xff80;
pub const HRAM_END: u16 = 0xfffe;
pub const IE_ADDR: u16 = 0xffff;
pub const IF_ADDR: u16 = 0xff0f;
pub struct Bus {
  wram: Option<Rc<RefCell<Ram>>>,
  eram: Option<Rc<RefCell<Ram>>>,
  hram: Option<Rc<RefCell<Ram>>>,
  cart: Option<Rc<RefCell<Cartridge>>>,
  ppu: Option<Rc<RefCell<Ppu>>>,
  ic: Option<Rc<RefCell<Interrupts>>>,
  timer: Option<Rc<RefCell<Timer>>>,
}

impl Bus {
  pub fn new() -> Bus {
    Bus {
      wram: None,
      eram: None,
      hram: None,
      cart: None,
      ppu: None,
      ic: None,
      timer: None,
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

  /// Adds a reference to the high ram to the bus
  pub fn connect_hram(&mut self, hram: Rc<RefCell<Ram>>) -> GbResult<()> {
    debug!("Connecting high ram to the bus");
    match self.hram {
      None => self.hram = Some(hram),
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
  pub fn connect_ppu(&mut self, ppu: Rc<RefCell<Ppu>>) -> GbResult<()> {
    debug!("Connecting gpu to the bus");
    match self.ppu {
      None => self.ppu = Some(ppu),
      Some(_) => return gb_err!(GbErrorType::AlreadyInitialized),
    }
    Ok(())
  }

  /// Adds a reference to the interrupt controller to the bus
  pub fn connect_ic(&mut self, ic: Rc<RefCell<Interrupts>>) -> GbResult<()> {
    debug!("Connecting gpu to the bus");
    match self.ic {
      None => self.ic = Some(ic),
      Some(_) => return gb_err!(GbErrorType::AlreadyInitialized),
    }
    Ok(())
  }

  /// Adds a reference to the timer to the bus
  pub fn connect_timer(&mut self, timer: Rc<RefCell<Timer>>) -> GbResult<()> {
    debug!("Connecting gpu to the bus");
    match self.timer {
      None => self.timer = Some(timer),
      Some(_) => return gb_err!(GbErrorType::AlreadyInitialized),
    }
    Ok(())
  }

  pub fn read8(&self, addr: u16) -> GbResult<u8> {
    #[cfg(debug_assertions)]
    trace!("READ8 ${:04X}", addr);

    // read with relative addressing
    match addr {
      CART_START..=CART_END => self.cart.lazy_dref().read(addr - CART_START),
      CART_IO_START..=CART_IO_END => self.cart.lazy_dref().io_read(addr),
      PPU_START..=PPU_END => self.ppu.lazy_dref().read(addr - PPU_START),
      PPU_IO_START..=PPU_IO_END => self.ppu.lazy_dref().io_read(addr),
      ERAM_START..=ERAM_END => self.eram.lazy_dref().read(addr - ERAM_START),
      WRAM_START..=WRAM_END => self.wram.lazy_dref().read(addr - WRAM_START),
      HRAM_START..=HRAM_END => self.hram.lazy_dref().read(addr - HRAM_START),
      TIMER_START..=TIMER_END => self.timer.lazy_dref().read(addr),
      IE_ADDR | IF_ADDR => self.ic.lazy_dref().read(addr),
      // unsupported
      _ => {
        warn!("Unsupported read8 address: ${:04X}. Returning 0", addr);
        Ok(0)
      }
    }
  }

  pub fn read16(&self, addr: u16) -> GbResult<u16> {
    #[cfg(debug_assertions)]
    trace!("READ16 ${:04X}", addr);

    // read with relative addressing
    Ok(match addr {
      CART_START..=CART_END => u16::from_le_bytes([
        self.cart.lazy_dref().read(addr - CART_START)?,
        self.cart.lazy_dref().read(addr - CART_START + 1)?,
      ]),
      CART_IO_START..=CART_IO_END => u16::from_le_bytes([
        self.cart.lazy_dref().io_read(addr)?,
        self.cart.lazy_dref().io_read(addr + 1)?,
      ]),
      PPU_START..=PPU_END => u16::from_le_bytes([
        self.ppu.lazy_dref().read(addr - PPU_START)?,
        self.ppu.lazy_dref().read(addr - PPU_START + 1)?,
      ]),
      PPU_IO_START..=PPU_IO_END => u16::from_le_bytes([
        self.ppu.lazy_dref().io_read(addr)?,
        self.ppu.lazy_dref().io_read(addr + 1)?,
      ]),
      ERAM_START..=ERAM_END => u16::from_le_bytes([
        self.eram.lazy_dref().read(addr - ERAM_START)?,
        self.eram.lazy_dref().read(addr - ERAM_START + 1)?,
      ]),
      WRAM_START..=WRAM_END => u16::from_le_bytes([
        self.wram.lazy_dref().read(addr - WRAM_START)?,
        self.wram.lazy_dref().read(addr - WRAM_START + 1)?,
      ]),
      HRAM_START..=HRAM_END => u16::from_le_bytes([
        self.hram.lazy_dref().read(addr - HRAM_START)?,
        self.hram.lazy_dref().read(addr - HRAM_START + 1)?,
      ]),
      TIMER_START..=TIMER_END => u16::from_le_bytes([
        self.timer.lazy_dref().read(addr)?,
        self.timer.lazy_dref().read(addr + 1)?,
      ]),
      IF_ADDR | IE_ADDR => u16::from_le_bytes([
        self.ic.lazy_dref().read(addr)?,
        self.ic.lazy_dref().read(addr + 1)?,
      ]),

      // unsupported
      _ => {
        warn!("Unsupported read16 address: ${:04X}. Returning 0", addr);
        0
      }
    })
  }

  pub fn write8(&mut self, addr: u16, val: u8) -> GbResult<()> {
    #[cfg(debug_assertions)]
    trace!("WRITE8 0x{:02x} ({}) to ${:04X}", val, val, addr);

    // write with relative addressing
    match addr {
      CART_START..=CART_END => self.cart.lazy_dref_mut().write(addr - CART_START, val),
      CART_IO_START..=CART_IO_END => self.cart.lazy_dref_mut().io_write(addr, val),
      PPU_START..=PPU_END => self.ppu.lazy_dref_mut().write(addr - PPU_START, val),
      PPU_IO_START..=PPU_IO_END => self.ppu.lazy_dref_mut().io_write(addr, val),
      ERAM_START..=ERAM_END => self.eram.lazy_dref_mut().write(addr - ERAM_START, val),
      WRAM_START..=WRAM_END => self.wram.lazy_dref_mut().write(addr - WRAM_START, val),
      HRAM_START..=HRAM_END => self.hram.lazy_dref_mut().write(addr - HRAM_START, val),
      TIMER_START..=TIMER_END => self.timer.lazy_dref_mut().write(addr, val),
      IE_ADDR | IF_ADDR => self.ic.lazy_dref_mut().write(addr, val),
      // unsupported
      _ => {
        warn!("Unsupported write8 address: [{:02X}] -> ${:04X}", val, addr);
        Ok(())
      }
    }
  }

  pub fn write16(&mut self, addr: u16, val: u16) -> GbResult<()> {
    #[cfg(debug_assertions)]
    trace!("WRITE16 0x{:04x} ({}) to ${:04X}", val, val, addr);

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
          .write(addr - CART_START + 1, bytes[1])?;
      }
      CART_IO_START..=CART_IO_END => {
        self.cart.lazy_dref_mut().io_write(addr, bytes[0])?;
        self.cart.lazy_dref_mut().io_write(addr + 1, bytes[1])?;
      }
      PPU_START..=PPU_END => {
        self.ppu.lazy_dref_mut().write(addr - PPU_START, bytes[0])?;
        self
          .ppu
          .lazy_dref_mut()
          .write(addr - PPU_START + 1, bytes[1])?;
      }
      PPU_IO_START..=PPU_IO_END => {
        self.ppu.lazy_dref_mut().io_write(addr, bytes[0])?;
        self.ppu.lazy_dref_mut().io_write(addr + 1, bytes[1])?;
      }
      ERAM_START..=ERAM_END => {
        self
          .eram
          .lazy_dref_mut()
          .write(addr - ERAM_START, bytes[0])?;
        self
          .eram
          .lazy_dref_mut()
          .write(addr - ERAM_START + 1, bytes[1])?;
      }
      WRAM_START..=WRAM_END => {
        self
          .wram
          .lazy_dref_mut()
          .write(addr - WRAM_START, bytes[0])?;
        self
          .wram
          .lazy_dref_mut()
          .write(addr - WRAM_START + 1, bytes[1])?;
      }
      HRAM_START..=HRAM_END => {
        self
          .hram
          .lazy_dref_mut()
          .write(addr - HRAM_START, bytes[0])?;
        self
          .hram
          .lazy_dref_mut()
          .write(addr - HRAM_START + 1, bytes[1])?;
      }
      TIMER_START..=TIMER_END => {
        self.timer.lazy_dref_mut().write(addr, bytes[0])?;
        self.timer.lazy_dref_mut().write(addr + 1, bytes[1])?;
      }
      IF_ADDR | IE_ADDR => {
        self.ic.lazy_dref_mut().write(addr, bytes[0])?;
        self.ic.lazy_dref_mut().write(addr + 1, bytes[1])?;
      }
      // unsupported
      _ => {
        warn!(
          "Unsupported write16 address: [{:04X}] -> ${:04X}",
          val, addr
        );
      }
    })
  }
}
