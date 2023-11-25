//! Gameboy Emulator entry point
#![feature(const_fn_floating_point_arithmetic)]

mod bus;
mod cart;
mod cpu;
mod err;
mod gb;
mod geometry;
mod logger;
mod ram;
mod screen;
mod util;
mod video;

use log::LevelFilter;

#[tokio::main]
async fn main() {
  println!("~~~ Enter the Gameboy Emulation ~~~");

  // set the max through compile time config in Cargo.toml
  let log_level_filter = LevelFilter::max();

  // initialize hardware
  let mut gameboy = gb::Gameboy::new(log_level_filter);
  gameboy.init().unwrap();

  // start the emulation
  gameboy.run().await.unwrap();
}
