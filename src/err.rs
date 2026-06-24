//! Errors and Result types for the gameboy emulator.


#[macro_export]
macro_rules! gb_err {
  ( $x:expr ) => {
    Err(GbError::new($x, file!(), line!()))
  };
}

pub type GbResult<T> = Result<T, GbError>;

/// Error type for the gameboy emulator
#[allow(dead_code)]
#[derive(Debug)]
pub struct GbError {
  error: GbErrorType,
  line: u32,
  file: &'static str,
}

impl GbError {
  pub fn new(error: GbErrorType, file: &'static str, line: u32) -> GbError {
    GbError { error, line, file }
  }
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum GbErrorType {
  NotInitialized,
  AlreadyInitialized,
  OutOfBounds,
  InvalidCpuInstruction,
  FileError,
  BadValue,
  Unsupported,
}
