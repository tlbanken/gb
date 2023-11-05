//! Logging support for the gameboy emulator.

use colored::*;
use log::{LevelFilter, Log, Metadata, Record};

/// Logging implementation for the Log trait.
pub struct Logger {
  level_filter: LevelFilter,
}

impl Logger {
  /// Default function to be used in const time use cases.
  pub const fn const_default() -> Self {
    Logger {
      level_filter: LevelFilter::Off,
    }
  }

  /// Create a new PsxLogger with the provided level filter.
  pub fn new(level: LevelFilter) -> Self {
    let logger = Logger {
      level_filter: level,
    };
    logger
  }
}

impl Log for Logger {
  fn enabled(&self, metadata: &Metadata<'_>) -> bool {
    metadata.level() <= self.level_filter
  }

  fn log(&self, record: &Record) {
    if self.enabled(record.metadata()) {
      let colored_level = match record.level() {
        log::Level::Error => format!("{}", record.level()).red(),
        log::Level::Warn => format!("{}", record.level()).yellow(),
        log::Level::Info => format!("{}", record.level()).cyan(),
        log::Level::Debug => format!("{}", record.level()).normal(),
        log::Level::Trace => format!("{}", record.level()).normal(),
      };
      println!(
        "[{:5}] [{}] {}",
        colored_level,
        record.metadata().target(),
        record.args()
      );
    }
  }

  fn flush(&self) {}
}
