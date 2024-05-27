// Joypad input for the gameboy emulator

use crate::err::GbResult;

use log::info;

pub enum JoypadInput {
  Up,
  Down,
  Left,
  Right,
  A,
  B,
  Start,
  Select,
}

const BUTTON_A_BIT: u8 = 0;
const BUTTON_B_BIT: u8 = 1;
const BUTTON_START_BIT: u8 = 2;
const BUTTON_SELECT_BIT: u8 = 3;
const DPAD_RIGHT_BIT: u8 = 0;
const DPAD_LEFT_BIT: u8 = 1;
const DPAD_UP_BIT: u8 = 2;
const DPAD_DOWN_BIT: u8 = 3;

pub enum InputBit {
  Button(u8),
  Dpad(u8),
}

impl JoypadInput {
  pub fn as_mask(self) -> InputBit {
    match self {
      JoypadInput::Up => InputBit::Dpad(1 << DPAD_UP_BIT),
      JoypadInput::Down => InputBit::Dpad(1 << DPAD_DOWN_BIT),
      JoypadInput::Left => InputBit::Dpad(1 << DPAD_LEFT_BIT),
      JoypadInput::Right => InputBit::Dpad(1 << DPAD_RIGHT_BIT),
      JoypadInput::A => InputBit::Button(1 << BUTTON_A_BIT),
      JoypadInput::B => InputBit::Button(1 << BUTTON_B_BIT),
      JoypadInput::Start => InputBit::Button(1 << BUTTON_START_BIT),
      JoypadInput::Select => InputBit::Button(1 << BUTTON_SELECT_BIT),
    }
  }
}

pub struct Joypad {
  pub buttons_state: u8,
  pub dpad_state: u8,
  pub button_mode: bool,
  pub dpad_mode: bool,
}

impl Joypad {
  pub fn new() -> Joypad {
    Joypad {
      // 1 means no input
      buttons_state: 0xf,
      dpad_state: 0xf,
      button_mode: false,
      dpad_mode: false,
    }
  }

  pub fn set_input(&mut self, input: JoypadInput) {
    // setting means turning off the bit
    match input.as_mask() {
      InputBit::Button(mask) => self.buttons_state &= !mask,
      InputBit::Dpad(mask) => self.dpad_state &= !mask,
    }
  }

  pub fn clear_input(&mut self, input: JoypadInput) {
    // setting means turning on the bit
    match input.as_mask() {
      InputBit::Button(mask) => self.buttons_state |= mask,
      InputBit::Dpad(mask) => self.dpad_state |= mask,
    }
  }

  pub fn read(&self, _addr: u16) -> GbResult<u8> {
    if self.button_mode {
      Ok(self.buttons_state & 0xf)
    } else if self.dpad_mode {
      Ok(self.dpad_state & 0xf)
    } else {
      Ok(0xf)
    }
  }

  pub fn write(&mut self, _addr: u16, data: u8) -> GbResult<()> {
    self.button_mode = (data >> 5) & 0x1 == 0;
    self.dpad_mode = (data >> 4) & 0x1 == 0;
    Ok(())
  }
}
