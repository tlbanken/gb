//! Keybindings mapping and management for Gameboy inputs.

use crate::joypad::JoypadInput;
use egui_winit::winit::keyboard::KeyCode;
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
  bindings: HashMap<KeyCode, Action>,
}

impl Default for Keybinds {
  /// Create a new Keybinds instance with the default key configuration.
  fn default() -> Self {
    let mut bindings = HashMap::new();

    // Default Joypad mappings
    bindings.insert(KeyCode::KeyW, Action::Joypad(JoypadInput::Up));
    bindings.insert(KeyCode::KeyS, Action::Joypad(JoypadInput::Down));
    bindings.insert(KeyCode::KeyA, Action::Joypad(JoypadInput::Left));
    bindings.insert(KeyCode::KeyD, Action::Joypad(JoypadInput::Right));
    bindings.insert(KeyCode::KeyJ, Action::Joypad(JoypadInput::A));
    bindings.insert(KeyCode::KeyI, Action::Joypad(JoypadInput::B));
    bindings.insert(KeyCode::Enter, Action::Joypad(JoypadInput::Start));
    bindings.insert(KeyCode::Space, Action::Joypad(JoypadInput::Select));

    Keybinds { bindings }
  }
}

impl Keybinds {
  /// Create a Keybinds configuration from an arbitrary collection/iterator of
  /// bindings.
  #[allow(dead_code)]
  pub fn new<I>(bindings: I) -> Self
  where
    I: IntoIterator<Item = (KeyCode, Action)>,
  {
    Keybinds {
      bindings: bindings.into_iter().collect(),
    }
  }

  /// Translate a keyboard key into its mapped action.
  pub fn translate(&self, key: KeyCode) -> Option<Action> {
    self.bindings.get(&key).copied()
  }

  /// Remap a keyboard key to a new action.
  #[allow(dead_code)]
  pub fn remap(&mut self, key: KeyCode, action: Action) {
    // Optionally clean up any existing key mapped to this action
    self.bindings.retain(|_, v| *v != action);
    self.bindings.insert(key, action);
  }

  /// Remove mapping for a key.
  #[allow(dead_code)]
  pub fn unmap(&mut self, key: KeyCode) {
    self.bindings.remove(&key);
  }

  /// Get the map of all bindings.
  #[allow(dead_code)]
  pub fn get_bindings(&self) -> &HashMap<KeyCode, Action> {
    &self.bindings
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn test_translation() {
    let keybinds = Keybinds::new([
      (KeyCode::KeyW, Action::Joypad(JoypadInput::Up)),
      (KeyCode::Space, Action::Joypad(JoypadInput::Select)),
    ]);
    assert_eq!(
      keybinds.translate(KeyCode::KeyW),
      Some(Action::Joypad(JoypadInput::Up))
    );
    assert_eq!(
      keybinds.translate(KeyCode::Space),
      Some(Action::Joypad(JoypadInput::Select))
    );
    assert_eq!(keybinds.translate(KeyCode::Escape), None);
  }

  #[test]
  fn test_remap() {
    let mut keybinds = Keybinds::new([(KeyCode::KeyW, Action::Joypad(JoypadInput::Up))]);

    // Map Up to ArrowUp
    keybinds.remap(KeyCode::ArrowUp, Action::Joypad(JoypadInput::Up));
    assert_eq!(
      keybinds.translate(KeyCode::ArrowUp),
      Some(Action::Joypad(JoypadInput::Up))
    );
    // Old mapping for Up (W) should be automatically unmapped to prevent duplicate
    // key actions
    assert_eq!(keybinds.translate(KeyCode::KeyW), None);
  }

  #[test]
  fn test_unmap() {
    let mut keybinds = Keybinds::new([(KeyCode::KeyW, Action::Joypad(JoypadInput::Up))]);
    keybinds.unmap(KeyCode::KeyW);
    assert_eq!(keybinds.translate(KeyCode::KeyW), None);
  }
}
