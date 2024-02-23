//! Gameboy state

use std::{cell::RefCell, rc::Rc};

use crate::int::Interrupts;
use crate::screen::Screen;
use crate::tick_counter::TickCounter;
use crate::timer::Timer;
use crate::{bus::Bus, cart::Cartridge, cpu, cpu::Cpu, err::GbResult, ppu::Ppu, ram::Ram};

/// Alpha used when calculating the rolling average
const CLOCK_RATE_ALPHA: f32 = 0.999;

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
  pub ppu: Rc<RefCell<Ppu>>,
  pub ic: Rc<RefCell<Interrupts>>,
  pub timer: Rc<RefCell<Timer>>,
  pub flow: EmuFlow,
  pub cycles: TickCounter,
  pub clock_rate: f32,
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
      ppu: Rc::new(RefCell::new(Ppu::new())),
      ic: Rc::new(RefCell::new(Interrupts::new())),
      timer: Rc::new(RefCell::new(Timer::new())),
      flow: EmuFlow::new(paused),
      cycles: TickCounter::new(CLOCK_RATE_ALPHA),
      clock_rate: 0.0,
    }
  }

  pub fn init(&mut self, screen: Rc<RefCell<Screen>>) -> GbResult<()> {
    // TODO: load cartridge

    // connect PPU to screen
    self.ppu.borrow_mut().connect_screen(screen)?;

    // connect interrupts to cpu
    self.ic.borrow_mut().connect_cpu(self.cpu.clone())?;

    // connect Bus to memory
    self.bus.borrow_mut().connect_eram(self.eram.clone())?;
    self.bus.borrow_mut().connect_wram(self.wram.clone())?;
    self.bus.borrow_mut().connect_hram(self.hram.clone())?;
    self.bus.borrow_mut().connect_cartridge(self.cart.clone())?;
    self.bus.borrow_mut().connect_ppu(self.ppu.clone())?;
    self.bus.borrow_mut().connect_ic(self.ic.clone())?;
    self.bus.borrow_mut().connect_timer(self.timer.clone())?;

    // connect modules to bus
    self.cpu.borrow_mut().connect_bus(self.bus.clone())?;

    // connect modules to interrupt controller
    self.timer.borrow_mut().connect_ic(self.ic.clone())?;
    self.ppu.borrow_mut().connect_ic(self.ic.clone())?;

    Ok(())
  }

  pub fn step(&mut self) -> GbResult<()> {
    if self.flow.paused && !self.flow.step {
      self.clock_rate = 0.0;
      return Ok(());
    }

    if self.flow.step {
      self.clock_rate = 0.0;
      self.step_one()?;
    } else {
      self.step_chunk()?;
    }

    self.flow.step = false;
    Ok(())
  }

  fn step_chunk(&mut self) -> GbResult<()> {
    // if we are running too fast, skip
    let clock_rate = self.cycles.tps();
    if clock_rate > cpu::CLOCK_RATE {
      return Ok(());
    }
    // only show clock rate when we are doing work
    self.clock_rate = clock_rate;

    // how many steps in a chunk
    const CHUNK_SIZE: u32 = 80;

    for _ in 0..CHUNK_SIZE {
      self.step_one()?;
    }

    Ok(())
  }

  #[inline]
  fn step_one(&mut self) -> GbResult<()> {
    let cycle_budget = self.cpu.borrow_mut().step()?;
    for _ in 0..cycle_budget {
      self.cycles.tick();
    }
    self.ppu.borrow_mut().step(cycle_budget)?;
    self.ic.borrow_mut().step();
    self.timer.borrow_mut().step(cycle_budget);
    Ok(())
  }
}
