//! Gameboy state

use std::{cell::RefCell, rc::Rc};

use crate::{bus::Bus, cart::Cartridge, cpu::Cpu, err::GbResult, ppu::Ppu, ram::Ram};

pub struct EmuFlow {
  pub paused: bool,
  pub step: bool,
}

impl EmuFlow {
  pub fn new(paused: bool) -> EmuFlow {
    EmuFlow {
      paused,
      step: false,
    }
  }
}

pub struct GbState {
  pub bus: Rc<RefCell<Bus>>,
  pub eram: Rc<RefCell<Ram>>,
  pub wram: Rc<RefCell<Ram>>,
  pub hram: Rc<RefCell<Ram>>,
  pub cart: Rc<RefCell<Cartridge>>,
  pub cpu: Rc<RefCell<Cpu>>,
  pub gpu: Rc<RefCell<Ppu>>,
  pub flow: EmuFlow,
  // TODO: maybe keep event proxy for signaling gpu draws
}

impl GbState {
  pub fn new(paused: bool) -> GbState {
    GbState {
      bus: Rc::new(RefCell::new(Bus::new())),
      eram: Rc::new(RefCell::new(Ram::new(8 * 1024))),
      wram: Rc::new(RefCell::new(Ram::new(8 * 1024))),
      hram: Rc::new(RefCell::new(Ram::new(127))),
      cart: Rc::new(RefCell::new(Cartridge::new())),
      cpu: Rc::new(RefCell::new(Cpu::new())),
      gpu: Rc::new(RefCell::new(Ppu::new())),
      flow: EmuFlow::new(paused),
    }
  }

  pub fn init(&mut self) -> GbResult<()> {
    // TODO: load cartridge

    // connect Bus to memory
    self.bus.borrow_mut().connect_eram(self.eram.clone())?;
    self.bus.borrow_mut().connect_wram(self.wram.clone())?;
    self.bus.borrow_mut().connect_hram(self.hram.clone())?;
    self.bus.borrow_mut().connect_cartridge(self.cart.clone())?;
    self.bus.borrow_mut().connect_gpu(self.gpu.clone())?;

    // connect modules to bus
    self.cpu.borrow_mut().connect_bus(self.bus.clone())?;

    Ok(())
  }

  pub fn step(&mut self) -> GbResult<()> {
    if self.flow.paused && !self.flow.step {
      return Ok(());
    }

    self.cpu.borrow_mut().step()?;

    self.flow.step = false;
    Ok(())
  }
}
