//! Gameboy Emulator entry point

mod bus;
mod err;
mod gb;
mod logger;
mod ram;

use log::LevelFilter;

fn main() {
  println!("~~~ Enter the Gameboy Emulation ~~~");

  let log_level_filter = LevelFilter::max();

  let mut gameboy = gb::Gameboy::new(log_level_filter);
  gameboy.init().unwrap();

  // should never return
  gameboy.run().unwrap();
}
