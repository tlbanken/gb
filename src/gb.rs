//! Main gameboy system module

use crate::err::{GbError, GbErrorType, GbResult};
use crate::gb_err;

#[allow(unused)]
use log::{debug, error, info, trace, warn, LevelFilter};

pub struct Gameboy {
  // TODO
  is_init: bool,
}

impl Gameboy {
  pub fn new() -> Gameboy {
    Gameboy { is_init: false }
  }

  pub fn init(&mut self) -> GbResult<()> {
    info!("Initializing system");
    self.is_init = true;
    Ok(())
  }

  pub fn run(&mut self) -> GbResult<()> {
    if !self.is_init {
      return gb_err!(GbErrorType::NotInitialized);
    }

    info!("Starting emulation");
    Ok(())
  }
}
