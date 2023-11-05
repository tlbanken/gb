//! Main gameboy system module

use crate::gb_err;
use crate::err::{GbResult, GbErrorType, GbError};

#[allow(unused)]
use log::{error, info, warn, debug, trace, LevelFilter};

pub struct Gameboy {
  // TODO
  is_init: bool
}

impl Gameboy {
  pub fn new() -> Gameboy {
    Gameboy {
      is_init: false
    }
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