//! Gameboy state

use std::{cell::RefCell, rc::Rc};

use crate::{bus::Bus, cart::Cartridge, cpu::Cpu, err::GbResult, ram::Ram};

pub struct GbState {
  pub bus: Rc<RefCell<Bus>>,
  pub eram: Rc<RefCell<Ram>>,
  pub wram: Rc<RefCell<Ram>>,
  pub cart: Rc<RefCell<Cartridge>>,
  pub cpu: Rc<RefCell<Cpu>>,
  // TODO: maybe keep event proxy for signaling gpu draws
}

impl GbState {
  pub fn init(&mut self) -> GbResult<()> {
    // TODO: load cartridge

    // connect Bus to memory
    self.bus.borrow_mut().connect_eram(self.eram.clone())?;
    self.bus.borrow_mut().connect_wram(self.wram.clone())?;
    self.bus.borrow_mut().connect_cartridge(self.cart.clone())?;

    // connect modules to bus
    self.cpu.borrow_mut().connect_bus(self.bus.clone())?;

    Ok(())
  }

  pub fn step(&mut self) -> GbResult<()> {
    self.cpu.borrow_mut().step()?;
    Ok(())
  }
}
