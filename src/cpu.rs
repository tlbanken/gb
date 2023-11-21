//! Cpu module for the Gameboy emulator. The Gameboy uses a "8-bit 8080-like
//! Sharp CPU (speculated to be a SM83 core)". It runs at a freq of 4.194304
//! MHz.

use log::error;
use std::{cell::RefCell, rc::Rc};

use crate::{
  bus::Bus,
  err::{GbError, GbErrorType, GbResult},
  gb_err,
  util::LazyDref,
};

type DispatchFn = fn(&mut Cpu, instr: u8) -> GbResult<()>;

pub struct Cpu {
  // registers: named as HiLo (A F -> Hi Lo)
  af: Register,
  bc: Register,
  de: Register,
  hl: Register,
  sp: u16,
  pc: u16,
  bus: Option<Rc<RefCell<Bus>>>,

  // instruction dispatchers
  dispatcher: Vec<DispatchFn>,
  dispatcher_cb: Vec<DispatchFn>,
}

struct Register {
  pub lo: u8,
  pub hi: u8,
}

impl Register {
  pub fn new() -> Register {
    Register { lo: 0, hi: 0 }
  }

  pub fn get_u16(&self) -> u16 {
    (self.lo as u16) | ((self.hi as u16) << 8)
  }

  pub fn set_u16(&mut self, val: u16) {
    self.lo = val as u8;
    self.hi = (val >> 8) as u8;
  }
}

impl Cpu {
  pub fn new() -> Cpu {
    Cpu {
      af: Register::new(),
      bc: Register::new(),
      de: Register::new(),
      hl: Register::new(),
      sp: 0,
      pc: 0,
      bus: None,
      dispatcher: Self::init_dispatcher(),
      dispatcher_cb: Self::init_dispatcher_cb(),
    }
  }

  /// Connect the cpu to a given bus
  pub fn connect_bus(&mut self, bus: Rc<RefCell<Bus>>) -> GbResult<()> {
    match self.bus {
      None => self.bus = Some(bus),
      Some(_) => return gb_err!(GbErrorType::AlreadyInitialized),
    };
    Ok(())
  }

  /// Execute one instruction
  pub fn step(&mut self) -> GbResult<()> {
    // read next instruction
    let instr = self.bus.lazy_dref().read(self.pc)?;

    // instruction dispatch
    self.dispatcher[instr as usize](self, instr)?;

    Ok(())
  }

  #[rustfmt::skip]
  /// Set up the dispatcher for general op codes
  fn init_dispatcher() -> Vec<DispatchFn> {
    // opcodes from https://www.pastraiser.com/cpu/gameboy/gameboy_opcodes.html
    vec![
      /* 00 */ Self::nop,         /* 01 */ Self::ld_bc_d16, /* 02 */ Self::ld__bc__a, /* 03 */ Self::inc_bc,
      /* 04 */ Self::inc_b,       /* 05 */ Self::dec_b,     /* 06 */ Self::ld_b_d8,   /* 07 */ Self::rlca,
      /* 08 */ Self::ld__a16__sp, /* 09 */ Self::add_hl_bc, /* 0A */ Self::ld_a__bc_, /* 0B */ Self::dec_bc,
      /* 0C */ Self::inc_c,       /* 0D */ Self::dec_c,     /* 0E */ Self::ld_c_d8,   /* 0F */ Self::rrca,

      /* 10 */ Self::stop,        /* 11 */ Self::ld_de_d16, /* 12 */ Self::ld__de__a, /* 13 */ Self::inc_de,
      /* 14 */ Self::inc_d,       /* 15 */ Self::dec_d,     /* 16 */ Self::ld_d_d8,   /* 17 */ Self::rla,
      /* 18 */ Self::jr_r8,       /* 19 */ Self::add_hl_de, /* 1A */ Self::ld_a__de_, /* 1B */ Self::dec_de,
      /* 1C */ Self::inc_e,       /* 1D */ Self::dec_e,     /* 1E */ Self::ld_e_d8,   /* 1F */ Self::rra,

      // TODO
    ]
  }

  /// Set up the dispatcher for CB prefix op codes
  fn init_dispatcher_cb() -> Vec<DispatchFn> {
    // opcodes from https://www.pastraiser.com/cpu/gameboy/gameboy_opcodes.html
    vec![]
  }

  // *** Instruction Dispatchers ***
  // Flags: Z N H C
  //  Z: Zero Flag
  //  N: Subtract Flag
  //  H: Half Carry Flag
  //  C: Carry Flag
  //  0: Reset Flag to 0
  //  -: No change

  /// Unknown Instruction, returns an error
  fn invalid_instr(&mut self, instr: u8) -> GbResult<()> {
    error!("Unknown instruction: 0x{:02x}", instr);
    gb_err!(GbErrorType::InvalidCpuInstruction)
  }

  fn dispatch_cb(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  /// nop
  ///
  /// Size: 1
  ///
  /// Cycles: 4
  ///
  /// Flags: - - - -
  ///
  /// Description: no operation
  fn nop(&mut self, _instr: u8) -> GbResult<()> {
    Ok(())
  }

  fn stop(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  // *** Loads/Stores ***

  fn ld_bc_d16(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_b_d8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld__bc__a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_a__bc_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld__a16__sp(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_c_d8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_de_d16(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld__de__a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_d_d8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_a__de_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_e_d8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  // *** ALU ***

  fn inc_bc(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn inc_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn inc_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn inc_de(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn inc_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn inc_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn dec_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn dec_bc(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn dec_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn dec_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn dec_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn dec_de(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn add_hl_bc(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn add_hl_de(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rlca(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rrca(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rla(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rra(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  // *** Branch/Jumps ***

  fn jr_r8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }
}
