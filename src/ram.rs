//! Ram space for the gameboy emulator. There are two segments of ram: The
//! External Ram and the Working ram. The external ram is held within the
//! cartridge on a real system. Often, this would also be battery backed to
//! allow saving. The emulator will save a ram file of the same name as the
//! given rom to mimic this. The working ram is held internally and is lost on a
//! power cycle.

use log::{debug, info};

use crate::{
  err::{GbError, GbErrorType, GbResult},
  gb_err,
};

pub struct Ram {
  data: Vec<u8>,
}

impl Ram {
  pub fn new(size: u16) -> Ram {
    debug!("Creating ram with size {} bytes", size);
    Ram {
      data: vec![0u8; size as usize],
    }
  }

  pub fn read(&self, addr: u16) -> GbResult<u8> {
    Ok(self.data[addr as usize])
  }

  pub fn write(&mut self, addr: u16, val: u8) -> GbResult<()> {
    self.data[addr as usize] = val;
    Ok(())
  }

  pub fn from_file(path: &'static str) -> GbResult<Ram> {
    unimplemented!();
  }

  pub fn dump(path: &'static str) -> GbResult<()> {
    unimplemented!();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn test_ram_rw() {
    const RAM_SIZE: u16 = 8 * 1024;
    let mut ram = Ram::new(RAM_SIZE);
    for i in 0..RAM_SIZE {
      ram.write(i, i as u8).unwrap();
      let val = ram.read(i).unwrap();
      assert_eq!(val, i as u8);
    }
  }
}
