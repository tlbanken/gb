//! Ram space for the gameboy emulator. There are two segments of ram: The
//! External Ram and the Working ram. The external ram is held within the
//! cartridge on a real system. Often, this would also be battery backed to
//! allow saving. The emulator will save a ram file of the same name as the
//! given rom to mimic this. The working ram is held internally and is lost on a
//! power cycle.

use crate::err::GbResult;

pub struct Ram {
  data: Vec<u8>,
}

impl Ram {
  pub fn new(size: u16) -> Ram {
    // TODO: modify path for creating save file
    Ram {
      data: vec![0u8; size as usize],
    }
  }

  pub fn read(&self, addr: u16) -> GbResult<u8> {
    unimplemented!();
  }

  pub fn write(&self, addr: u16) -> GbResult<()> {
    unimplemented!();
  }

  pub fn from_file(path: &'static str) -> GbResult<Ram> {
    unimplemented!();
  }

  pub fn dump(path: &'static str) -> GbResult<()> {
    unimplemented!();
  }
}
