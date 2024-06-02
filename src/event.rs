//! Events for the Emulator

use std::path::PathBuf;

#[derive(Debug)]
pub enum UserEvent {
  RequestResize(u32, u32),
  EmuPause,
  EmuStep,
  EmuPlay,
  EmuReset(Option<PathBuf>),
  RequestRender,
}
