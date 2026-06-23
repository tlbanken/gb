//! Mbc3 mapper

use crate::cart::mapper::Mapper;
use crate::cart::{
  ERAM_END, ERAM_START, RAM_BANK_SIZE, ROM0_END, ROM0_START, ROM1_END, ROM1_START, ROM_BANK_SIZE,
};
use crate::err::{GbError, GbErrorType, GbResult};
use crate::gb_err;
use log::{error, warn};

// registers
const RAM_TIMER_ENABLE_START: u16 = 0x0000;
const RAM_TIMER_ENABLE_END: u16 = 0x1fff;
const ROM_BANK_NUM_START: u16 = 0x2000;
const ROM_BANK_NUM_END: u16 = 0x3fff;
const RAM_BANK_RTC_SELECT_START: u16 = 0x4000;
const RAM_BANK_RTC_SELECT_END: u16 = 0x5fff;
const LATCH_CLOCK_START: u16 = 0x6000;
const LATCH_CLOCK_END: u16 = 0x7fff;

enum RamRtcSelect {
  RamBank(usize),
  RtcS,
  RtcM,
  RtcH,
  RtcDL,
  RtcDH,
  Invalid,
}

impl From<u8> for RamRtcSelect {
  fn from(val: u8) -> RamRtcSelect {
    match val {
      0x00..=0x07 => RamRtcSelect::RamBank(val as usize),
      0x08 => RamRtcSelect::RtcS,
      0x09 => RamRtcSelect::RtcM,
      0x0A => RamRtcSelect::RtcH,
      0x0B => RamRtcSelect::RtcDL,
      0x0C => RamRtcSelect::RtcDH,
      _ => {
        warn!("Invalid Ram/Rtc selection: {val}");
        RamRtcSelect::Invalid
      }
    }
  }
}

/// real time clock register
#[derive(Default, Copy, Clone)]
struct Rtc {
  // sec
  pub s: u8,
  // min
  pub m: u8,
  // hour
  pub h: u8,
  // day low
  pub dl: u8,
  // day hi
  //   Bit 0  Most Sig bit of day counter (bit 8)
  //   Bit 6  Halt (0=Active, 1=Stop Timer)
  //   Bit 7  Day Counter Carry Bit (1=overflow)
  pub dh: u8,
  pub halt: bool,
  pub day_carry: bool,
}

pub struct Mbc3 {
  rom: Vec<[u8; ROM_BANK_SIZE]>,
  ram: Vec<[u8; RAM_BANK_SIZE]>,
  ram_and_timer_enabled: bool,
  rom_bank: usize,
  ram_rtc_select: RamRtcSelect,
  rtc: Rtc,
  latched_rtc: Rtc,
  last_latch_val: u8,
}

impl Mbc3 {
  pub fn new(rom: Vec<u8>, num_rom_banks: usize, num_ram_banks: usize) -> Self {
    // set up rom
    let mut rom_banks: Vec<[u8; ROM_BANK_SIZE]> = Vec::new();
    for bank in 0..num_rom_banks {
      let bank_offset = bank * ROM_BANK_SIZE;
      let bank_range = bank_offset..(bank_offset + ROM_BANK_SIZE);
      rom_banks.push([0u8; ROM_BANK_SIZE]);
      rom_banks[bank].copy_from_slice(&rom[bank_range]);
    }

    // set up ram
    let mut ram_banks: Vec<[u8; RAM_BANK_SIZE]> = Vec::new();
    for _bank in 0..num_ram_banks {
      ram_banks.push([0u8; RAM_BANK_SIZE]);
    }

    Self {
      rom: rom_banks,
      ram: ram_banks,
      ram_and_timer_enabled: false,
      rom_bank: 1,
      ram_rtc_select: RamRtcSelect::RamBank(0),
      rtc: Rtc::default(),
      latched_rtc: Rtc::default(),
      last_latch_val: 0xff,
    }
  }

