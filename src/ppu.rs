//! PPU for the Gameboy emulator.

use crate::err::GbResult;

// 8Kb of video memory
const VRAM_SIZE: usize = 8 * 1024;

enum AddressMode {
  // use $8000 as the base pointer with unsigned offsets and tile data ranges from $8000-$8fff
  X8000,
  // use $9000 as the base pointer with signed offsets. Tile data ranges from $8800-$97ff
  X8800,
}

pub struct Ppu {
  pub vram: Vec<u8>,
}

impl Ppu {
  pub fn new() -> Ppu {
    Ppu {
      vram: vec![0; VRAM_SIZE],
    }
  }

  pub fn step(&mut self) -> GbResult<u8> {
    // TODO: this should probably render a row?
    todo!()
  }

  pub fn read(&self, addr: u16) -> GbResult<u8> {
    Ok(self.vram[addr as usize])
  }

  pub fn write(&mut self, addr: u16, data: u8) -> GbResult<()> {
    self.vram[addr as usize] = data;
    Ok(())
  }
}
