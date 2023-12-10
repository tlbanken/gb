//! Gameboy state

use std::{cell::RefCell, rc::Rc};

use crate::screen::Screen;
use crate::tick_counter::TickCounter;
use crate::{bus::Bus, cart::Cartridge, cpu, cpu::Cpu, err::GbResult, ppu::Ppu, ram::Ram};

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
  pub flow: EmuFlow,
  pub cycle: TickCounter,
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
      flow: EmuFlow::new(paused),
      cycle: TickCounter::new(CLOCK_RATE_ALPHA),
      clock_rate: 0.0,
    }
  }

  pub fn init(&mut self, screen: Rc<RefCell<Screen>>) -> GbResult<()> {
    // TODO: load cartridge

    // connect PPU to screen
    self.ppu.borrow_mut().connect_screen(screen)?;

    // connect Bus to memory
    self.bus.borrow_mut().connect_eram(self.eram.clone())?;
    self.bus.borrow_mut().connect_wram(self.wram.clone())?;
    self.bus.borrow_mut().connect_hram(self.hram.clone())?;
    self.bus.borrow_mut().connect_cartridge(self.cart.clone())?;
    self.bus.borrow_mut().connect_ppu(self.ppu.clone())?;

    // connect modules to bus
    self.cpu.borrow_mut().connect_bus(self.bus.clone())?;

    Ok(())
  }

  pub fn step(&mut self) -> GbResult<()> {
    if self.flow.paused && !self.flow.step {
      return Ok(());
    }

    if self.flow.step {
      self.step_one()?;
    } else {
      self.step_chunk()?;
    }

    self.flow.step = false;
    Ok(())
  }

  fn step_chunk(&mut self) -> GbResult<()> {
    // if we are running too fast, skip
    let clock_rate = self.cycle.tps() * 4.0;
    if clock_rate > cpu::CLOCK_RATE {
      return Ok(());
    }
    // only show clock rate when we are doing work
    self.clock_rate = clock_rate;

    // 70,224 clocks to draw a complete screen.
    // instructions about 4 clocks on average
    // 256 rows (including hidden rows)
    // so num instrs per row is 70,224 / 4 / 256
    const INSTR_PER_ROW: u32 = 68;
    let mut cpu = self.cpu.borrow_mut();
    for _ in 0..INSTR_PER_ROW {
      cpu.step()?;
      self.cycle.tick();
    }
    // TODO: step ppu

    Ok(())
  }

  fn step_one(&mut self) -> GbResult<()> {
    // TODO
    self.clock_rate = 0.0;
    self.cpu.borrow_mut().step()?;
    self.cycle.tick();
    Ok(())
  }
}
