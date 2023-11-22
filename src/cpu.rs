//! Cpu module for the Gameboy emulator. The Gameboy uses a "8-bit 8080-like
//! Sharp CPU (speculated to be a SM83 core)". It runs at a freq of 4.194304
//! MHz.
#![allow(non_snake_case)]

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
      /* 00 */ Self::nop,         /* 01 */ Self::ld_bc_d16, /* 02 */ Self::ld__bc__a,  /* 03 */ Self::inc_bc,
      /* 04 */ Self::inc_b,       /* 05 */ Self::dec_b,     /* 06 */ Self::ld_b_d8,    /* 07 */ Self::rlca,
      /* 08 */ Self::ld__a16__sp, /* 09 */ Self::add_hl_bc, /* 0A */ Self::ld_a__bc_,  /* 0B */ Self::dec_bc,
      /* 0C */ Self::inc_c,       /* 0D */ Self::dec_c,     /* 0E */ Self::ld_c_d8,    /* 0F */ Self::rrca,

      /* 10 */ Self::stop,        /* 11 */ Self::ld_de_d16, /* 12 */ Self::ld__de__a,  /* 13 */ Self::inc_de,
      /* 14 */ Self::inc_d,       /* 15 */ Self::dec_d,     /* 16 */ Self::ld_d_d8,    /* 17 */ Self::rla,
      /* 18 */ Self::jr_r8,       /* 19 */ Self::add_hl_de, /* 1A */ Self::ld_a__de_,  /* 1B */ Self::dec_de,
      /* 1C */ Self::inc_e,       /* 1D */ Self::dec_e,     /* 1E */ Self::ld_e_d8,    /* 1F */ Self::rra,

      /* 20 */ Self::jr_nz_r8,    /* 21 */ Self::ld_hl_d16, /* 22 */ Self::ld__hlp__a, /* 23 */ Self::inc_hl,
      /* 24 */ Self::inc_h,       /* 25 */ Self::dec_h,     /* 26 */ Self::ld_h_d8,    /* 27 */ Self::daa,
      /* 28 */ Self::jr_z_r8,     /* 29 */ Self::add_hl_hl, /* 2A */ Self::ld_a__hlp_, /* 2B */ Self::dec_hl,
      /* 2C */ Self::inc_l,       /* 2D */ Self::dec_l,     /* 2E */ Self::ld_l_d8,    /* 2F */ Self::cpl,

      /* 30 */ Self::jr_nc_r8,    /* 31 */ Self::ld_sp_d16, /* 32 */ Self::ld__hlm__a, /* 33 */ Self::inc_sp,
      /* 34 */ Self::inc__hl_,    /* 35 */ Self::dec__hl_,  /* 36 */ Self::ld__hl__d8, /* 37 */ Self::scf,
      /* 38 */ Self::jr_c_r8,     /* 39 */ Self::add_hl_sp, /* 3A */ Self::ld_a__hlm_, /* 3B */ Self::dec_sp,
      /* 3C */ Self::inc_a,       /* 3D */ Self::dec_a,     /* 3E */ Self::ld_a_d8,    /* 3F */ Self::ccf,

      /* 40 */ Self::ld_b_b,      /* 41 */ Self::ld_b_c,    /* 42 */ Self::ld_b_d,     /* 43 */ Self::ld_b_e,
      /* 44 */ Self::ld_b_h,      /* 45 */ Self::ld_b_l,    /* 46 */ Self::ld_b__hl_,  /* 47 */ Self::ld_b_a,
      /* 48 */ Self::ld_c_b,      /* 49 */ Self::ld_c_c,    /* 4A */ Self::ld_c_d,     /* 4B */ Self::ld_c_e,
      /* 4C */ Self::ld_c_h,      /* 4D */ Self::ld_c_l,    /* 4E */ Self::ld_c__hl_,  /* 4F */ Self::ld_c_a,

      /* 50 */ Self::ld_d_b,      /* 51 */ Self::ld_d_c,    /* 52 */ Self::ld_d_d,     /* 53 */ Self::ld_d_e,
      /* 54 */ Self::ld_d_h,      /* 55 */ Self::ld_d_l,    /* 56 */ Self::ld_d__hl_,  /* 57 */ Self::ld_d_a,
      /* 58 */ Self::ld_e_b,      /* 59 */ Self::ld_e_c,    /* 5A */ Self::ld_e_d,     /* 5B */ Self::ld_e_e,
      /* 5C */ Self::ld_e_h,      /* 5D */ Self::ld_e_l,    /* 5E */ Self::ld_e__hl_,  /* 5F */ Self::ld_e_a,

      /* 60 */ Self::ld_h_b,      /* 61 */ Self::ld_h_c,    /* 62 */ Self::ld_h_d,     /* 63 */ Self::ld_h_e,
      /* 64 */ Self::ld_h_h,      /* 65 */ Self::ld_h_l,    /* 66 */ Self::ld_h__hl_,  /* 67 */ Self::ld_h_a,
      /* 68 */ Self::ld_l_b,      /* 69 */ Self::ld_l_c,    /* 6A */ Self::ld_l_d,     /* 6B */ Self::ld_l_e,
      /* 6C */ Self::ld_l_h,      /* 6D */ Self::ld_l_l,    /* 6E */ Self::ld_l__hl_,  /* 6F */ Self::ld_l_a,

      /* 70 */ Self::ld__hl__b,   /* 71 */ Self::ld__hl__c, /* 72 */ Self::ld__hl__d,  /* 73 */ Self::ld__hl__e,
      /* 74 */ Self::ld__hl__h,   /* 75 */ Self::ld__hl__l, /* 76 */ Self::halt,       /* 77 */ Self::ld__hl__a,
      /* 78 */ Self::ld_a_b,      /* 79 */ Self::ld_a_c,    /* 7A */ Self::ld_a_d,     /* 7B */ Self::ld_a_e,
      /* 7C */ Self::ld_a_h,      /* 7D */ Self::ld_a_l,    /* 7E */ Self::ld_a__hl_,  /* 7F */ Self::ld_a_a,

      /* 80 */ Self::add_a_b,     /* 81 */ Self::add_a_c,   /* 82 */ Self::add_a_d,    /* 83 */ Self::add_a_e,
      /* 84 */ Self::add_a_h,     /* 85 */ Self::add_a_l,   /* 86 */ Self::add_a__hl_, /* 87 */ Self::add_a_a,
      /* 88 */ Self::adc_a_b,     /* 89 */ Self::adc_a_c,   /* 8A */ Self::adc_a_d,    /* 8B */ Self::adc_a_e,
      /* 8C */ Self::adc_a_h,     /* 8D */ Self::adc_a_l,   /* 8E */ Self::adc_a__hl_, /* 8F */ Self::adc_a_a,

      /* 90 */ Self::sub_b,       /* 91 */ Self::sub_c,     /* 92 */ Self::sub_d,      /* 93 */ Self::sub_e,
      /* 94 */ Self::sub_h,       /* 95 */ Self::sub_l,     /* 96 */ Self::sub__hl_,   /* 97 */ Self::sub_a,
      /* 98 */ Self::sbc_a_b,     /* 99 */ Self::sbc_a_c,   /* 9A */ Self::sbc_a_d,    /* 9B */ Self::sbc_a_e,
      /* 9C */ Self::sbc_a_h,     /* 9D */ Self::sbc_a_l,   /* 9E */ Self::sbc_a__hl_, /* 9F */ Self::sbc_a_a,

      /* A0 */ Self::and_b,       /* A1 */ Self::and_c,     /* A2 */ Self::and_d,      /* A3 */ Self::and_e,
      /* A4 */ Self::and_h,       /* A5 */ Self::and_l,     /* A6 */ Self::and__hl_,   /* A7 */ Self::and_a,
      /* A8 */ Self::xor_b,       /* A9 */ Self::xor_c,     /* AA */ Self::xor_d,      /* AB */ Self::xor_e,
      /* AC */ Self::xor_h,       /* AD */ Self::xor_l,     /* AE */ Self::xor__hl_,   /* AF */ Self::xor_a,

      /* B0 */ Self::or_b,        /* B1 */ Self::or_c,      /* B2 */ Self::or_d,       /* B3 */ Self::or_e,
      /* B4 */ Self::or_h,        /* B5 */ Self::or_l,      /* B6 */ Self::or__hl_,    /* B7 */ Self::or_a,
      /* B8 */ Self::cp_b,        /* B9 */ Self::cp_c,      /* BA */ Self::cp_d,       /* BB */ Self::cp_e,
      /* BC */ Self::cp_h,        /* BD */ Self::cp_l,      /* BE */ Self::cp__hl_,    /* BF */ Self::cp_a,

      /* C0 */ Self::req_nz,      /* C1 */ Self::pop_bc,    /* C2 */ Self::jp_nz_a16,  /* C3 */ Self::jp_a16,
      /* C4 */ Self::call_nz_a16, /* C5 */ Self::push_bc,   /* C6 */ Self::add_a_d8,   /* C7 */ Self::rst_00h,
      /* C8 */ Self::ret_z,       /* C9 */ Self::ret,       /* CA */ Self::jp_z_a16,   /* CB */ Self::prefix_cb,
      /* CC */ Self::call_z_a16,  /* CD */ Self::call_a16,  /* CE */ Self::adc_a_d8,   /* CF */ Self::rst_08h,

      /* D0 */ Self::ret_nc,      /* D1 */ Self::pop_de,    /* D2 */ Self::jp_nc_a16,  /* D3 */ Self::badi,
      /* D4 */ Self::call_nc_a16, /* D5 */ Self::push_de,   /* D6 */ Self::sub_d8,     /* D7 */ Self::rst_10h,
      /* D8 */ Self::ret_c,       /* D9 */ Self::reti,      /* DA */ Self::jp_c_a16,   /* DB */ Self::badi,
      /* DC */ Self::call_c_a16,  /* DD */ Self::badi,      /* DE */ Self::sbc_a_d8,   /* DF */ Self::rst_18h,

      /* E0 */ Self::ldh__a8__a,  /* E1 */ Self::pop_hl,    /* E2 */ Self::ld__c__a,   /* E3 */ Self::badi,
      /* E4 */ Self::badi,        /* E5 */ Self::push_hl,   /* E6 */ Self::and_d8,     /* E7 */ Self::rst_20h,
      /* E8 */ Self::add_sp_r8,   /* E9 */ Self::jp__hl_,   /* EA */ Self::ld__a16__a, /* EB */ Self::badi,
      /* EC */ Self::badi,        /* ED */ Self::badi,      /* EE */ Self::xor_d8,     /* EF */ Self::rst_28h,

      /* F0 */ Self::ldh_a__a8_,  /* F1 */ Self::pop_af,    /* F2 */ Self::ld_a__c_,   /* F3 */ Self::di,
      /* F4 */ Self::badi,        /* F5 */ Self::push_af,   /* F6 */ Self::or_d8,      /* F7 */ Self::rst_30h,
      /* F8 */ Self::ld_hl_sp_r8, /* F9 */ Self::ld_sp_hl,  /* FA */ Self::ld_a__a16_, /* FB */ Self::ei,
      /* FC */ Self::badi,        /* FD */ Self::badi,      /* FE */ Self::cp_d8,      /* FF */ Self::rst_38h,
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
  fn badi(&mut self, instr: u8) -> GbResult<()> {
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

  fn halt(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn prefix_cb(&mut self, instr: u8) -> GbResult<()> {
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

  fn ld_hl_d16(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld__hlp__a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_h_d8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_l_d8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_sp_d16(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_a__hlp_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld__hlm__a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld__hl__d8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_a__hlm_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_a_d8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_b_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_b_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_b_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_b_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_b_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_b_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_b__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_b_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_c_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_c_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_c_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_c_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_c_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_c_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_c__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_c_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_d_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_d_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_d_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_d_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_d_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_d_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_d__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_d_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_e_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_e_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_e_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_e_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_e_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_e_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_e__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_e_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_h_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_h_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_h_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_h_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_h_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_h_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_h__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_h_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_l_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_l_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_l_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_l_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_l_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_l_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_l__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_l_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld__hl__b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld__hl__c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld__hl__d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld__hl__e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld__hl__h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld__hl__l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld__hl__a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_a_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_a_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_a_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_a_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_a_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_a_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_a__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_a_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld__c__a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld__a16__a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_a__c_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_sp_hl(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_a__a16_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ld_hl_sp_r8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ldh__a8__a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ldh_a__a8_(&mut self, instr: u8) -> GbResult<()> {
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

  fn inc_hl(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn inc_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn inc_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn inc__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn inc_sp(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn inc_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn dec_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn dec_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn dec_bc(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn dec_sp(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn dec_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn dec_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn dec_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn dec_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn dec_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn dec_de(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn dec_hl(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn dec__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn add_hl_bc(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn add_hl_hl(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn add_hl_de(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn add_hl_sp(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn add_a_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn add_a_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn add_a_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn add_a_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn add_a_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn add_a_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn add_a__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn add_a_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn add_a_d8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn add_sp_r8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn adc_a_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn adc_a_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn adc_a_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn adc_a_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn adc_a_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn adc_a_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn adc_a__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn adc_a_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn adc_a_d8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sub_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sub_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sub_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sub_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sub_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sub_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sub__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sub_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sub_d8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sbc_a_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sbc_a_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sbc_a_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sbc_a_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sbc_a_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sbc_a_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sbc_a__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sbc_a_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sbc_a_d8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn and_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn and_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn and_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn and_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn and_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn and_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn and__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn and_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn and_d8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn xor_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn xor_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn xor_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn xor_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn xor_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn xor_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn xor__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn xor_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn xor_d8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn or_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn or_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn or_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn or_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn or_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn or_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn or__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn or_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn or_d8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn cp_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn cp_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn cp_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn cp_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn cp_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn cp_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn cp__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn cp_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn cp_d8(&mut self, instr: u8) -> GbResult<()> {
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

  fn daa(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn cpl(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn scf(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ccf(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  // *** Branch/Jumps ***

  fn jr_r8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn jr_nz_r8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn jr_z_r8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn jr_nc_r8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn jr_c_r8(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn jp_nz_a16(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn jp_a16(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn jp_z_a16(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn jp_nc_a16(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn jp_c_a16(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn jp__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn call_nz_a16(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn call_z_a16(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn call_a16(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn call_nc_a16(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn call_c_a16(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rst_00h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rst_08h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rst_10h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rst_18h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rst_20h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rst_28h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rst_30h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rst_38h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ret(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ret_z(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ret_nc(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ret_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn reti(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  // *** Other ***

  fn req_nz(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn pop_bc(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn pop_de(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn pop_hl(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn pop_af(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn push_bc(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn push_de(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn push_hl(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn push_af(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn di(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn ei(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }
}
