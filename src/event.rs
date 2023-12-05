//! Events for the Emulator

#[derive(Debug)]
pub enum UserEvent {
  RequestResize(u32, u32),
  EmuPause,
  EmuStep,
  EmuPlay,
  EmuReset,
}
