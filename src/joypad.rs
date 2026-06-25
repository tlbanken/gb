// Joypad input for the gameboy emulator

use crate::err::{GbError, GbErrorType, GbResult};
use crate::gb_err;
use crate::int::{Interrupt, Interrupts};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone, Debug, Copy, PartialEq, Eq)]
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
  ic: Option<Rc<RefCell<Interrupts>>>,
}

impl Joypad {
  pub fn new() -> Joypad {
    Joypad {
      // 1 means no input
      buttons_state: 0xf,
      dpad_state: 0xf,
      button_mode: false,
      dpad_mode: false,
      ic: None,
    }
  }

  pub fn connect_ic(&mut self, ic: Rc<RefCell<Interrupts>>) -> GbResult<()> {
    match self.ic {
      None => self.ic = Some(ic),
      Some(_) => return gb_err!(GbErrorType::AlreadyInitialized),
    }
    Ok(())
  }

  fn get_joyp_nibble(&self) -> u8 {
    let mut inputs = 0xf;
    if self.button_mode {
      inputs &= self.buttons_state;
    }
    if self.dpad_mode {
      inputs &= self.dpad_state;
    }
    inputs & 0xf
  }

  fn check_interrupt(&self, old_nibble: u8) {
    let new_nibble = self.get_joyp_nibble();
    if (old_nibble & !new_nibble) != 0 {
      if let Some(ref ic) = self.ic {
        ic.borrow_mut().raise(Interrupt::Joypad);
      }
    }
  }

  pub fn set_input(&mut self, input: JoypadInput) {
    let old_nibble = self.get_joyp_nibble();
    // setting means turning off the bit
    match input.as_mask() {
      InputBit::Button(mask) => self.buttons_state &= !mask,
      InputBit::Dpad(mask) => self.dpad_state &= !mask,
    }
    self.check_interrupt(old_nibble);
  }

  pub fn clear_input(&mut self, input: JoypadInput) {
    // setting means turning on the bit
    match input.as_mask() {
      InputBit::Button(mask) => self.buttons_state |= mask,
      InputBit::Dpad(mask) => self.dpad_state |= mask,
    }
  }

  pub fn read(&self, _addr: u16) -> GbResult<u8> {
    // Bits 6 and 7 are always 1
    let mut val = 0xc0;

    // Bit 5 is 1 if NOT in button mode, 0 if in button mode
    if !self.button_mode {
      val |= 0x20;
    }
    // Bit 4 is 1 if NOT in dpad mode, 0 if in dpad mode
    if !self.dpad_mode {
      val |= 0x10;
    }

    val |= self.get_joyp_nibble();
    Ok(val)
  }

  pub fn write(&mut self, _addr: u16, data: u8) -> GbResult<()> {
    let old_nibble = self.get_joyp_nibble();
    self.button_mode = (data >> 5) & 0x1 == 0;
    self.dpad_mode = (data >> 4) & 0x1 == 0;
    self.check_interrupt(old_nibble);
    Ok(())
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::bus::IF_ADDR;

  #[test]
  fn test_joypad_read_write() {
    let mut joypad = Joypad::new();

    // Default mode: neither selected. JOYP read should return 0xFF
    assert_eq!(joypad.read(0xff00).unwrap(), 0xff);

    // Select buttons (write bit 5 = 0, bit 4 = 1 => data = 0x10)
    // Writing 0x10: bit 5 is 0 (button mode = true), bit 4 is 1 (dpad mode = false)
    joypad.write(0xff00, 0x10).unwrap();
    // No button pressed, so lower nibble is 0xf. Value should be 0xd0 | 0xf = 0xdf
    assert_eq!(joypad.read(0xff00).unwrap(), 0xdf);

    // Press button A (bit 0 of buttons_state becomes 0)
    joypad.set_input(JoypadInput::A);
    // Now lower nibble is 0xe. Read value should be 0xd0 | 0xe = 0xde
    assert_eq!(joypad.read(0xff00).unwrap(), 0xde);

    // Release button A
    joypad.clear_input(JoypadInput::A);
    assert_eq!(joypad.read(0xff00).unwrap(), 0xdf);
  }

  #[test]
  fn test_joypad_interrupt() {
    let ic = Rc::new(RefCell::new(Interrupts::new()));
    let mut joypad = Joypad::new();
    joypad.connect_ic(ic.clone()).unwrap();

    // Select buttons mode
    joypad.write(0xff00, 0x10).unwrap();

    // Check that IF has no joypad interrupt requested
    assert_eq!(ic.borrow().read(IF_ADDR).unwrap() & (Interrupt::Joypad as u8), 0);

    // Press button A: transitions button selection from 1 to 0 (High to Low)
    joypad.set_input(JoypadInput::A);

    // Check that Joypad interrupt is now requested
    assert_ne!(ic.borrow().read(IF_ADDR).unwrap() & (Interrupt::Joypad as u8), 0);
  }
}
