//! Main gameboy system module

use std::cell::RefCell;
use std::rc::Rc;

use crate::bus::*;
use crate::cart::Cartridge;
use crate::cpu::Cpu;
use crate::err::{GbError, GbErrorType, GbResult};
use crate::gb_err;
use crate::logger::Logger;
use crate::ram::*;

#[allow(unused)]
use log::{debug, error, info, trace, warn, LevelFilter};

static mut LOGGER: Logger = Logger::const_default();

pub struct Gameboy {
  is_init: bool,
  bus: Rc<RefCell<Bus>>,
  eram: Rc<RefCell<Ram>>,
  wram: Rc<RefCell<Ram>>,
  cart: Rc<RefCell<Cartridge>>,
  cpu: Cpu,
}

impl Gameboy {
  pub fn new(level_filter: LevelFilter) -> Gameboy {
    init_logging(level_filter);
    Gameboy {
      is_init: false,
      bus: Rc::new(RefCell::new(Bus::new())),
      eram: Rc::new(RefCell::new(Ram::new(8 * 1024))),
      wram: Rc::new(RefCell::new(Ram::new(8 * 1024))),
      cart: Rc::new(RefCell::new(Cartridge::new())),
      cpu: Cpu::new(),
    }
  }

  pub fn init(&mut self) -> GbResult<()> {
    info!("Initializing system");

    // TODO: load cartridge

    // connect Bus to memory
    self.bus.borrow_mut().connect_eram(self.eram.clone())?;
    self.bus.borrow_mut().connect_wram(self.wram.clone())?;
    self.bus.borrow_mut().connect_cartridge(self.cart.clone())?;

    // connect modules to bus
    self.cpu.connect_bus(self.bus.clone())?;

    self.is_init = true;
    Ok(())
  }

  pub fn run(&mut self) -> GbResult<()> {
    if !self.is_init {
      return gb_err!(GbErrorType::NotInitialized);
    }

    info!("Starting emulation");
    loop {
      self.step()?;
    }
  }

  fn step(&mut self) -> GbResult<()> {
    self.cpu.step()?;
    Ok(())
  }
}

// Initialize logging and set the level filter
fn init_logging(level_filter: LevelFilter) {
  log::set_max_level(level_filter);
  unsafe {
    LOGGER = Logger::new(level_filter);
    match log::set_logger(&LOGGER) {
      Ok(()) => {}
      Err(msg) => panic!("Failed to initialize logging: {}", msg),
    }
  }
  error!("Log Level ERROR Enabled!");
  warn!("Log Level WARN Enabled!");
  info!("Log Level INFO Enabled!");
  debug!("Log Level DEBUG Enabled!");
  trace!("Log Level TRACE Enabled!");
}
