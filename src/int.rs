//! Interrupts for the Gameboy.

use crate::cpu::Cpu;
use crate::err::{GbError, GbErrorType, GbResult};
use crate::gb_err;
use crate::util::LazyDref;
use log::error;
use std::cell::RefCell;
use std::rc::Rc;

const IE_ADDR: u16 = 0xffff;
const IF_ADDR: u16 = 0xff0f;

#[derive(Copy, Clone)]
pub enum Interrupt {
  Vblank = 1 << 0,
  Lcd = 1 << 1,
  Timer = 1 << 2,
  Serial = 1 << 3,
  Joypad = 1 << 4,
}

impl TryFrom<u8> for Interrupt {
  type Error = GbErrorType;
  fn try_from(value: u8) -> Result<Self, Self::Error> {
    match value {
      value if value == Interrupt::Vblank as u8 => Ok(Interrupt::Vblank),
      value if value == Interrupt::Lcd as u8 => Ok(Interrupt::Lcd),
      value if value == Interrupt::Timer as u8 => Ok(Interrupt::Timer),
      value if value == Interrupt::Serial as u8 => Ok(Interrupt::Serial),
      value if value == Interrupt::Joypad as u8 => Ok(Interrupt::Joypad),
      _ => Err(GbErrorType::BadValue),
    }
  }
}

pub struct Interrupts {
  // regs
  /// Interrupt Enable
  ie: u8,
  /// Interrupt Flag
  iflag: u8,

  cpu: Option<Rc<RefCell<Cpu>>>,
}

impl Interrupts {
  pub fn new() -> Interrupts {
    Interrupts {
      cpu: None,
      ie: 0,
      iflag: 0,
    }
  }

  pub fn connect_cpu(&mut self, cpu: Rc<RefCell<Cpu>>) -> GbResult<()> {
    match self.cpu {
      Some(_) => return gb_err!(GbErrorType::AlreadyInitialized),
      None => self.cpu = Some(cpu),
    }
    Ok(())
  }

  pub fn raise(&mut self, interrupt: Interrupt) {
    self.iflag |= interrupt as u8;
  }

  pub fn step(&self) {
    // TODO: collect interrupts only when needed
    for interrupt in self.collect_interrupts() {
      if interrupt as u8 & self.ie > 0 {
        self.cpu.lazy_dref_mut().interrupt(interrupt);
        // only handle one interrupt
        return;
      }
    }
  }

  pub fn read(&self, addr: u16) -> GbResult<u8> {
    match addr {
      IE_ADDR => Ok(self.ie),
      IF_ADDR => Ok(self.iflag),
      _ => {
        error!("Unknown read from addr ${:04X}", addr);
        gb_err!(GbErrorType::OutOfBounds)
      }
    }
  }

  pub fn write(&mut self, addr: u16, data: u8) -> GbResult<()> {
    match addr {
      IE_ADDR => self.ie = data,
      IF_ADDR => self.iflag = data,
      _ => {
        error!("Unknown write: 0x{:02X} -> ${:04X}", data, addr);
        return gb_err!(GbErrorType::OutOfBounds);
      }
    }
    Ok(())
  }

  fn collect_interrupts(&self) -> Vec<Interrupt> {
    let mut ints = Vec::new();
    for bit in 0..7 {
      if (1 << bit) & self.iflag > 0 {
        ints.push(Interrupt::try_from(1 << bit).unwrap());
      }
    }
    ints
  }
}
