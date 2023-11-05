//! Gameboy Emulator entry point

mod err;
mod gb;
mod logger;

#[allow(unused)]
use log::{debug, error, info, trace, warn, LevelFilter};
use logger::Logger;

static mut LOGGER: Logger = Logger::const_default();

fn main() {
  println!("~~~ Enter the Gameboy Emulation ~~~");

  init_logging(LevelFilter::max());

  let mut gameboy = gb::Gameboy::new();
  gameboy.init().unwrap();

  // should never return
  gameboy.run().unwrap();
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