  // read from one of the rtc registers
  pub fn read_rtc(&self) -> GbResult<u8> {
    match self.ram_rtc_select {
      RamRtcSelect::RtcS => Ok(self.latched_rtc.s),
      RamRtcSelect::RtcM => Ok(self.latched_rtc.m),
      RamRtcSelect::RtcH => Ok(self.latched_rtc.h),
      RamRtcSelect::RtcDL => Ok(self.latched_rtc.dl),
      RamRtcSelect::RtcDH => Ok(self.latched_rtc.dh),
      _ => Ok(0xff),
    }
  }

  // write to one of the rtc registers
  pub fn write_rtc(&mut self, val: u8) -> GbResult<()> {
    match self.ram_rtc_select {
      RamRtcSelect::RtcS => self.rtc.s = val & 0x3f,
      RamRtcSelect::RtcM => self.rtc.m = val & 0x3f,
      RamRtcSelect::RtcH => self.rtc.h = val & 0x1f,
      RamRtcSelect::RtcDL => self.rtc.dl = val,
      RamRtcSelect::RtcDH => {
        self.rtc.dh = val & 0xc1;
        self.rtc.halt = (val & 0x40) != 0;
        self.rtc.day_carry = (val & 0x80) != 0;
      }
      _ => {}
    }
    Ok(())
  }
}

impl Mapper for Mbc3 {
  fn read(&self, addr: u16) -> GbResult<u8> {
    let rel_rom_addr = addr as usize % ROM_BANK_SIZE;
    let rel_ram_addr = addr as usize % RAM_BANK_SIZE;
    match addr {
      ROM0_START..=ROM0_END => Ok(self.rom[0][rel_rom_addr]),
      ROM1_START..=ROM1_END => {
        let bank = self.rom_bank % self.rom.len();
        Ok(self.rom[bank][rel_rom_addr])
      }
      ERAM_START..=ERAM_END => {
        if self.ram_and_timer_enabled {
          match self.ram_rtc_select {
            RamRtcSelect::RamBank(bank) => {
              if !self.ram.is_empty() {
                let bank = bank % self.ram.len();
                Ok(self.ram[bank][rel_ram_addr])
              } else {
                Ok(0xff)
              }
            }
            _ => self.read_rtc(),
          }
        } else {
          warn!(
            "Reading ERAM @0x{:04x} while disabled! Returning 0xff...",
            addr
          );
          Ok(0xff)
        }
      }
      _ => {
        error!("Invalid Read ${:04X}", addr);
        gb_err!(GbErrorType::OutOfBounds)
      }
    }
  }

  fn write(&mut self, addr: u16, val: u8) -> GbResult<()> {
    let rel_ram_addr = addr as usize % RAM_BANK_SIZE;
    match addr {
      RAM_TIMER_ENABLE_START..=RAM_TIMER_ENABLE_END => {
        // write $XA to enable ram/timer
        self.ram_and_timer_enabled = val & 0x0f == 0xa;
      }
      ROM_BANK_NUM_START..=ROM_BANK_NUM_END => {
        // setting to 0 acts as setting to 1
        if val == 0 {
          self.rom_bank = 0x01;
        } else {
          self.rom_bank = val as usize & 0x7f;
        }
      }
      RAM_BANK_RTC_SELECT_START..=RAM_BANK_RTC_SELECT_END => {
        self.ram_rtc_select = RamRtcSelect::from(val)
      }
      LATCH_CLOCK_START..=LATCH_CLOCK_END => {
        // Latch when writing 0x00 and then 0x01
        if self.last_latch_val == 0x00 && val == 0x01 {
          self.latched_rtc = self.rtc;
        }
        self.last_latch_val = val;
      }
      ERAM_START..=ERAM_END => {
        if self.ram_and_timer_enabled {
          match self.ram_rtc_select {
            RamRtcSelect::RamBank(bank) => {
              if !self.ram.is_empty() {
                let bank = bank % self.ram.len();
                self.ram[bank][rel_ram_addr] = val;
              }
            }
            _ => self.write_rtc(val)?,
          }
        } else {
          warn!("Disabled ERAM write [{:02X}] -> ${:04X}", val, addr);
        }
      }
      _ => {
        error!("Invalid Write [{:02X}] -> ${:04X}", val, addr);
        return gb_err!(GbErrorType::OutOfBounds);
      }
    }

    Ok(())
  }
}
