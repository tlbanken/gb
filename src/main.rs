//! Gameboy Emulator entry point

mod bus;
mod cart;
mod cpu;
mod err;
mod gb;
mod logger;
mod ram;
mod util;

use log::LevelFilter;

fn main() {
  println!("~~~ Enter the Gameboy Emulation ~~~");

  let log_level_filter = LevelFilter::max();

  // initialize hardware
  let mut gameboy = gb::Gameboy::new(log_level_filter);
  gameboy.init().unwrap();

  // start the emulation
  gameboy.run().unwrap();
}
