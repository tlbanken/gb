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

  #[rustfmt::skip]
  /// Set up the dispatcher for CB prefix op codes
  fn init_dispatcher_cb() -> Vec<DispatchFn> {
    // opcodes from https://www.pastraiser.com/cpu/gameboy/gameboy_opcodes.html
    vec![
      /* 00 */ Self::rlc_b,   /* 01 */ Self::rlc_c,   /* 02 */ Self::rlc_d,       /* 03 */ Self::rlc_e,
      /* 04 */ Self::rlc_h,   /* 05 */ Self::rlc_l,   /* 06 */ Self::rlc__hl_,    /* 07 */ Self::rlc_a,
      /* 08 */ Self::rrc_b,   /* 09 */ Self::rrc_c,   /* 0A */ Self::rrc_d,       /* 0B */ Self::rrc_e,
      /* 0C */ Self::rrc_h,   /* 0D */ Self::rrc_l,   /* 0E */ Self::rrc__hl_,    /* 0F */ Self::rrc_a,

      /* 10 */ Self::rl_b,    /* 11 */ Self::rl_c,    /* 12 */ Self::rl_d,        /* 13 */ Self::rl_e,
      /* 14 */ Self::rl_h,    /* 15 */ Self::rl_l,    /* 16 */ Self::rl__hl_,     /* 17 */ Self::rl_a,
      /* 18 */ Self::rr_b,    /* 19 */ Self::rr_c,    /* 1A */ Self::rr_d,        /* 1B */ Self::rr_e,
      /* 1C */ Self::rr_h,    /* 1D */ Self::rr_l,    /* 1E */ Self::rr__hl_,     /* 1F */ Self::rr_a,

      /* 20 */ Self::sla_b,   /* 21 */ Self::sla_c,   /* 22 */ Self::sla_d,       /* 23 */ Self::sla_e,
      /* 24 */ Self::sla_h,   /* 25 */ Self::sla_l,   /* 26 */ Self::sla__hl_,    /* 27 */ Self::sla_a,
      /* 28 */ Self::sra_b,   /* 29 */ Self::sra_c,   /* 2A */ Self::sra_d,       /* 2B */ Self::sra_e,
      /* 2C */ Self::sra_h,   /* 2D */ Self::sra_l,   /* 2E */ Self::sra__hl_,    /* 2F */ Self::sra_a,

      /* 30 */ Self::swap_b,  /* 31 */ Self::swap_c,  /* 32 */ Self::swap_d,      /* 33 */ Self::swap_e,
      /* 34 */ Self::swap_h,  /* 35 */ Self::swap_l,  /* 36 */ Self::swap__hl_,   /* 37 */ Self::swap_a,
      /* 38 */ Self::srl_b,   /* 39 */ Self::srl_c,   /* 3A */ Self::srl_d,       /* 3B */ Self::srl_e,
      /* 3C */ Self::srl_h,   /* 3D */ Self::srl_l,   /* 3E */ Self::srl__hl_,    /* 3F */ Self::srl_a,

      /* 40 */ Self::bit_0_b, /* 41 */ Self::bit_0_c, /* 42 */ Self::bit_0_d,     /* 43 */ Self::bit_0_e,
      /* 44 */ Self::bit_0_h, /* 45 */ Self::bit_0_l, /* 46 */ Self::bit_0__hl_,  /* 47 */ Self::bit_0_a,
      /* 48 */ Self::bit_1_b, /* 49 */ Self::bit_1_c, /* 4A */ Self::bit_1_d,     /* 4B */ Self::bit_1_e,
      /* 4C */ Self::bit_1_h, /* 4D */ Self::bit_1_l, /* 4E */ Self::bit_1__hl_,  /* 4F */ Self::bit_1_a,

      /* 50 */ Self::bit_2_b, /* 51 */ Self::bit_2_c, /* 52 */ Self::bit_2_d,     /* 53 */ Self::bit_2_e,
      /* 54 */ Self::bit_2_h, /* 55 */ Self::bit_2_l, /* 56 */ Self::bit_2__hl_,  /* 57 */ Self::bit_2_a,
      /* 58 */ Self::bit_3_b, /* 59 */ Self::bit_3_c, /* 5A */ Self::bit_3_d,     /* 5B */ Self::bit_3_e,
      /* 5C */ Self::bit_3_h, /* 5D */ Self::bit_3_l, /* 5E */ Self::bit_3__hl_,  /* 5F */ Self::bit_3_a,

      /* 60 */ Self::bit_4_b, /* 61 */ Self::bit_4_c, /* 62 */ Self::bit_4_d,     /* 63 */ Self::bit_4_e,
      /* 64 */ Self::bit_4_h, /* 65 */ Self::bit_4_l, /* 66 */ Self::bit_4__hl_,  /* 67 */ Self::bit_4_a,
      /* 68 */ Self::bit_5_b, /* 69 */ Self::bit_5_c, /* 6A */ Self::bit_5_d,     /* 6B */ Self::bit_5_e,
      /* 6C */ Self::bit_5_h, /* 6D */ Self::bit_5_l, /* 6E */ Self::bit_5__hl_,  /* 6F */ Self::bit_5_a,

      /* 70 */ Self::bit_6_b, /* 71 */ Self::bit_6_c, /* 72 */ Self::bit_6_d,     /* 73 */ Self::bit_6_e,
      /* 74 */ Self::bit_6_h, /* 75 */ Self::bit_6_l, /* 76 */ Self::bit_6__hl_,  /* 77 */ Self::bit_6_a,
      /* 78 */ Self::bit_7_b, /* 79 */ Self::bit_7_c, /* 7A */ Self::bit_7_d,     /* 7B */ Self::bit_7_e,
      /* 7C */ Self::bit_7_h, /* 7D */ Self::bit_7_l, /* 7E */ Self::bit_7__hl_,  /* 7F */ Self::bit_7_a,

      /* 80 */ Self::res_0_b, /* 81 */ Self::res_0_c, /* 82 */ Self::res_0_d,     /* 83 */ Self::res_0_e,
      /* 84 */ Self::res_0_h, /* 85 */ Self::res_0_l, /* 86 */ Self::res_0__hl_,  /* 87 */ Self::res_0_a,
      /* 88 */ Self::res_1_b, /* 89 */ Self::res_1_c, /* 8A */ Self::res_1_d,     /* 8B */ Self::res_1_e,
      /* 8C */ Self::res_1_h, /* 8D */ Self::res_1_l, /* 8E */ Self::res_1__hl_,  /* 8F */ Self::res_1_a,

      /* 90 */ Self::res_2_b, /* 91 */ Self::res_2_c, /* 92 */ Self::res_2_d,     /* 93 */ Self::res_2_e,
      /* 94 */ Self::res_2_h, /* 95 */ Self::res_2_l, /* 96 */ Self::res_2__hl_,  /* 97 */ Self::res_2_a,
      /* 98 */ Self::res_3_b, /* 99 */ Self::res_3_c, /* 9A */ Self::res_3_d,     /* 9B */ Self::res_3_e,
      /* 9C */ Self::res_3_h, /* 9D */ Self::res_3_l, /* 9E */ Self::res_3__hl_,  /* 9F */ Self::res_3_a,

      /* A0 */ Self::res_4_b, /* A1 */ Self::res_4_c, /* A2 */ Self::res_4_d,     /* A3 */ Self::res_4_e,
      /* A4 */ Self::res_4_h, /* A5 */ Self::res_4_l, /* A6 */ Self::res_4__hl_,  /* A7 */ Self::res_4_a,
      /* A8 */ Self::res_5_b, /* A9 */ Self::res_5_c, /* AA */ Self::res_5_d,     /* AB */ Self::res_5_e,
      /* AC */ Self::res_5_h, /* AD */ Self::res_5_l, /* AE */ Self::res_5__hl_,  /* AF */ Self::res_5_a,

      /* B0 */ Self::res_6_b, /* B1 */ Self::res_6_c, /* B2 */ Self::res_6_d,     /* B3 */ Self::res_6_e,
      /* B4 */ Self::res_6_h, /* B5 */ Self::res_6_l, /* B6 */ Self::res_6__hl_,  /* B7 */ Self::res_6_a,
      /* B8 */ Self::res_7_b, /* B9 */ Self::res_7_c, /* BA */ Self::res_7_d,     /* BB */ Self::res_7_e,
      /* BC */ Self::res_7_h, /* BD */ Self::res_7_l, /* BE */ Self::res_7__hl_,  /* BF */ Self::res_7_a,

      /* C0 */ Self::set_0_b, /* C1 */ Self::set_0_c, /* C2 */ Self::set_0_d,     /* C3 */ Self::set_0_e,
      /* C4 */ Self::set_0_h, /* C5 */ Self::set_0_l, /* C6 */ Self::set_0__hl_,  /* C7 */ Self::set_0_a,
      /* C8 */ Self::set_1_b, /* C9 */ Self::set_1_c, /* CA */ Self::set_1_d,     /* CB */ Self::set_1_e,
      /* CC */ Self::set_1_h, /* CD */ Self::set_1_l, /* CE */ Self::set_1__hl_,  /* CF */ Self::set_1_a,

      /* D0 */ Self::set_2_b, /* D1 */ Self::set_2_c, /* D2 */ Self::set_2_d,     /* D3 */ Self::set_2_e,
      /* D4 */ Self::set_2_h, /* D5 */ Self::set_2_l, /* D6 */ Self::set_2__hl_,  /* D7 */ Self::set_2_a,
      /* D8 */ Self::set_3_b, /* D9 */ Self::set_3_c, /* DA */ Self::set_3_d,     /* DB */ Self::set_3_e,
      /* DC */ Self::set_3_h, /* DD */ Self::set_3_l, /* DE */ Self::set_3__hl_,  /* DF */ Self::set_3_a,

      /* E0 */ Self::set_4_b, /* E1 */ Self::set_4_c, /* E2 */ Self::set_4_d,     /* E3 */ Self::set_4_e,
      /* E4 */ Self::set_4_h, /* E5 */ Self::set_4_l, /* E6 */ Self::set_4__hl_,  /* E7 */ Self::set_4_a,
      /* E8 */ Self::set_5_b, /* E9 */ Self::set_5_c, /* EA */ Self::set_5_d,     /* EB */ Self::set_5_e,
      /* EC */ Self::set_5_h, /* ED */ Self::set_5_l, /* EE */ Self::set_5__hl_,  /* EF */ Self::set_5_a,

      /* F0 */ Self::set_6_b, /* F1 */ Self::set_6_c, /* F2 */ Self::set_6_d,     /* F3 */ Self::set_6_e,
      /* F4 */ Self::set_6_h, /* F5 */ Self::set_6_l, /* F6 */ Self::set_6__hl_,  /* F7 */ Self::set_6_a,
      /* F8 */ Self::set_7_b, /* F9 */ Self::set_7_c, /* FA */ Self::set_7_d,     /* FB */ Self::set_7_e,
      /* FC */ Self::set_7_h, /* FD */ Self::set_7_l, /* FE */ Self::set_7__hl_,  /* FF */ Self::set_7_a,
    ]
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

  // *** Prefix CB ***

  fn rlc_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rlc_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rlc_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rlc_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rlc_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rlc_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rlc__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rlc_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rrc_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rrc_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rrc_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rrc_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rrc_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rrc_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rrc__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rrc_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rl_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rl_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rl_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rl_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rl_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rl_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rl__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rl_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rr_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rr_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rr_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rr_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rr_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rr_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rr__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rr_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sla_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sla_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sla_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sla_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sla_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sla_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sla__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sla_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sra_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sra_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sra_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sra_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sra_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sra_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sra__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sra_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn swap_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn swap_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn swap_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn swap_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn swap_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn swap_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn swap__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn swap_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn srl_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn srl_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn srl_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn srl_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn srl_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn srl_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn srl__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn srl_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_0_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_0_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_0_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_0_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_0_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_0_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_0__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_0_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_1_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_1_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_1_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_1_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_1_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_1_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_1__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_1_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_2_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_2_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_2_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_2_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_2_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_2_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_2__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_2_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_3_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_3_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_3_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_3_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_3_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_3_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_3__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_3_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_4_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_4_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_4_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_4_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_4_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_4_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_4__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_4_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_5_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_5_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_5_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_5_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_5_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_5_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_5__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_5_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_6_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_6_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_6_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_6_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_6_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_6_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_6__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_6_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_7_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_7_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_7_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_7_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_7_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_7_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_7__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_7_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_0_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_0_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_0_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_0_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_0_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_0_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_0__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_0_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_1_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_1_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_1_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_1_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_1_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_1_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_1__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_1_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_2_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_2_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_2_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_2_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_2_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_2_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_2__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_2_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_3_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_3_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_3_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_3_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_3_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_3_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_3__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_3_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_4_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_4_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_4_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_4_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_4_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_4_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_4__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_4_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_5_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_5_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_5_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_5_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_5_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_5_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_5__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_5_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_6_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_6_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_6_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_6_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_6_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_6_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_6__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_6_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_7_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_7_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_7_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_7_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_7_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_7_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_7__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_7_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_0_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_0_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_0_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_0_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_0_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_0_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_0__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_0_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_1_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_1_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_1_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_1_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_1_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_1_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_1__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_1_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_2_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_2_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_2_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_2_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_2_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_2_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_2__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_2_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_3_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_3_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_3_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_3_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_3_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_3_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_3__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_3_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_4_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_4_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_4_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_4_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_4_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_4_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_4__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_4_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_5_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_5_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_5_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_5_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_5_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_5_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_5__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_5_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_6_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_6_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_6_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_6_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_6_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_6_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_6__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_6_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_7_b(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_7_c(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_7_d(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_7_e(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_7_h(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_7_l(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_7__hl_(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_7_a(&mut self, instr: u8) -> GbResult<()> {
    unimplemented!()
  }
}
