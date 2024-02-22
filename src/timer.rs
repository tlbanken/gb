//! Timer for the Gameboy system.

use crate::err::{GbError, GbErrorType, GbResult};
use crate::int::{Interrupt, Interrupts};
use crate::util::LazyDref;
use crate::{cpu, gb_err};
use log::error;
use std::cell::RefCell;
use std::rc::Rc;

const DIV_ADDR: u16 = 0xff04;
const TIMA_ADDR: u16 = 0xff05;
const TMA_ADDR: u16 = 0xff06;
const TAC_ADDR: u16 = 0xff07;

#[derive(Copy, Clone)]
pub enum ClockRate {
  Div1024 = 0,
  Div16 = 1,
  Div64 = 2,
  Div256 = 3,
}

impl ClockRate {
  pub fn as_div(self) -> u32 {
    match self {
      ClockRate::Div1024 => 1024,
      ClockRate::Div16 => 16,
      ClockRate::Div64 => 64,
      ClockRate::Div256 => 256,
    }
  }
}

impl From<u8> for ClockRate {
  fn from(value: u8) -> Self {
    match value {
      0 => ClockRate::Div1024,
      1 => ClockRate::Div16,
      2 => ClockRate::Div64,
      3 => ClockRate::Div256,
      _ => panic!("Unexpected value for ClockRate: {}", value),
    }
  }
}

#[derive(Copy, Clone)]
pub struct Tac {
  pub enable: bool,
  pub clock_rate: ClockRate,
}

impl From<u8> for Tac {
  fn from(value: u8) -> Self {
    Tac {
      enable: value & 0x4 > 0,
      clock_rate: ClockRate::from(value & 0x3),
    }
  }
}

impl From<Tac> for u8 {
  fn from(value: Tac) -> Self {
    let ie = value.enable as u8;
    (ie << 2) | (value.clock_rate as u8)
  }
}

pub struct Timer {
  // Registers
  /// Divider register
  pub div: u8,
  /// Timer Counter
  pub tima: u8,
  /// Timer Modulo
  pub tma: u8,
  /// Timer Control
  pub tac: Tac,

  /// interrupt controller handle
  ic: Option<Rc<RefCell<Interrupts>>>,

  /// keep track of cpu ticks
  master_clock: u32,
}

impl Timer {
  pub fn new() -> Self {
    Self {
      div: 0,
      tima: 0,
      tma: 0,
      tac: Tac::from(0),
      ic: None,
      master_clock: 0,
    }
  }

  /// Adds a reference to the interrupt controller to the timer
  pub fn connect_ic(&mut self, ic: Rc<RefCell<Interrupts>>) -> GbResult<()> {
    match self.ic {
      None => self.ic = Some(ic),
      Some(_) => return gb_err!(GbErrorType::AlreadyInitialized),
    }
    Ok(())
  }

  /// Step the timer. Will tick as many times as budget allows.
  pub fn step(&mut self, cycle_budget: u32) {
    for cycle in 0..cycle_budget {
      self.step_one();
    }
  }

  fn step_one(&mut self) {
    self.master_clock = self.master_clock.wrapping_add(1);

    // DIV clock rate is always Div256
    if self.master_clock % ClockRate::Div256.as_div() == 0 {
      self.div = self.div.wrapping_add(1);
    }

    // TIMA checks
    if self.tac.enable && self.master_clock % self.tac.clock_rate.as_div() == 0 {
      self.tick();
    }
  }

  /// Increment the TIMA register. If overflow occurs, reset to TMA register
  /// value.
  fn tick(&mut self) {
    self.tima = self.tima.wrapping_add(1);
    if self.tima == 0 {
      self.ic.lazy_dref_mut().raise(Interrupt::Timer);
      self.tima = self.tma;
    }
  }

  pub fn read(&self, addr: u16) -> GbResult<u8> {
    match addr {
      DIV_ADDR => Ok(self.div),
      TIMA_ADDR => Ok(self.tima),
      TMA_ADDR => Ok(self.tma),
      TAC_ADDR => Ok(self.tac.into()),
      _ => {
        error!("Unknown read from addr ${:04X}", addr);
        gb_err!(GbErrorType::OutOfBounds)
      }
    }
  }

  pub fn write(&mut self, addr: u16, data: u8) -> GbResult<()> {
    match addr {
      // writing any value to DIV resets to 0
      DIV_ADDR => self.div = 0,
      TIMA_ADDR => self.tima = data,
      TMA_ADDR => self.tma = data,
      TAC_ADDR => self.tac = Tac::from(data),
      _ => {
        error!("Unknown write: 0x{:02X} -> ${:04X}", data, addr);
        return gb_err!(GbErrorType::OutOfBounds);
      }
    }
    Ok(())
  }
}
