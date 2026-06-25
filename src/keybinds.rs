//! Keybindings mapping and management for Gameboy inputs.

use crate::joypad::JoypadInput;
use egui_winit::winit::event::VirtualKeyCode;
use std::collections::HashMap;

/// Represents an action triggered by a key press or release.
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum Action {
  Joypad(JoypadInput),
  #[allow(dead_code)]
  Control(EmuControl),
}

/// Extensible emulation control features.
#[allow(dead_code)]
#[derive(Clone, Debug, Copy, PartialEq, Eq)]
pub enum EmuControl {
  Pause,
  Reset,
}

/// Keybindings configuration.
#[derive(Clone, Debug)]
pub struct Keybinds {
  bindings: HashMap<VirtualKeyCode, Action>,
}

impl Default for Keybinds {
  /// Create a new Keybinds instance with the default key configuration.
  fn default() -> Self {
    let mut bindings = HashMap::new();

    // Default Joypad mappings
    bindings.insert(VirtualKeyCode::W, Action::Joypad(JoypadInput::Up));
    bindings.insert(VirtualKeyCode::S, Action::Joypad(JoypadInput::Down));
    bindings.insert(VirtualKeyCode::A, Action::Joypad(JoypadInput::Left));
    bindings.insert(VirtualKeyCode::D, Action::Joypad(JoypadInput::Right));
    bindings.insert(VirtualKeyCode::J, Action::Joypad(JoypadInput::A));
    bindings.insert(VirtualKeyCode::I, Action::Joypad(JoypadInput::B));
    bindings.insert(VirtualKeyCode::Return, Action::Joypad(JoypadInput::Start));
    bindings.insert(VirtualKeyCode::Space, Action::Joypad(JoypadInput::Select));

    Keybinds { bindings }
  }
}

impl Keybinds {
  /// Create a Keybinds configuration from an arbitrary collection/iterator of
  /// bindings.
  #[allow(dead_code)]
  pub fn new<I>(bindings: I) -> Self
  where
    I: IntoIterator<Item = (VirtualKeyCode, Action)>,
  {
    Keybinds {
      bindings: bindings.into_iter().collect(),
    }
  }

  /// Translate a keyboard key into its mapped action.
  pub fn translate(&self, key: VirtualKeyCode) -> Option<Action> {
    self.bindings.get(&key).copied()
  }

  /// Remap a keyboard key to a new action.
  #[allow(dead_code)]
  pub fn remap(&mut self, key: VirtualKeyCode, action: Action) {
    // Optionally clean up any existing key mapped to this action
    self.bindings.retain(|_, v| *v != action);
    self.bindings.insert(key, action);
  }

  /// Remove mapping for a key.
  #[allow(dead_code)]
  pub fn unmap(&mut self, key: VirtualKeyCode) {
    self.bindings.remove(&key);
  }

  /// Get the map of all bindings.
  #[allow(dead_code)]
  pub fn get_bindings(&self) -> &HashMap<VirtualKeyCode, Action> {
    &self.bindings
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_translation() {
    let keybinds = Keybinds::new([
      (VirtualKeyCode::W, Action::Joypad(JoypadInput::Up)),
      (VirtualKeyCode::Space, Action::Joypad(JoypadInput::Select)),
    ]);
    assert_eq!(
      keybinds.translate(VirtualKeyCode::W),
      Some(Action::Joypad(JoypadInput::Up))
    );
    assert_eq!(
      keybinds.translate(VirtualKeyCode::Space),
      Some(Action::Joypad(JoypadInput::Select))
    );
    assert_eq!(keybinds.translate(VirtualKeyCode::Escape), None);
  }

  #[test]
  fn test_remap() {
    let mut keybinds = Keybinds::new([(VirtualKeyCode::W, Action::Joypad(JoypadInput::Up))]);

    // Map Up to UpArrow
    keybinds.remap(VirtualKeyCode::Up, Action::Joypad(JoypadInput::Up));
    assert_eq!(
      keybinds.translate(VirtualKeyCode::Up),
      Some(Action::Joypad(JoypadInput::Up))
    );
    // Old mapping for Up (W) should be automatically unmapped to prevent duplicate
    // key actions
    assert_eq!(keybinds.translate(VirtualKeyCode::W), None);
  }

  #[test]
  fn test_unmap() {
    let mut keybinds = Keybinds::new([(VirtualKeyCode::W, Action::Joypad(JoypadInput::Up))]);
    keybinds.unmap(VirtualKeyCode::W);
    assert_eq!(keybinds.translate(VirtualKeyCode::W), None);
  }
}
