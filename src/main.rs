//! Gameboy Emulator entry point

mod bus;
mod cart;
mod cpu;
mod dasm;
mod err;
mod event;
mod gb;
mod logger;
mod ppu;
mod ram;
mod screen;
mod state;
mod tick_counter;
mod ui;
mod util;
mod video;

use log::LevelFilter;

fn main() {
  println!("~~~ Enter the Gameboy Emulation ~~~");

  // set the max through compile time config in Cargo.toml
  let log_level_filter = LevelFilter::Info;

  // initialize hardware
  let mut gameboy = gb::Gameboy::new(log_level_filter);

  // start the emulation
  gameboy.run().unwrap();
}
