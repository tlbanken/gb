//! Cpu module for the Gameboy emulator. The Gameboy uses a "8-bit 8080-like
//! Sharp CPU (speculated to be a SM83 core)". It runs at a freq of 4.194304
//! MHz.
#![allow(non_snake_case)]

use log::error;
use std::collections::VecDeque;
use std::env;
use std::fs::File;
use std::io::Write;
use std::{cell::RefCell, rc::Rc};

use crate::{
  bus::Bus,
  dasm::Dasm,
  err::{GbError, GbErrorType, GbResult},
  gb_err,
  util::LazyDref,
};

type DispatchFn = fn(&mut Cpu, instr: u8) -> GbResult<()>;

// flags const
/// Zero flag. Set if result of an operation is zero.
pub const FLAG_Z: u8 = (1 << 7);
/// Subtraction Flag. Indicates if the previous instruction was a subtraction.
pub const FLAG_N: u8 = (1 << 6);
/// Half-Carry Flag. Indicates carry for the lower 4 bits of the result.
pub const FLAG_H: u8 = (1 << 5);
/// Carry Flag. Set on the following:
///
/// * When the result of an 8-bit addition is higher than $FF.
/// * When the result of a 16-bit addition is higher than $FFFF.
/// * When the result of a subtraction or comparison is lower than zero (like in
///   Z80 and x86 CPUs, but unlike in 65XX and ARM CPUs).
/// * When a rotate/shift operation shifts out a “1” bit.
pub const FLAG_C: u8 = (1 << 4);

const HISTORY_CAP: usize = 5;

pub struct InstrHistory {
  cap: usize,
  data: VecDeque<u16>,
}

impl InstrHistory {
  pub fn new(cap: usize) -> InstrHistory {
    InstrHistory {
      data: VecDeque::new(),
      cap,
    }
  }

  pub fn len(&self) -> usize {
    self.data.len()
  }

  pub fn cap(&self) -> usize {
    self.cap
  }

  pub fn push(&mut self, entry: u16) {
    self.data.push_back(entry);
    if self.data.len() > self.cap {
      self.data.pop_front();
    }
  }

  pub fn entries(&self) -> &VecDeque<u16> {
    &self.data
  }
}

pub struct Cpu {
  // registers: named as HiLo (A F -> Hi Lo)
  /// A -> Hi, F -> Lo
  pub af: Register,
  /// B -> Hi, C -> Lo
  pub bc: Register,
  /// D -> Hi, E -> Lo
  pub de: Register,
  /// H -> Hi, L -> Lo
  pub hl: Register,
  pub sp: u16,
  pub pc: u16,
  pub bus: Option<Rc<RefCell<Bus>>>,
  pub history: InstrHistory,
  #[cfg(feature = "instr-trace")]
  trace_file: File,

  // instruction dispatchers
  dispatcher: Vec<DispatchFn>,
  dispatcher_cb: Vec<DispatchFn>,
}

pub struct Register {
  pub lo: u8,
  pub hi: u8,
}

impl Register {
  pub fn new() -> Register {
    Register { lo: 0, hi: 0 }
  }

  pub fn hilo(&self) -> u16 {
    (self.lo as u16) | ((self.hi as u16) << 8)
  }

  pub fn set_u16(&mut self, val: u16) {
    self.lo = val as u8;
    self.hi = (val >> 8) as u8;
  }
}

impl Cpu {
  pub fn new() -> Cpu {
    #[cfg(feature = "instr-trace")]
    let trace_file = {
      let mut path = env::current_exe().unwrap();
      path.pop();
      path.push("gb_instr_dump.txt");
      File::create(&path).unwrap()
    };
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
      history: InstrHistory::new(HISTORY_CAP),
      #[cfg(feature = "instr-trace")]
      trace_file,
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
    // instruction tracing
    #[cfg(feature = "instr-trace")]
    {
      let mut dasm = Dasm::new();
      let mut raw_bytes = Vec::<u8>::new();
      let mut vpc = self.pc;
      let mut output = format!(" PC:{:04X}  ", vpc);
      loop {
        let byte = self.bus.lazy_dref().read8(vpc).unwrap();
        raw_bytes.push(byte);
        vpc += 1;
        if let Some(instr) = dasm.munch(byte) {
          let mut raw_bytes_str = String::new();
          for b in raw_bytes {
            raw_bytes_str.push_str(format!("{:02X} ", b).as_str());
          }
          output.push_str(format!("{:9} ", raw_bytes_str).as_str());
          output.push_str(format!("{:12} ", instr).as_str());
          break;
        }
      }
      self.trace_instr(&output);
    }

    // read next instruction
    self.history.push(self.pc);
    let instr = self.bus.lazy_dref().read8(self.pc)?;
    self.pc += 1;

    // instruction dispatch
    self.dispatcher[instr as usize](self, instr)?;

    Ok(())
  }

  #[cfg(feature = "instr-trace")]
  fn trace_instr(&mut self, s: &str) {
    writeln!(self.trace_file, "{}", s).unwrap();
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

      /* 20 */ Self::jr_nz_r8,    /* 21 */ Self::ld_hl_d16, /* 22 */ Self::ld__hli__a, /* 23 */ Self::inc_hl,
      /* 24 */ Self::inc_h,       /* 25 */ Self::dec_h,     /* 26 */ Self::ld_h_d8,    /* 27 */ Self::daa,
      /* 28 */ Self::jr_z_r8,     /* 29 */ Self::add_hl_hl, /* 2A */ Self::ld_a__hli_, /* 2B */ Self::dec_hl,
      /* 2C */ Self::inc_l,       /* 2D */ Self::dec_l,     /* 2E */ Self::ld_l_d8,    /* 2F */ Self::cpl,

      /* 30 */ Self::jr_nc_r8,    /* 31 */ Self::ld_sp_d16, /* 32 */ Self::ld__hld__a, /* 33 */ Self::inc_sp,
      /* 34 */ Self::inc__hl_,    /* 35 */ Self::dec__hl_,  /* 36 */ Self::ld__hl__d8, /* 37 */ Self::scf,
      /* 38 */ Self::jr_c_r8,     /* 39 */ Self::add_hl_sp, /* 3A */ Self::ld_a__hld_, /* 3B */ Self::dec_sp,
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

  // *** Helpers ***

  /// Reads the next 2 bytes and constructs the imm16 value. This will modify
  /// the pc state.
  fn get_imm16(&mut self) -> GbResult<u16> {
    let imm16 = self.bus.lazy_dref().read16(self.pc)?;
    self.pc = self.pc.wrapping_add(2);
    Ok(imm16)
  }

  /// Reads the next byte and constructs the imm8 value. This will modify
  /// the pc state.
  fn get_imm8(&mut self) -> GbResult<u8> {
    let imm8 = self.bus.lazy_dref().read8(self.pc)?;
    self.pc = self.pc.wrapping_add(1);
    Ok(imm8)
  }

  /// Unknown Instruction, returns an error
  fn badi(&mut self, instr: u8) -> GbResult<()> {
    error!("Unknown instruction: 0x{:02x}", instr);
    gb_err!(GbErrorType::InvalidCpuInstruction)
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

  fn stop(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn halt(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  /// CB XX
  ///
  /// Dispatches an instruction which has the "CB" prefix.
  fn prefix_cb(&mut self, _instr: u8) -> GbResult<()> {
    self.pc = self.pc.wrapping_add(1);
    let instr = self.bus.lazy_dref().read8(self.pc)?;
    self.dispatcher_cb[instr as usize](self, instr)
  }

  // *** Loads/Stores ***

  /// LD BC d16
  ///
  /// Loads an imm16 into BC register.
  ///
  /// Flags: - - - -
  fn ld_bc_d16(&mut self, _instr: u8) -> GbResult<()> {
    let d16 = self.get_imm16()?;
    self.bc.set_u16(d16);
    Ok(())
  }

  /// LD B d8
  ///
  /// Loads an imm8 into the B register.
  ///
  /// Flags: - - - -
  fn ld_b_d8(&mut self, _instr: u8) -> GbResult<()> {
    let d8 = self.get_imm8()?;
    self.bc.hi = d8;
    Ok(())
  }

  /// LD (BC) A
  ///
  /// Store A into address pointed to by BC
  ///
  /// Flags: - - - -
  fn ld__bc__a(&mut self, _instr: u8) -> GbResult<()> {
    self
      .bus
      .lazy_dref_mut()
      .write8(self.bc.hilo(), self.af.hi)?;
    Ok(())
  }

  /// LD A (BC)
  ///
  /// Load A from address pointed to by BC
  ///
  /// Flags: - - - -
  fn ld_a__bc_(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.bus.lazy_dref().read8(self.bc.hilo())?;
    Ok(())
  }

  /// LD (a16) SP
  ///
  /// Store SP into address given by imm16
  ///
  /// Flags: - - - -
  fn ld__a16__sp(&mut self, _instr: u8) -> GbResult<()> {
    let a16 = self.get_imm16()?;
    self.bus.lazy_dref_mut().write16(a16, self.sp)
  }

  /// LD C d8
  ///
  /// Load imm8 into C register
  ///
  /// FLAGS: - - - -
  fn ld_c_d8(&mut self, _instr: u8) -> GbResult<()> {
    let d8 = self.get_imm8()?;
    self.bc.lo = d8;
    Ok(())
  }

  /// LD DE d16
  ///
  /// Load imm16 into DE register
  ///
  /// FLAGS: - - - -
  fn ld_de_d16(&mut self, _instr: u8) -> GbResult<()> {
    let d16 = self.get_imm16()?;
    self.de.set_u16(d16);
    Ok(())
  }

  /// LD (DE) A
  ///
  /// Store A register into address pointed by DE
  ///
  /// FLAGS: - - - -
  fn ld__de__a(&mut self, _instr: u8) -> GbResult<()> {
    self.bus.lazy_dref_mut().write8(self.de.hilo(), self.af.hi)
  }

  /// LD D d8
  ///
  /// Load imm8 into D
  ///
  /// Flags: - - - -
  fn ld_d_d8(&mut self, _instr: u8) -> GbResult<()> {
    let d8 = self.get_imm8()?;
    self.de.hi = d8;
    Ok(())
  }

  /// LD A (DE)
  ///
  /// Load value pointed to by DE into A.
  ///
  /// Flags: - - - -
  fn ld_a__de_(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.bus.lazy_dref().read8(self.de.hilo())?;
    Ok(())
  }

  /// LD E d8
  ///
  /// Load imm8 into E register
  ///
  /// Flags: - - - -
  fn ld_e_d8(&mut self, _instr: u8) -> GbResult<()> {
    let d8 = self.get_imm8()?;
    self.de.lo = d8;
    Ok(())
  }

  /// LD HI d16
  ///
  /// Load imm16 into HL register.
  ///
  /// Flags: - - - -
  fn ld_hl_d16(&mut self, _instr: u8) -> GbResult<()> {
    let d16 = self.get_imm16()?;
    self.hl.set_u16(d16);
    Ok(())
  }

  /// LD (HL+) A
  ///
  /// Load A into value pointed by HL. Increment HL.
  ///
  /// Flags: - - - -
  fn ld__hli__a(&mut self, _instr: u8) -> GbResult<()> {
    self
      .bus
      .lazy_dref_mut()
      .write8(self.hl.hilo(), self.af.hi)?;
    self.hl.set_u16(self.hl.hilo().wrapping_add(1));
    Ok(())
  }

  /// LD H d8
  ///
  /// Load imm8 into H register.
  ///
  /// Flags: - - - -
  fn ld_h_d8(&mut self, _instr: u8) -> GbResult<()> {
    let d8 = self.get_imm8()?;
    self.hl.hi = d8;
    Ok(())
  }

  /// LD L d8
  ///
  /// Load imm8 into L register.
  ///
  /// Flags: - - - -
  fn ld_l_d8(&mut self, _instr: u8) -> GbResult<()> {
    let d8 = self.get_imm8()?;
    self.hl.lo = d8;
    Ok(())
  }

  /// ld sp d16
  ///
  /// Loads the sp register with the provided imm16
  ///
  /// Flags: - - - -
  fn ld_sp_d16(&mut self, _instr: u8) -> GbResult<()> {
    let d16 = self.get_imm16()?;
    self.sp = d16;
    Ok(())
  }

  /// LD A (HL+)
  ///
  /// Loads value pointed by HL into A and increments HL.
  ///
  /// Flags: - - - -
  fn ld_a__hli_(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.bus.lazy_dref().read8(self.hl.hilo())?;
    self.hl.set_u16(self.hl.hilo().wrapping_add(1));
    Ok(())
  }

  /// LD (HL-) A
  ///
  /// Store A into address pointed by HL and decrement HL.
  ///
  /// Flags: - - - -
  fn ld__hld__a(&mut self, _instr: u8) -> GbResult<()> {
    self
      .bus
      .lazy_dref_mut()
      .write8(self.hl.hilo(), self.af.hi)?;
    self.hl.set_u16(self.hl.hilo().wrapping_sub(1));
    Ok(())
  }

  /// LD (HL) d8
  ///
  /// Store imm8 into address pointed to by HL.
  ///
  /// Flags: - - - -
  fn ld__hl__d8(&mut self, _instr: u8) -> GbResult<()> {
    let d8 = self.get_imm8()?;
    self.bus.lazy_dref_mut().write8(self.hl.hilo(), d8)
  }

  /// LD A (HL-)
  ///
  /// Load value pointed to by HL into A register. Decrement HL register.
  ///
  /// Flags: - - - -
  fn ld_a__hld_(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.bus.lazy_dref().read8(self.hl.hilo())?;
    self.hl.set_u16(self.hl.hilo().wrapping_sub(1));
    Ok(())
  }

  /// LD A d8
  ///
  /// Load imm8 into A register
  ///
  /// Flags: - - - -
  fn ld_a_d8(&mut self, _instr: u8) -> GbResult<()> {
    let d8 = self.get_imm8()?;
    self.af.hi = d8;
    Ok(())
  }

  /// LD B B
  ///
  /// Load B register into B
  ///
  /// Flags: - - - -
  fn ld_b_b(&mut self, _instr: u8) -> GbResult<()> {
    // nop
    Ok(())
  }

  /// LD B C
  ///
  /// Load C into B register.
  ///
  /// Flags: - - - -
  fn ld_b_c(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.hi = self.bc.lo;
    Ok(())
  }

  /// LD B D
  ///
  /// Load D into B register
  ///
  /// Flags: - - - -
  fn ld_b_d(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.hi = self.de.hi;
    Ok(())
  }

  /// LD B E
  ///
  /// Load E into B register.
  ///
  /// Flags: - - - -
  fn ld_b_e(&mut self, _instr: u8) -> GbResult<()> {
    self.de.lo = self.bc.hi;
    Ok(())
  }

  /// LD B H
  ///
  /// Load H into B register.
  ///
  /// Flags: - - - -
  fn ld_b_h(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.hi = self.hl.hi;
    Ok(())
  }

  /// LD B L
  ///
  /// Load L into B register.
  ///
  /// Flags: - - - -
  fn ld_b_l(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.hi = self.hl.lo;
    Ok(())
  }

  /// LD B (HL)
  ///
  /// Load value pointed by HL into B register.
  ///
  /// Flags: - - - -
  fn ld_b__hl_(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.hi = self.bus.lazy_dref().read8(self.hl.hilo())?;
    Ok(())
  }

  /// LD B A
  ///
  /// Load A into B register
  ///
  /// Flags: - - - -
  fn ld_b_a(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.hi = self.af.hi;
    Ok(())
  }

  /// LD C B
  ///
  /// Load B into C register.
  ///
  /// Flags: - - - -
  fn ld_c_b(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.lo = self.bc.hi;
    Ok(())
  }

  /// LD C C
  ///
  /// Load C into C register
  ///
  /// Flags: - - - -
  fn ld_c_c(&mut self, _instr: u8) -> GbResult<()> {
    // nop
    Ok(())
  }

  /// LD C D
  ///
  /// Load D into C register
  ///
  /// Flags: - - - -
  fn ld_c_d(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.lo = self.de.hi;
    Ok(())
  }

  /// LD C E
  ///
  /// Load E into C register
  ///
  /// Flags: - - - -
  fn ld_c_e(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.lo = self.de.lo;
    Ok(())
  }

  /// LD C H
  ///
  /// Load H into C register
  ///
  /// Flags: - - - -
  fn ld_c_h(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.lo = self.hl.hi;
    Ok(())
  }

  /// LD C L
  ///
  /// Load L into C register
  ///
  /// Flags: - - - -
  fn ld_c_l(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.lo = self.hl.lo;
    Ok(())
  }

  /// LD C (HL)
  ///
  /// Load val pointed by HL into C register
  ///
  /// Flags: - - - -
  fn ld_c__hl_(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.lo = self.bus.lazy_dref().read8(self.hl.hilo())?;
    Ok(())
  }

  /// LD C A
  ///
  /// Load A into C register
  ///
  /// Flags: - - - -
  fn ld_c_a(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.lo = self.af.hi;
    Ok(())
  }

  /// LD D B
  ///
  /// Load B into D register
  ///
  /// Flags: - - - -
  fn ld_d_b(&mut self, _instr: u8) -> GbResult<()> {
    self.de.hi = self.bc.hi;
    Ok(())
  }

  /// LD D C
  ///
  /// Load C into D register
  ///
  /// Flags: - - - -
  fn ld_d_c(&mut self, _instr: u8) -> GbResult<()> {
    self.de.hi = self.bc.lo;
    Ok(())
  }

  /// LD D D
  ///
  /// Load D into D register
  ///
  /// Flags: - - - -
  fn ld_d_d(&mut self, _instr: u8) -> GbResult<()> {
    // nop
    Ok(())
  }

  /// LD D E
  ///
  /// Load E into D
  ///
  /// Flags: - - - -
  fn ld_d_e(&mut self, _instr: u8) -> GbResult<()> {
    self.de.hi = self.de.lo;
    Ok(())
  }

  /// LD D H
  ///
  /// Load H into D register
  ///
  /// Flags: - - - -
  fn ld_d_h(&mut self, _instr: u8) -> GbResult<()> {
    self.de.hi = self.hl.hi;
    Ok(())
  }

  /// LD D L
  ///
  /// Load L into D register
  ///
  /// Flags: - - - -
  fn ld_d_l(&mut self, _instr: u8) -> GbResult<()> {
    self.de.hi = self.hl.lo;
    Ok(())
  }

  /// LD D (HL)
  ///
  /// Load value pointed to by HL into D
  ///
  /// Flags: - - - -
  fn ld_d__hl_(&mut self, _instr: u8) -> GbResult<()> {
    self.de.hi = self.bus.lazy_dref().read8(self.hl.hilo())?;
    Ok(())
  }

  /// LD D A
  ///
  /// Load A into D
  ///
  /// Flags: - - - -
  fn ld_d_a(&mut self, _instr: u8) -> GbResult<()> {
    self.de.hi = self.af.hi;
    Ok(())
  }

  /// LD E B
  ///
  /// Load B into E
  ///
  /// Flags: - - - -
  fn ld_e_b(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.hi = self.de.lo;
    Ok(())
  }

  /// LD E C
  ///
  /// Load C into E
  ///
  /// Flags: - - - -
  fn ld_e_c(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.lo = self.de.lo;
    Ok(())
  }

  /// LD E D
  ///
  /// Load D into E
  ///
  /// Flags: - - - -
  fn ld_e_d(&mut self, _instr: u8) -> GbResult<()> {
    self.de.lo = self.de.hi;
    Ok(())
  }

  /// LD E E
  ///
  /// Load E into E
  ///
  /// Flags: - - - -
  fn ld_e_e(&mut self, _instr: u8) -> GbResult<()> {
    // nop
    Ok(())
  }

  /// LD E H
  ///
  /// Load H into E
  ///
  /// Flags: - - - -
  fn ld_e_h(&mut self, _instr: u8) -> GbResult<()> {
    self.de.lo = self.hl.hi;
    Ok(())
  }

  /// LD E L
  ///
  /// Load L into E
  ///
  /// Flags: - - - -
  fn ld_e_l(&mut self, _instr: u8) -> GbResult<()> {
    self.de.lo = self.hl.lo;
    Ok(())
  }

  /// LD E (HL)
  ///
  /// Load value pointed to by HL into E
  ///
  /// Flags: - - - -
  fn ld_e__hl_(&mut self, _instr: u8) -> GbResult<()> {
    self.de.lo = self.bus.lazy_dref().read8(self.hl.hilo())?;
    Ok(())
  }

  /// LD E A
  ///
  /// Load A into E
  ///
  /// Flags: - - - -
  fn ld_e_a(&mut self, _instr: u8) -> GbResult<()> {
    self.de.lo = self.af.hi;
    Ok(())
  }

  /// LD H B
  ///
  /// Load B into H
  ///
  /// Flags: - - - -
  fn ld_h_b(&mut self, _instr: u8) -> GbResult<()> {
    self.hl.hi = self.bc.hi;
    Ok(())
  }

  /// LD H C
  ///
  /// Load C into H
  ///
  /// Flags: - - - -
  fn ld_h_c(&mut self, _instr: u8) -> GbResult<()> {
    self.hl.hi = self.bc.lo;
    Ok(())
  }

  /// LD H D
  ///
  /// Load D into H
  ///
  /// Flags: - - - -
  fn ld_h_d(&mut self, _instr: u8) -> GbResult<()> {
    self.de.hi = self.hl.hi;
    Ok(())
  }

  /// LD H E
  ///
  /// Load E into H
  ///
  /// Flags: - - - -
  fn ld_h_e(&mut self, _instr: u8) -> GbResult<()> {
    self.hl.hi = self.de.lo;
    Ok(())
  }

  /// LD H H
  ///
  /// Load H into H
  ///
  /// Flags: - - - -
  fn ld_h_h(&mut self, _instr: u8) -> GbResult<()> {
    // nop
    Ok(())
  }

  /// LD H L
  ///
  /// Load L into H
  ///
  /// Flags: - - - -
  fn ld_h_l(&mut self, _instr: u8) -> GbResult<()> {
    self.hl.hi = self.hl.lo;
    Ok(())
  }

  /// LD H (HL)
  ///
  /// Load val pointed to by HL into H
  ///
  /// Flags: - - - -
  fn ld_h__hl_(&mut self, _instr: u8) -> GbResult<()> {
    self.hl.hi = self.bus.lazy_dref().read8(self.hl.hilo())?;
    Ok(())
  }

  /// LD H A
  ///
  /// Load A into H
  ///
  /// Flags: - - - -
  fn ld_h_a(&mut self, _instr: u8) -> GbResult<()> {
    self.hl.hi = self.af.hi;
    Ok(())
  }

  /// LD L B
  ///
  /// Load B into L
  ///
  /// Flags: - - - -
  fn ld_l_b(&mut self, _instr: u8) -> GbResult<()> {
    self.hl.lo = self.bc.hi;
    Ok(())
  }

  /// LD L C
  ///
  /// Load C into L
  ///
  /// Flags: - - - -
  fn ld_l_c(&mut self, _instr: u8) -> GbResult<()> {
    self.hl.lo = self.bc.lo;
    Ok(())
  }

  /// LD L D
  ///
  /// Load D into L
  ///
  /// Flags: - - - -
  fn ld_l_d(&mut self, _instr: u8) -> GbResult<()> {
    self.de.hi = self.hl.lo;
    Ok(())
  }

  /// LD L E
  ///
  /// Load E into L
  ///
  /// Flags: - - - -
  fn ld_l_e(&mut self, _instr: u8) -> GbResult<()> {
    self.de.lo = self.hl.lo;
    Ok(())
  }

  /// LD L H
  ///
  /// Load H into L
  ///
  /// Flags: - - - -
  fn ld_l_h(&mut self, _instr: u8) -> GbResult<()> {
    self.hl.lo = self.hl.hi;
    Ok(())
  }

  /// LD L L
  ///
  /// Load L into L
  ///
  /// Flags: - - - -
  fn ld_l_l(&mut self, _instr: u8) -> GbResult<()> {
    // nop
    Ok(())
  }

  /// LD L (HL)
  ///
  /// Load value pointed by HL into L
  ///
  /// Flags: - - - -
  fn ld_l__hl_(&mut self, _instr: u8) -> GbResult<()> {
    self.hl.lo = self.bus.lazy_dref().read8(self.hl.hilo())?;
    Ok(())
  }

  /// LD L A
  ///
  /// Load A into L
  ///
  /// Flags: - - - -
  fn ld_l_a(&mut self, _instr: u8) -> GbResult<()> {
    self.hl.lo = self.af.hi;
    Ok(())
  }

  /// LD (HL) B
  ///
  /// Store B into address held by HL
  ///
  /// Flags: - - - -
  fn ld__hl__b(&mut self, _instr: u8) -> GbResult<()> {
    self.bus.lazy_dref_mut().write8(self.hl.hilo(), self.bc.hi)
  }

  /// LD (HL) C
  ///
  /// Store C into address held by HL
  ///
  /// Flags: - - - -
  fn ld__hl__c(&mut self, _instr: u8) -> GbResult<()> {
    self.bus.lazy_dref_mut().write8(self.hl.hilo(), self.bc.lo)
  }

  /// LD (HL) D
  ///
  /// Store D into address held by HL
  ///
  /// Flags: - - - -
  fn ld__hl__d(&mut self, _instr: u8) -> GbResult<()> {
    self.bus.lazy_dref_mut().write8(self.hl.hilo(), self.de.hi)
  }

  /// LD (HL) E
  ///
  /// Store E into address held by HL
  ///
  /// Flags: - - - -
  fn ld__hl__e(&mut self, _instr: u8) -> GbResult<()> {
    self.bus.lazy_dref_mut().write8(self.hl.hilo(), self.de.lo)
  }

  /// LD (HL) H
  ///
  /// Store H into address held by HL
  ///
  /// Flags: - - - -
  fn ld__hl__h(&mut self, _instr: u8) -> GbResult<()> {
    self.bus.lazy_dref_mut().write8(self.hl.hilo(), self.hl.hi)
  }

  /// LD (HL) L
  ///
  /// Store L into address held by HL
  ///
  /// Flags: - - - -
  fn ld__hl__l(&mut self, _instr: u8) -> GbResult<()> {
    self.bus.lazy_dref_mut().write8(self.hl.hilo(), self.hl.lo)
  }

  /// LD (HL) A
  ///
  /// Store A into address held by HL
  ///
  /// Flags: - - - -
  fn ld__hl__a(&mut self, _instr: u8) -> GbResult<()> {
    self.bus.lazy_dref_mut().write8(self.hl.hilo(), self.af.hi)
  }

  /// LD A B
  ///
  /// Load B into A
  ///
  /// Flags: - - - -
  fn ld_a_b(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.bc.hi;
    Ok(())
  }

  /// LD A C
  ///
  /// Load C into A
  ///
  /// Flags: - - - -
  fn ld_a_c(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.bc.lo;
    Ok(())
  }

  /// LD A D
  ///
  /// Load D into A
  ///
  /// Flags: - - - -
  fn ld_a_d(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.de.hi;
    Ok(())
  }

  /// LD A E
  ///
  /// Load E into A
  ///
  /// Flags: - - - -
  fn ld_a_e(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.de.lo;
    Ok(())
  }

  /// LD A H
  ///
  /// Load H into A
  ///
  /// Flags: - - - -
  fn ld_a_h(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.hl.hi;
    Ok(())
  }

  /// LD A L
  ///
  /// Load L into A
  ///
  /// Flags: - - - -
  fn ld_a_l(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.hl.lo;
    Ok(())
  }

  /// LD A (HL)
  ///
  /// Load value pointed to by HL into A
  ///
  /// Flags: - - - -
  fn ld_a__hl_(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.bus.lazy_dref().read8(self.hl.hilo())?;
    Ok(())
  }

  /// LD A A
  ///
  /// Load A into A
  ///
  /// Flags: - - - -
  fn ld_a_a(&mut self, _instr: u8) -> GbResult<()> {
    // nop
    Ok(())
  }

  /// LD (C) A
  ///
  /// Load A into address 0xFF00 + C
  ///
  /// Flags: - - - -
  fn ld__c__a(&mut self, _instr: u8) -> GbResult<()> {
    self
      .bus
      .lazy_dref_mut()
      .write8(0xff00 + self.bc.lo as u16, self.af.hi)
  }

  /// LD (a16) A
  ///
  /// Store A into imm16 address
  ///
  /// Flags: - - - -
  fn ld__a16__a(&mut self, _instr: u8) -> GbResult<()> {
    let a16 = self.get_imm16()?;
    self.bus.lazy_dref_mut().write8(a16, self.af.hi)
  }

  /// LD A (C)
  ///
  /// Load from 0xFF00 + C into A
  ///
  /// Flags: - - - -
  fn ld_a__c_(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.bus.lazy_dref().read8(0xff00 + self.bc.lo as u16)?;
    Ok(())
  }

  /// LD SP HL
  ///
  /// Load HL into SP
  ///
  /// Flags: - - - -
  fn ld_sp_hl(&mut self, _instr: u8) -> GbResult<()> {
    self.sp = self.hl.hilo();
    Ok(())
  }

  /// LD A (a16)
  ///
  /// Load value from provided address into A
  ///
  /// Flags: - - - -
  fn ld_a__a16_(&mut self, _instr: u8) -> GbResult<()> {
    let a16 = self.get_imm16()?;
    self.af.hi = self.bus.lazy_dref().read8(a16)?;
    Ok(())
  }

  /// LD HL SP+r8
  ///
  /// Load SP + r8 into HL
  ///
  /// Flags: 0 0 H C
  fn ld_hl_sp_r8(&mut self, _instr: u8) -> GbResult<()> {
    // reset flags
    self.af.lo = 0;

    // read r8 with sign extension
    let r8 = self.get_imm8()? as i8 as i16;
    let hcarry = if (self.sp & 0xf) as u8 + (r8 & 0xf) as u8 > 0xf {
      FLAG_H
    } else {
      0
    };
    let carry = if (self.sp & 0xff) + (r8 & 0xff) as u16 > 0xff {
      FLAG_C
    } else {
      0
    };

    // update state
    self.af.lo |= carry | hcarry;
    self.hl.set_u16(self.sp.wrapping_add_signed(r8));
    Ok(())
  }

  /// LDH (a8) A
  ///
  /// Store A into 0xff00 + imm8
  ///
  /// Flags: - - - -
  fn ldh__a8__a(&mut self, _instr: u8) -> GbResult<()> {
    let a8 = self.get_imm8()? as u16;
    self.bus.lazy_dref_mut().write8(0xff00 + a8, self.af.hi)
  }

  /// LDH A (a8)
  ///
  /// Load from 0xff00 + imm8 into A
  ///
  /// Flags: - - - -
  fn ldh_a__a8_(&mut self, _instr: u8) -> GbResult<()> {
    let a8 = self.get_imm8()? as u16;
    self.af.hi = self.bus.lazy_dref().read8(0xff00 + a8)?;
    Ok(())
  }

  // *** ALU ***

  // Helpers

  /// Add 2 u8 values, affects Z, N, and H flags
  fn add_hc(&mut self, n1: u8, n2: u8) -> u8 {
    // remove everything other than carry flag
    self.af.lo &= FLAG_C;
    let res = n1.wrapping_add(n2);

    // check half carry
    self.af.lo |= if (n1 & 0xf) + (n2 & 0xf) > 0xf {
      FLAG_H
    } else {
      0
    };

    // check zero
    self.af.lo |= if res == 0 { FLAG_Z } else { 0 };

    // sub flag always zero so don't set it

    res
  }

  /// Add 2 u16 values, affects N, H, C flags
  fn add16(&mut self, n1: u16, n2: u16) -> u16 {
    // reset all but Z flag
    self.af.lo &= FLAG_Z;

    // check half carry
    self.af.lo |= if (n1 & 0x0fff) + (n2 & 0x0fff) > 0x0fff {
      FLAG_H
    } else {
      0
    };

    // check full carry
    self.af.lo |= if (n1 as u32) + (n2 as u32) > 0xffff_u32 {
      FLAG_C
    } else {
      0
    };

    // not sub so leave that as 0

    n1.wrapping_add(n2)
  }

  fn add8(&mut self, n1: u8, n2: u8) -> u8 {
    // reset flags
    self.af.lo = 0;
    let res = n1.wrapping_add(n2);

    // check half carry
    self.af.lo |= if (n1 & 0x0f) + (n2 & 0x0f) > 0x0f {
      FLAG_H
    } else {
      0
    };

    // check full carry
    self.af.lo |= if (n1 as u16) + (n2 as u16) > 0xff_u16 {
      FLAG_C
    } else {
      0
    };

    // check zero flag
    self.af.lo |= if res == 0 { FLAG_Z } else { 0 };

    // not sub so leave that as 0

    res
  }

  fn adc8(&mut self, n1: u8, n2: u8) -> u8 {
    let carry = if self.af.lo & FLAG_C > 0 { 1 } else { 0 };

    // reset flags
    self.af.lo = 0;
    let res = n1.wrapping_add(n2).wrapping_add(carry);

    // check half carry
    self.af.lo |= if (n1 & 0x0f) + (n2 & 0x0f) + carry > 0x0f {
      FLAG_H
    } else {
      0
    };

    // check full carry
    self.af.lo |= if (n1 as u16) + (n2 as u16) + carry as u16 > 0xff_u16 {
      FLAG_C
    } else {
      0
    };

    // check zero flag
    self.af.lo |= if res == 0 { FLAG_Z } else { 0 };

    // not sub so leave that as 0

    res
  }

  /// Sub 2 u8 values, affects Z, N, and H flags
  fn sub_hc(&mut self, n1: u8, n2: u8) -> u8 {
    // remove everything other than the carry flag
    self.af.lo &= FLAG_C;
    let res = n1.wrapping_sub(n2);

    // check half carry
    self.af.lo |= if (n2 & 0xf) > (n1 & 0xf) { FLAG_H } else { 0 };

    // check zero
    self.af.lo |= if res == 0 { FLAG_Z } else { 0 };

    // this is a sub operation, so set the flag
    self.af.lo |= FLAG_N;

    res
  }

  /// Subs r from self.a and sets appropriate flags.
  fn sub_r(&mut self, r: u8) {
    // reset flags
    self.af.lo = 0;
    let a = self.af.hi;
    let res = a.wrapping_sub(r);

    // check half carry
    self.af.lo |= if (r & 0xf) > (a & 0xf) { FLAG_H } else { 0 };

    // check carry
    self.af.lo |= if r > a { FLAG_C } else { 0 };

    // check zero
    self.af.lo |= if res == 0 { FLAG_Z } else { 0 };

    // this is a sub operation, so set the flag
    self.af.lo |= FLAG_N;

    self.af.hi = res;
  }

  /// Subs r from self.a and sets appropriate flags.
  fn sbc_r(&mut self, r: u8) {
    let carry = if self.af.lo & FLAG_C > 0 { 1 } else { 0 };

    // reset flags
    self.af.lo = 0;
    let a = self.af.hi;
    let res = a.wrapping_sub(r).wrapping_sub(carry);

    // check half carry
    self.af.lo |= if ((r & 0xf) + 1) > (a & 0xf) {
      FLAG_H
    } else {
      0
    };

    // check carry
    self.af.lo |= if (r as u16 + 1) > (a as u16) {
      FLAG_C
    } else {
      0
    };

    // check zero
    self.af.lo |= if res == 0 { FLAG_Z } else { 0 };

    // this is a sub operation, so set the flag
    self.af.lo |= FLAG_N;

    self.af.hi = res;
  }

  fn and_r(&mut self, r: u8) {
    // start with only H flags set.
    self.af.lo = FLAG_H;
    self.af.hi &= r;
    // check zero flag
    self.af.lo |= if self.af.hi == 0 { FLAG_Z } else { 0 };
  }

  fn xor_r(&mut self, r: u8) {
    // reset flags
    self.af.lo = 0;
    self.af.hi ^= r;
    // check zero flag
    self.af.lo |= if self.af.hi == 0 { FLAG_Z } else { 0 };
  }

  fn or_r(&mut self, r: u8) {
    // reset flags
    self.af.lo = 0;
    self.af.hi |= r;
    // check zero flag
    self.af.lo |= if self.af.hi == 0 { FLAG_Z } else { 0 };
  }

  fn cp_r(&mut self, r: u8) {
    // reset flags
    self.af.lo = 0;

    // half carry
    self.af.lo |= if (r & 0x0f) > (self.af.hi & 0x0f) {
      FLAG_H
    } else {
      0
    };

    // carry
    self.af.lo |= if r > self.af.hi { FLAG_C } else { 0 };

    // zero
    self.af.lo |= if r == self.af.hi { FLAG_Z } else { 0 };

    // sub
    self.af.lo |= FLAG_N;
  }

  /// INC BC
  ///
  /// Increment the BC register.
  ///
  /// Flags: - - - -
  fn inc_bc(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.set_u16(self.bc.hilo().wrapping_add(1));
    Ok(())
  }

  /// INC B
  ///
  /// Increment the B register
  ///
  /// Flags: Z 0 H -
  fn inc_b(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.hi = self.add_hc(self.bc.hi, 1);
    Ok(())
  }

  /// INC C
  ///
  /// Increment the C register
  ///
  /// Flags: Z 0 H -
  fn inc_c(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.lo = self.add_hc(self.bc.lo, 1);
    Ok(())
  }

  /// INC DE
  ///
  /// Increment the DC register
  ///
  /// Flags: - - - -
  fn inc_de(&mut self, _instr: u8) -> GbResult<()> {
    self.de.set_u16(self.de.hilo().wrapping_add(1));
    Ok(())
  }

  /// INC D
  ///
  /// Increment the D register
  ///
  /// Flags: Z 0 H -
  fn inc_d(&mut self, _instr: u8) -> GbResult<()> {
    self.de.hi = self.add_hc(self.de.hi, 1);
    Ok(())
  }

  /// INC E
  ///
  /// Increment the E register
  ///
  /// Flags: Z 0 H -
  fn inc_e(&mut self, _instr: u8) -> GbResult<()> {
    self.de.lo = self.add_hc(self.de.lo, 1);
    Ok(())
  }

  /// INC HL
  ///
  /// Increment the HL register
  ///
  /// Flags: - - - -
  fn inc_hl(&mut self, _instr: u8) -> GbResult<()> {
    self.hl.set_u16(self.hl.hilo().wrapping_add(1));
    Ok(())
  }

  /// INC H
  ///
  /// Increment the H register
  ///
  /// Flags: Z 0 H -
  fn inc_h(&mut self, _instr: u8) -> GbResult<()> {
    self.hl.hi = self.add_hc(self.hl.hi, 1);
    Ok(())
  }

  /// INC L
  ///
  /// Increment the L register
  ///
  /// Flags: Z 0 H -
  fn inc_l(&mut self, _instr: u8) -> GbResult<()> {
    self.hl.lo = self.add_hc(self.hl.lo, 1);
    Ok(())
  }

  /// INC (HL)
  ///
  /// Increment the value pointed by HL
  ///
  /// Flags: Z 0 H -
  fn inc__hl_(&mut self, _instr: u8) -> GbResult<()> {
    let val = self.bus.lazy_dref().read8(self.hl.hilo())?;
    let val = self.add_hc(val, 1);
    self.bus.lazy_dref_mut().write8(self.hl.hilo(), val)
  }

  /// INC SP
  ///
  /// Increment the SP register
  ///
  /// Flags: - - - -
  fn inc_sp(&mut self, _instr: u8) -> GbResult<()> {
    self.sp = self.sp.wrapping_add(1);
    Ok(())
  }

  /// INC A
  ///
  /// Increment the A register
  ///
  /// Flags: Z 0 H -
  fn inc_a(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.add_hc(self.af.hi, 1);
    Ok(())
  }

  /// DEC A
  ///
  /// Decrements the A register
  ///
  /// Flags: Z 1 H -
  fn dec_a(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.sub_hc(self.af.hi, 1);
    Ok(())
  }

  /// DEC B
  ///
  /// Decrements the B register
  ///
  /// Flags: Z 1 H -
  fn dec_b(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.hi = self.sub_hc(self.bc.hi, 1);
    Ok(())
  }

  /// DEC BC
  ///
  /// Decrements the BC register
  ///
  /// Flags: - - - -
  fn dec_bc(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.set_u16(self.bc.hilo().wrapping_sub(1));
    Ok(())
  }

  /// DEC SP
  ///
  /// Decrements the SP register
  ///
  /// Flags: - - - -
  fn dec_sp(&mut self, _instr: u8) -> GbResult<()> {
    self.sp = self.sp.wrapping_sub(1);
    Ok(())
  }

  /// DEC C
  ///
  /// Decrements the C register
  ///
  /// Flags: Z 1 H -
  fn dec_c(&mut self, _instr: u8) -> GbResult<()> {
    self.bc.lo = self.sub_hc(self.bc.lo, 1);
    Ok(())
  }

  /// DEC E
  ///
  /// Decrements the E register
  ///
  /// Flags: Z 1 H -
  fn dec_e(&mut self, _instr: u8) -> GbResult<()> {
    self.de.lo = self.sub_hc(self.de.lo, 1);
    Ok(())
  }

  /// DEC L
  ///
  /// Decrements the L register
  ///
  /// Flags: Z 1 H -
  fn dec_l(&mut self, _instr: u8) -> GbResult<()> {
    self.hl.lo = self.sub_hc(self.hl.lo, 1);
    Ok(())
  }

  /// DEC D
  ///
  /// Decrements the D register
  ///
  /// Flags: Z 1 H -
  fn dec_d(&mut self, _instr: u8) -> GbResult<()> {
    self.de.hi = self.sub_hc(self.de.hi, 1);
    Ok(())
  }

  /// DEC H
  ///
  /// Decrements the H register
  ///
  /// Flags: Z 1 H -
  fn dec_h(&mut self, _instr: u8) -> GbResult<()> {
    self.hl.hi = self.sub_hc(self.hl.hi, 1);
    Ok(())
  }

  /// DEC DE
  ///
  /// Decrements the DE register
  ///
  /// Flags: - - - -
  fn dec_de(&mut self, _instr: u8) -> GbResult<()> {
    self.de.set_u16(self.de.hilo().wrapping_sub(1));
    Ok(())
  }

  /// DEC HL
  ///
  /// Decrements the HL register
  ///
  /// Flags: - - - -
  fn dec_hl(&mut self, _instr: u8) -> GbResult<()> {
    self.hl.set_u16(self.hl.hilo().wrapping_sub(1));
    Ok(())
  }

  /// DEC (HL)
  ///
  /// Decrements the value pointed to by HL
  ///
  /// Flags: Z 1 H -
  fn dec__hl_(&mut self, _instr: u8) -> GbResult<()> {
    let val = self.bus.lazy_dref().read8(self.hl.hilo())?;
    let val = self.sub_hc(val, 1);
    self.bus.lazy_dref_mut().write8(self.hl.hilo(), val)
  }

  /// ADD HL BC
  ///
  /// Add BC to HL and store into HL
  ///
  /// Flags: - 0 H C
  fn add_hl_bc(&mut self, _instr: u8) -> GbResult<()> {
    let res = self.add16(self.hl.hilo(), self.bc.hilo());
    self.hl.set_u16(res);
    Ok(())
  }

  /// ADD HL HL
  ///
  /// Add HL to HL and store into HL
  ///
  /// Flags: - 0 H C
  fn add_hl_hl(&mut self, _instr: u8) -> GbResult<()> {
    let res = self.add16(self.hl.hilo(), self.hl.hilo());
    self.hl.set_u16(res);
    Ok(())
  }

  /// ADD HL DE
  ///
  /// Add DE to HL and store into HL
  ///
  /// Flags: - 0 H C
  fn add_hl_de(&mut self, _instr: u8) -> GbResult<()> {
    let res = self.add16(self.hl.hilo(), self.de.hilo());
    self.hl.set_u16(res);
    Ok(())
  }

  /// ADD HL SP
  ///
  /// Add SP to HL and store into HL
  ///
  /// Flags: - 0 H C
  fn add_hl_sp(&mut self, _instr: u8) -> GbResult<()> {
    let res = self.add16(self.hl.hilo(), self.sp);
    self.hl.set_u16(res);
    Ok(())
  }

  /// ADD A B
  ///
  /// Add B to A and store into A
  ///
  /// Flags: Z 0 H C
  fn add_a_b(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.add8(self.af.hi, self.bc.hi);
    Ok(())
  }

  /// ADD A C
  ///
  /// Add C to A and store into A
  ///
  /// Flags: Z 0 H C
  fn add_a_c(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.add8(self.af.hi, self.bc.lo);
    Ok(())
  }

  /// ADD A D
  ///
  /// Add D to A and store into A
  ///
  /// Flags: Z 0 H C
  fn add_a_d(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.add8(self.af.hi, self.de.hi);
    Ok(())
  }

  /// ADD A E
  ///
  /// Add E to A and store into A
  ///
  /// Flags: Z 0 H C
  fn add_a_e(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.add8(self.af.hi, self.de.lo);
    Ok(())
  }

  /// ADD A H
  ///
  /// Add H to A and store into A
  ///
  /// Flags: Z 0 H C
  fn add_a_h(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.add8(self.af.hi, self.hl.hi);
    Ok(())
  }

  /// ADD A L
  ///
  /// Add L to A and store into A
  ///
  /// Flags: Z 0 H C
  fn add_a_l(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.add8(self.af.hi, self.hl.lo);
    Ok(())
  }

  /// ADD A (HL)
  ///
  /// Add value pointed by HL to A and store into A
  ///
  /// Flags: Z 0 H C
  fn add_a__hl_(&mut self, _instr: u8) -> GbResult<()> {
    let val = self.bus.lazy_dref().read8(self.hl.hilo())?;
    self.af.hi = self.add8(self.af.hi, val);
    Ok(())
  }

  /// ADD A A
  ///
  /// Add A to A and store into A
  ///
  /// Flags: Z 0 H C
  fn add_a_a(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.add8(self.af.hi, self.af.hi);
    Ok(())
  }

  /// ADD A d8
  ///
  /// Add imm8 with A and store into A
  ///
  /// Flags: Z 0 H C
  fn add_a_d8(&mut self, _instr: u8) -> GbResult<()> {
    let d8 = self.get_imm8()?;
    self.af.hi = self.add8(self.af.hi, d8);
    Ok(())
  }

  /// ADD SP r8
  ///
  /// Add imm8 to SP and store into SP
  ///
  /// Flags: 0 0 H C
  fn add_sp_r8(&mut self, _instr: u8) -> GbResult<()> {
    // reset flags
    self.af.lo = 0;

    // read r8 with sign extension
    let r8 = self.get_imm8()? as i8 as i16;
    let hcarry = if (self.sp & 0xf) as u8 + (r8 & 0xf) as u8 > 0xf {
      FLAG_H
    } else {
      0
    };
    let carry = if (self.sp & 0xff) + (r8 & 0xff) as u16 > 0xff {
      FLAG_C
    } else {
      0
    };

    // update state
    self.af.lo |= carry | hcarry;
    self.sp = self.sp.wrapping_add_signed(r8);
    Ok(())
  }

  /// ADC A B
  ///
  /// Add B to A with Carry and store into A
  ///
  /// Flags: Z 0 H C
  fn adc_a_b(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.adc8(self.af.hi, self.bc.hi);
    Ok(())
  }

  /// ADC A C
  ///
  /// Add C to A with carry and store into A
  ///
  /// Flags: Z 0 H C
  fn adc_a_c(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.adc8(self.af.hi, self.bc.lo);
    Ok(())
  }

  /// ADC A D
  ///
  /// Add D to A with Carry and store into A
  ///
  /// Flags: Z 0 H C
  fn adc_a_d(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.adc8(self.af.hi, self.de.hi);
    Ok(())
  }

  /// ADC A E
  ///
  /// Add E to A with carry and store into A
  ///
  /// Flags: Z 0 H C
  fn adc_a_e(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.adc8(self.af.hi, self.de.lo);
    Ok(())
  }

  /// ADC A H
  ///
  /// Add A to H with carry and store into A
  ///
  /// Flags: Z 0 H C
  fn adc_a_h(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.adc8(self.af.hi, self.hl.hi);
    Ok(())
  }

  /// ADC A L
  ///
  /// Add L to A with carry and store into A
  ///
  /// Flags: Z 0 H C
  fn adc_a_l(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.adc8(self.af.hi, self.hl.lo);
    Ok(())
  }

  /// ADC A (HL)
  ///
  /// Add value pointed by HL to A and store into A
  ///
  /// Flags: Z 0 H C
  fn adc_a__hl_(&mut self, _instr: u8) -> GbResult<()> {
    let val = self.bus.lazy_dref().read8(self.hl.hilo())?;
    self.af.hi = self.adc8(self.af.hi, val);
    Ok(())
  }

  /// ADC A A
  ///
  /// Add A to A with carry and store into A
  ///
  /// Flags: Z 0 H C
  fn adc_a_a(&mut self, _instr: u8) -> GbResult<()> {
    self.af.hi = self.adc8(self.af.hi, self.af.hi);
    Ok(())
  }

  /// ADC A d8
  ///
  /// Add imm8 to A and store into A
  ///
  /// Flags: Z 0 H C
  fn adc_a_d8(&mut self, _instr: u8) -> GbResult<()> {
    let d8 = self.get_imm8()?;
    self.af.hi = self.adc8(self.af.hi, d8);
    Ok(())
  }

  /// SUB B
  ///
  /// Sub B from A and store into A
  ///
  /// Flags: Z 1 H C
  fn sub_b(&mut self, _instr: u8) -> GbResult<()> {
    self.sub_r(self.bc.hi);
    Ok(())
  }

  /// SUB C
  ///
  /// Sub C from A and store into A
  ///
  /// Flags: Z 1 H C
  fn sub_c(&mut self, _instr: u8) -> GbResult<()> {
    self.sub_r(self.bc.lo);
    Ok(())
  }

  /// SUB D
  ///
  /// Sub D from A and store into A
  ///
  /// Flags: Z 1 H C
  fn sub_d(&mut self, _instr: u8) -> GbResult<()> {
    self.sub_r(self.de.hi);
    Ok(())
  }

  /// SUB E
  ///
  /// Sub E from A and store into A
  ///
  /// Flags: Z 1 H C
  fn sub_e(&mut self, _instr: u8) -> GbResult<()> {
    self.sub_r(self.de.lo);
    Ok(())
  }

  /// SUB H
  ///
  /// Sub H from A and store into A
  ///
  /// Flags: Z 1 H C
  fn sub_h(&mut self, _instr: u8) -> GbResult<()> {
    self.sub_r(self.hl.hi);
    Ok(())
  }

  /// SUB L
  ///
  /// Sub L from A and store into A
  ///
  /// Flags: Z 1 H C
  fn sub_l(&mut self, _instr: u8) -> GbResult<()> {
    self.sub_r(self.hl.lo);
    Ok(())
  }

  /// SUB (HL)
  ///
  /// Sub val pointed to by HL and store into A
  ///
  /// Flags: Z 1 H C
  fn sub__hl_(&mut self, _instr: u8) -> GbResult<()> {
    let val = self.bus.lazy_dref().read8(self.hl.hilo())?;
    self.sub_r(val);
    Ok(())
  }

  /// SUB A
  ///
  /// Sub A from A and store into A
  ///
  /// Flags: Z 1 H C
  fn sub_a(&mut self, _instr: u8) -> GbResult<()> {
    self.sub_r(self.af.hi);
    Ok(())
  }

  /// SUB d8
  ///
  /// Sub imm8 from A and store into A
  ///
  /// Flags: Z 1 H C
  fn sub_d8(&mut self, _instr: u8) -> GbResult<()> {
    let d8 = self.get_imm8()?;
    self.sub_r(d8);
    Ok(())
  }

  /// SBC A B
  ///
  /// Sub B from A with carry and store into A
  ///
  /// Flags: Z 1 H C
  fn sbc_a_b(&mut self, _instr: u8) -> GbResult<()> {
    self.sbc_r(self.bc.hi);
    Ok(())
  }

  /// SBC A C
  ///
  /// Sub C from A with carry and store into A
  ///
  /// Flags: Z 1 H C
  fn sbc_a_c(&mut self, _instr: u8) -> GbResult<()> {
    self.sbc_r(self.bc.lo);
    Ok(())
  }

  /// SBC A D
  ///
  /// Sub D from A with carry and store into A
  ///
  /// Flags: Z 1 H C
  fn sbc_a_d(&mut self, _instr: u8) -> GbResult<()> {
    self.sbc_r(self.de.hi);
    Ok(())
  }

  /// SBC A E
  ///
  /// Sub E from A with carry and store into A
  ///
  /// Flags: Z 1 H C
  fn sbc_a_e(&mut self, _instr: u8) -> GbResult<()> {
    self.sbc_r(self.de.lo);
    Ok(())
  }

  /// SBC A H
  ///
  /// Sub H from A with carry and store into A
  ///
  /// Flags: Z 1 H C
  fn sbc_a_h(&mut self, _instr: u8) -> GbResult<()> {
    self.sbc_r(self.hl.hi);
    Ok(())
  }

  /// SBC A L
  ///
  /// Sub L from A with carry and store into A
  ///
  /// Flags: Z 1 H C
  fn sbc_a_l(&mut self, _instr: u8) -> GbResult<()> {
    self.sbc_r(self.hl.lo);
    Ok(())
  }

  /// SBC A (HL)
  ///
  /// Sub val pointed by HL with carry and store into A
  ///
  /// Flags: Z 1 H C
  fn sbc_a__hl_(&mut self, _instr: u8) -> GbResult<()> {
    let val = self.bus.lazy_dref().read8(self.hl.hilo())?;
    self.sbc_r(val);
    Ok(())
  }

  /// SBC A A
  ///
  /// Sub A from A with carry and store into A
  ///
  /// Flags: Z 1 H C
  fn sbc_a_a(&mut self, _instr: u8) -> GbResult<()> {
    self.sbc_r(self.af.hi);
    Ok(())
  }

  /// SBC A d8
  ///
  /// Sub imm8 from A and store into A
  ///
  /// Flags: Z 1 H C
  fn sbc_a_d8(&mut self, _instr: u8) -> GbResult<()> {
    let d8 = self.get_imm8()?;
    self.sbc_r(d8);
    Ok(())
  }

  /// AND B
  ///
  /// AND B with A and store into A
  ///
  /// Flags: Z 0 1 0
  fn and_b(&mut self, _instr: u8) -> GbResult<()> {
    self.and_r(self.bc.hi);
    Ok(())
  }

  /// AND C
  ///
  /// AND C with A and store into A
  ///
  /// Flags: Z 0 1 0
  fn and_c(&mut self, _instr: u8) -> GbResult<()> {
    self.and_r(self.bc.lo);
    Ok(())
  }

  /// AND D
  ///
  /// AND D with A and store into A
  ///
  /// Flags: Z 0 1 0
  fn and_d(&mut self, _instr: u8) -> GbResult<()> {
    self.and_r(self.de.hi);
    Ok(())
  }

  /// AND E
  ///
  /// AND E with A and store into A
  ///
  /// Flags: Z 0 1 0
  fn and_e(&mut self, _instr: u8) -> GbResult<()> {
    self.and_r(self.de.lo);
    Ok(())
  }

  /// AND H
  ///
  /// AND H with A and store into A
  ///
  /// Flags: Z 0 1 0
  fn and_h(&mut self, _instr: u8) -> GbResult<()> {
    self.and_r(self.hl.hi);
    Ok(())
  }

  /// AND L
  ///
  /// AND L with A and store into A
  ///
  /// Flags: Z 0 1 0
  fn and_l(&mut self, _instr: u8) -> GbResult<()> {
    self.and_r(self.hl.lo);
    Ok(())
  }

  /// AND (HL)
  ///
  /// AND val pointed by HL with A and store into A
  ///
  /// Flags: Z 0 1 0
  fn and__hl_(&mut self, _instr: u8) -> GbResult<()> {
    let val = self.bus.lazy_dref().read8(self.hl.hilo())?;
    self.and_r(val);
    Ok(())
  }

  /// AND A
  ///
  /// AND A with A and store into A
  ///
  /// Flags: Z 0 1 0
  fn and_a(&mut self, _instr: u8) -> GbResult<()> {
    self.and_r(self.af.hi);
    Ok(())
  }

  /// AND d8
  ///
  /// AND imm8 with A and store into A
  ///
  /// Flags: Z 0 1 0
  fn and_d8(&mut self, _instr: u8) -> GbResult<()> {
    let d8 = self.get_imm8()?;
    self.and_r(d8);
    Ok(())
  }

  /// XOR B
  ///
  /// XOR B with A and store into A
  ///
  /// Flags: Z 0 0 0
  fn xor_b(&mut self, _instr: u8) -> GbResult<()> {
    self.xor_r(self.bc.hi);
    Ok(())
  }

  /// XOR C
  ///
  /// XOR C with A and store into A
  ///
  /// Flags: Z 0 0 0
  fn xor_c(&mut self, _instr: u8) -> GbResult<()> {
    self.xor_r(self.bc.lo);
    Ok(())
  }

  /// XOR D
  ///
  /// XOR D with A and store into A
  ///
  /// Flags: Z 0 0 0
  fn xor_d(&mut self, _instr: u8) -> GbResult<()> {
    self.xor_r(self.de.hi);
    Ok(())
  }

  /// XOR E
  ///
  /// XOR E with A and store into A
  ///
  /// Flags: Z 0 0 0
  fn xor_e(&mut self, _instr: u8) -> GbResult<()> {
    self.xor_r(self.de.lo);
    Ok(())
  }

  /// XOR H
  ///
  /// XOR H with A and store into A
  ///
  /// Flags: Z 0 0 0
  fn xor_h(&mut self, _instr: u8) -> GbResult<()> {
    self.xor_r(self.hl.hi);
    Ok(())
  }

  /// XOR L
  ///
  /// XOR L with A and store into A
  ///
  /// Flags: Z 0 0 0
  fn xor_l(&mut self, _instr: u8) -> GbResult<()> {
    self.xor_r(self.hl.lo);
    Ok(())
  }

  /// XOR (HL)
  ///
  /// XOR val pointed by HL with A and store into A
  ///
  /// Flags: Z 0 0 0
  fn xor__hl_(&mut self, _instr: u8) -> GbResult<()> {
    let val = self.bus.lazy_dref().read8(self.hl.hilo())?;
    self.xor_r(val);
    Ok(())
  }

  /// XOR A
  ///
  /// XOR A with A and store into A
  ///
  /// Flags Z 0 0 0
  fn xor_a(&mut self, _instr: u8) -> GbResult<()> {
    self.xor_r(self.af.hi);
    Ok(())
  }

  /// XOR d8
  ///
  /// XOR imm8 with A and store into A
  ///
  /// Flags: Z 0 0 0
  fn xor_d8(&mut self, _instr: u8) -> GbResult<()> {
    let d8 = self.get_imm8()?;
    self.xor_r(d8);
    Ok(())
  }

  /// OR B
  ///
  /// OR B with A and store into A
  ///
  /// Flags: Z 0 0 0
  fn or_b(&mut self, _instr: u8) -> GbResult<()> {
    self.or_r(self.bc.hi);
    Ok(())
  }

  /// OR C
  ///
  /// OR C with A and store into A
  ///
  /// Flags: Z 0 0 0
  fn or_c(&mut self, _instr: u8) -> GbResult<()> {
    self.or_r(self.bc.lo);
    Ok(())
  }

  /// OR D
  ///
  /// OR D with A and store into A
  ///
  /// Flags: Z 0 0 0
  fn or_d(&mut self, _instr: u8) -> GbResult<()> {
    self.or_r(self.de.hi);
    Ok(())
  }

  /// OR E
  ///
  /// OR E with A and store into A
  ///
  /// Flags: Z 0 0 0
  fn or_e(&mut self, _instr: u8) -> GbResult<()> {
    self.or_r(self.de.lo);
    Ok(())
  }

  /// OR H
  ///
  /// OR H with A and store into A
  ///
  /// Flags: Z 0 0 0
  fn or_h(&mut self, _instr: u8) -> GbResult<()> {
    self.or_r(self.hl.hi);
    Ok(())
  }

  /// OR L
  ///
  /// OR L with A and store into A
  ///
  /// Flags: Z 0 0 0
  fn or_l(&mut self, _instr: u8) -> GbResult<()> {
    self.or_r(self.hl.lo);
    Ok(())
  }

  /// OR (HL)
  ///
  /// OR val pointed by HL with A and store into A
  ///
  /// Flags: Z 0 0 0
  fn or__hl_(&mut self, _instr: u8) -> GbResult<()> {
    let val = self.bus.lazy_dref().read8(self.hl.hilo())?;
    self.or_r(val);
    Ok(())
  }

  /// OR A
  ///
  /// OR A with A and store into A
  ///
  /// Flags: Z 0 0 0
  fn or_a(&mut self, _instr: u8) -> GbResult<()> {
    self.or_r(self.af.hi);
    Ok(())
  }

  /// OR d8
  ///
  /// OR imm8 with A and store into A
  ///
  /// Flags: Z 0 0 0
  fn or_d8(&mut self, _instr: u8) -> GbResult<()> {
    let d8 = self.get_imm8()?;
    self.or_r(d8);
    Ok(())
  }

  /// CP B
  ///
  /// Compare B with A
  ///
  /// Flags: Z 1 H C
  fn cp_b(&mut self, _instr: u8) -> GbResult<()> {
    self.cp_r(self.bc.hi);
    Ok(())
  }

  /// CP C
  ///
  /// Compare C with A
  ///
  /// Flags: Z 1 H C
  fn cp_c(&mut self, _instr: u8) -> GbResult<()> {
    self.cp_r(self.bc.lo);
    Ok(())
  }

  /// CP D
  ///
  /// Compare D with A
  ///
  /// Flags: Z 1 H C
  fn cp_d(&mut self, _instr: u8) -> GbResult<()> {
    self.cp_r(self.de.hi);
    Ok(())
  }

  /// CP E
  ///
  /// Compare E with A
  ///
  /// Flags: Z 1 H C
  fn cp_e(&mut self, _instr: u8) -> GbResult<()> {
    self.cp_r(self.de.lo);
    Ok(())
  }

  /// CP H
  ///
  /// Compare H with A
  ///
  /// Flags: Z 1 H C
  fn cp_h(&mut self, _instr: u8) -> GbResult<()> {
    self.cp_r(self.hl.hi);
    Ok(())
  }

  /// CP L
  ///
  /// Compare L with A
  ///
  /// Flags: Z 1 H C
  fn cp_l(&mut self, _instr: u8) -> GbResult<()> {
    self.cp_r(self.hl.lo);
    Ok(())
  }

  /// CP (HL)
  ///
  /// Compare val pointed by HL with A
  ///
  /// Flags: Z 1 H C
  fn cp__hl_(&mut self, _instr: u8) -> GbResult<()> {
    let val = self.bus.lazy_dref().read8(self.hl.hilo())?;
    self.cp_r(val);
    Ok(())
  }

  /// CP A
  ///
  /// Compare A with A
  ///
  /// Flags: Z 1 H C
  fn cp_a(&mut self, _instr: u8) -> GbResult<()> {
    self.cp_r(self.af.hi);
    Ok(())
  }

  /// CP d8
  ///
  /// Compare imm8 with A
  ///
  /// Flags: Z 1 H C
  fn cp_d8(&mut self, _instr: u8) -> GbResult<()> {
    let d8 = self.get_imm8()?;
    self.cp_r(d8);
    Ok(())
  }

  /// RLCA
  ///
  /// Rotate A register Left
  ///
  /// Flags: 0 0 0 C
  fn rlca(&mut self, _instr: u8) -> GbResult<()> {
    // reset flags
    self.af.lo = 0;
    let bit7 = self.af.hi & 0x80;
    let carry = if bit7 > 0 { FLAG_C } else { 0 };

    self.af.hi <<= 1;
    self.af.hi |= bit7 >> 7;

    // set carry flag
    self.af.lo |= carry;

    Ok(())
  }

  /// RRCA
  ///
  /// Rotate A register right
  ///
  /// Flags: 0 0 0 C
  fn rrca(&mut self, _instr: u8) -> GbResult<()> {
    // reset flags
    self.af.lo = 0;
    let bit0 = self.af.hi & 0x01;
    let carry = if bit0 > 0 { FLAG_C } else { 0 };

    self.af.hi >>= 1;
    self.af.hi |= bit0 << 7;

    // set carry flag
    self.af.lo |= carry;

    Ok(())
  }

  /// RLA
  ///
  /// Rotate A register left through carry
  ///
  /// Flags: 0 0 0 C
  fn rla(&mut self, _instr: u8) -> GbResult<()> {
    let bit_carry = (self.af.lo & FLAG_C > 0) as u8;
    // reset flags
    self.af.lo = 0;
    let bit7 = self.af.hi & 0x80;
    let carry = if bit7 > 0 { FLAG_C } else { 0 };

    self.af.hi <<= 1;
    self.af.hi |= bit_carry;

    // set carry flag
    self.af.lo |= carry;

    Ok(())
  }

  /// RRA
  ///
  /// Rotate A register right through carry
  ///
  /// Flags: 0 0 0 C
  fn rra(&mut self, _instr: u8) -> GbResult<()> {
    let bit_carry = (self.af.lo & FLAG_C > 0) as u8;
    // reset flags
    self.af.lo = 0;
    let bit0 = self.af.hi & 0x01;
    let carry = if bit0 > 0 { FLAG_C } else { 0 };

    self.af.hi >>= 1;
    self.af.hi |= bit_carry << 7;

    // set carry flag
    self.af.lo |= carry;

    Ok(())
  }

  /// DAA
  ///
  /// Decimal adjust A
  ///
  /// Flags: Z - 0 C
  fn daa(&mut self, _instr: u8) -> GbResult<()> {
    todo!("what is DAA?")
  }

  /// CPL
  ///
  /// Compliment of A
  ///
  /// Flags: 0 1 1 0
  fn cpl(&mut self, _instr: u8) -> GbResult<()> {
    self.af.lo = (FLAG_N | FLAG_H);
    self.af.hi ^= 0xff;
    Ok(())
  }

  /// SCF
  ///
  /// Set carry flag
  ///
  /// Flags: - 0 0 1
  fn scf(&mut self, _instr: u8) -> GbResult<()> {
    // only keep Z flag
    self.af.lo &= FLAG_Z;
    self.af.lo |= FLAG_C;
    Ok(())
  }

  /// CCF
  ///
  /// Toggle carry flag
  ///
  /// Flags: - 0 0 C
  fn ccf(&mut self, _instr: u8) -> GbResult<()> {
    // only keep Z flags
    self.af.lo &= FLAG_Z;
    self.af.lo ^= FLAG_C;
    Ok(())
  }

  // *** Branch/Jumps ***

  fn jr_flag_r8(&mut self, flag: u8, test_set: bool) -> GbResult<()> {
    let r8 = self.get_imm8()? as i8;
    if (test_set && (self.af.lo & flag != 0)) || (!test_set && (self.af.lo & flag == 0)) {
      // undo pc changes
      self.pc -= 2;
      // now jump!
      self.pc = self.pc.wrapping_add_signed(r8 as i16);
    }
    Ok(())
  }

  fn jp_flag_a16(&mut self, flag: u8, test_set: bool) -> GbResult<()> {
    let a16 = self.get_imm16()?;
    if (test_set && (self.af.lo & flag != 0)) || (!test_set && (self.af.lo & flag == 0)) {
      // now jump!
      self.pc = a16;
    }
    Ok(())
  }

  fn call(&mut self, a16: u16) -> GbResult<()> {
    self.sp = self.sp.wrapping_sub(2);
    self.bus.lazy_dref_mut().write16(self.sp, self.pc)?;
    self.pc = a16;
    Ok(())
  }

  fn call_flag_a16(&mut self, flag: u8, test_set: bool) -> GbResult<()> {
    let a16 = self.get_imm16()?;
    if (test_set && (self.af.lo & flag != 0)) || (!test_set && (self.af.lo & flag == 0)) {
      // now jump!
      self.call(a16)?;
    }
    Ok(())
  }

  fn ret_flag(&mut self, flag: u8, test_set: bool) -> GbResult<()> {
    if (test_set && (self.af.lo & flag != 0)) || (!test_set && (self.af.lo & flag == 0)) {
      self.pc = self.bus.lazy_dref().read16(self.sp)?;
      self.sp = self.sp.wrapping_add(2);
    }
    Ok(())
  }

  /// JR r8
  ///
  /// Jump to PC + r8 (signed)
  ///
  /// Flags: - - - -
  fn jr_r8(&mut self, _instr: u8) -> GbResult<()> {
    // always jump
    self.jr_flag_r8(0, false)?;
    Ok(())
  }

  /// JR NZ r8
  ///
  /// jump to PC + r8 (signed) if Z flag cleared
  ///
  /// Flags: - - - -
  fn jr_nz_r8(&mut self, _instr: u8) -> GbResult<()> {
    self.jr_flag_r8(FLAG_Z, false)?;
    Ok(())
  }

  /// JR Z r8
  ///
  /// jump to PC + r8 (signed) if Z flag set
  ///
  /// Flags: - - - -
  fn jr_z_r8(&mut self, _instr: u8) -> GbResult<()> {
    self.jr_flag_r8(FLAG_Z, true)?;
    Ok(())
  }

  /// JR NC r8
  ///
  /// jump to PC + r8 (signed) if C flag cleared
  ///
  /// Flags: - - - -
  fn jr_nc_r8(&mut self, _instr: u8) -> GbResult<()> {
    self.jr_flag_r8(FLAG_C, false)?;
    Ok(())
  }

  /// JR C r8
  ///
  /// jump to PC + r8 (signed) if C flag set
  ///
  /// Flags: - - - -
  fn jr_c_r8(&mut self, _instr: u8) -> GbResult<()> {
    self.jr_flag_r8(FLAG_C, true)?;
    Ok(())
  }

  /// JP NZ a16
  ///
  /// jump to imm16 if Z flag cleared
  ///
  /// Flags: - - - -
  fn jp_nz_a16(&mut self, _instr: u8) -> GbResult<()> {
    self.jp_flag_a16(FLAG_Z, false)?;
    Ok(())
  }

  /// JP a16
  ///
  /// jump to imm16
  ///
  /// Flags: - - - -
  fn jp_a16(&mut self, _instr: u8) -> GbResult<()> {
    // always jump
    self.jp_flag_a16(0, false)?;
    Ok(())
  }

  /// JP Z a16
  ///
  /// jump to imm16 if Z flag set
  ///
  /// Flags: - - - -
  fn jp_z_a16(&mut self, _instr: u8) -> GbResult<()> {
    self.jp_flag_a16(FLAG_Z, true)?;
    Ok(())
  }

  /// JP NC a16
  ///
  /// jump to imm16 if C flag cleared
  ///
  /// Flags: - - - -
  fn jp_nc_a16(&mut self, _instr: u8) -> GbResult<()> {
    self.jp_flag_a16(FLAG_C, false)?;
    Ok(())
  }

  /// JP C a16
  ///
  /// jump to imm16 if C flag set
  ///
  /// Flags: - - - -
  fn jp_c_a16(&mut self, _instr: u8) -> GbResult<()> {
    self.jp_flag_a16(FLAG_C, true)?;
    Ok(())
  }

  /// JP (HL)
  ///
  /// jump to address held by HL
  ///
  /// Flags: - - - -
  fn jp__hl_(&mut self, _instr: u8) -> GbResult<()> {
    self.pc = self.hl.hilo();
    Ok(())
  }

  /// CALL NZ a16
  ///
  /// Call routine at a16 if Z flag cleared
  ///
  /// Flags: - - - -
  fn call_nz_a16(&mut self, _instr: u8) -> GbResult<()> {
    self.call_flag_a16(FLAG_Z, false)?;
    Ok(())
  }

  /// CALL Z a16
  ///
  /// Call routine at a16 if Z flag set
  ///
  /// Flags: - - - -
  fn call_z_a16(&mut self, _instr: u8) -> GbResult<()> {
    self.call_flag_a16(FLAG_Z, true)?;
    Ok(())
  }

  /// CALL a16
  ///
  /// Call routine at a16
  ///
  /// Flags: - - - -
  fn call_a16(&mut self, _instr: u8) -> GbResult<()> {
    // always jump
    self.call_flag_a16(0, false)?;
    Ok(())
  }

  /// CALL NC a16
  ///
  /// Call routine at a16 if C flag cleared
  ///
  /// Flags: - - - -
  fn call_nc_a16(&mut self, _instr: u8) -> GbResult<()> {
    self.call_flag_a16(FLAG_C, false)?;
    Ok(())
  }

  /// CALL C a16
  ///
  /// Call routine at a16 if C flag set
  ///
  /// Flags: - - - -
  fn call_c_a16(&mut self, _instr: u8) -> GbResult<()> {
    self.call_flag_a16(FLAG_C, true)?;
    Ok(())
  }

  /// RST 00h
  ///
  /// Call to 00h
  ///
  /// Flags: - - - -
  fn rst_00h(&mut self, _instr: u8) -> GbResult<()> {
    self.call(0x00)?;
    Ok(())
  }

  /// RST 08h
  ///
  /// Call to 08h
  ///
  /// Flags: - - - -
  fn rst_08h(&mut self, _instr: u8) -> GbResult<()> {
    self.call(0x08)?;
    Ok(())
  }

  /// RST 10h
  ///
  /// Call to 10h
  ///
  /// Flags: - - - -
  fn rst_10h(&mut self, _instr: u8) -> GbResult<()> {
    self.call(0x10)?;
    Ok(())
  }

  /// RST 18h
  ///
  /// Call to 18h
  ///
  /// Flags: - - - -
  fn rst_18h(&mut self, _instr: u8) -> GbResult<()> {
    self.call(0x18)?;
    Ok(())
  }

  /// RST 20h
  ///
  /// Call to 20h
  ///
  /// Flags: - - - -
  fn rst_20h(&mut self, _instr: u8) -> GbResult<()> {
    self.call(0x20)?;
    Ok(())
  }

  /// RST 28h
  ///
  /// Call to 28h
  ///
  /// Flags: - - - -
  fn rst_28h(&mut self, _instr: u8) -> GbResult<()> {
    self.call(0x28)?;
    Ok(())
  }

  /// RST 30h
  ///
  /// Call to 30h
  ///
  /// Flags: - - - -
  fn rst_30h(&mut self, _instr: u8) -> GbResult<()> {
    self.call(0x30)?;
    Ok(())
  }

  /// RST 38h
  ///
  /// Call to 38h
  ///
  /// Flags: - - - -
  fn rst_38h(&mut self, _instr: u8) -> GbResult<()> {
    self.call(0x38)?;
    Ok(())
  }

  /// RET
  ///
  /// Return from subroutine
  ///
  /// Flags: - - - -
  fn ret(&mut self, _instr: u8) -> GbResult<()> {
    // always ret
    self.ret_flag(0, false)
  }

  /// RET NZ
  ///
  /// Return from subroutine if Z flag cleared
  ///
  /// Flags: - - - -
  fn req_nz(&mut self, _instr: u8) -> GbResult<()> {
    self.ret_flag(FLAG_Z, false)
  }

  /// RET Z
  ///
  /// Return from subroutine if Z flag set
  ///
  /// Flags: - - - -
  fn ret_z(&mut self, _instr: u8) -> GbResult<()> {
    self.ret_flag(FLAG_Z, true)
  }

  /// RET NC
  ///
  /// Return from subroutine if C flag cleared
  ///
  /// Flags: - - - -
  fn ret_nc(&mut self, _instr: u8) -> GbResult<()> {
    self.ret_flag(FLAG_C, false)
  }

  /// RET C
  ///
  /// Return from subroutine if C flag set
  ///
  /// Flags: - - - -
  fn ret_c(&mut self, _instr: u8) -> GbResult<()> {
    self.ret_flag(FLAG_C, true)
  }

  /// RETI
  ///
  /// Return and enable interrupts
  ///
  /// Flags: - - - -
  fn reti(&mut self, _instr: u8) -> GbResult<()> {
    todo!("Implement interrupts")
  }

  // *** Other ***

  fn pop(&mut self) -> GbResult<u16> {
    let val = self.bus.lazy_dref().read16(self.sp)?;
    self.sp = self.sp.wrapping_add(2);
    Ok(val)
  }

  fn push(&mut self, rr: u16) -> GbResult<()> {
    self.sp = self.sp.wrapping_sub(2);
    self.bus.lazy_dref_mut().write16(self.sp, rr)
  }

  /// POP BC
  ///
  /// Pop from the stack and store into BC
  ///
  /// Flags: - - - -
  fn pop_bc(&mut self, _instr: u8) -> GbResult<()> {
    let val = self.pop()?;
    self.bc.set_u16(val);
    Ok(())
  }

  /// POP DE
  ///
  /// Pop from the stack and store into DE
  ///
  /// Flags: - - - -
  fn pop_de(&mut self, _instr: u8) -> GbResult<()> {
    let val = self.pop()?;
    self.de.set_u16(val);
    Ok(())
  }

  /// POP HL
  ///
  /// Pop from the stack and store into HL
  ///
  /// Flags: - - - -
  fn pop_hl(&mut self, _instr: u8) -> GbResult<()> {
    let val = self.pop()?;
    self.hl.set_u16(val);
    Ok(())
  }

  /// POP AF
  ///
  /// Pop from the stack and store into AF
  ///
  /// Flags: Z N H C
  fn pop_af(&mut self, _instr: u8) -> GbResult<()> {
    let val = self.pop()?;
    self.af.set_u16(val);
    Ok(())
  }

  /// PUSH BC
  ///
  /// Push BC to the stack
  ///
  /// Flags: - - - -
  fn push_bc(&mut self, _instr: u8) -> GbResult<()> {
    self.push(self.bc.hilo())
  }

  /// PUSH DE
  ///
  /// Push DE to the stack
  ///
  /// Flags: - - - -
  fn push_de(&mut self, _instr: u8) -> GbResult<()> {
    self.push(self.de.hilo())
  }

  /// PUSH HL
  ///
  /// Push HL to the stack
  ///
  /// Flags: - - - -
  fn push_hl(&mut self, _instr: u8) -> GbResult<()> {
    self.push(self.hl.hilo())
  }

  /// PUSH AF
  ///
  /// Push AF to the stack
  ///
  /// Flags: - - - -
  fn push_af(&mut self, _instr: u8) -> GbResult<()> {
    self.push(self.af.hilo())
  }

  /// DI
  ///
  /// Disable Interrupts
  ///
  /// Flags: - - - -
  fn di(&mut self, _instr: u8) -> GbResult<()> {
    todo!("Implement interrupts")
  }

  /// EI
  ///
  /// Enable Interrupts
  ///
  /// Flags: - - - -
  fn ei(&mut self, _instr: u8) -> GbResult<()> {
    todo!("Implement interrupts")
  }

  // *** Prefix CB ***

  fn rlc_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rlc_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rlc_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rlc_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rlc_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rlc_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rlc__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rlc_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rrc_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rrc_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rrc_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rrc_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rrc_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rrc_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rrc__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rrc_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rl_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rl_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rl_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rl_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rl_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rl_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rl__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rl_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rr_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rr_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rr_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rr_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rr_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rr_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rr__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn rr_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sla_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sla_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sla_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sla_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sla_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sla_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sla__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sla_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sra_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sra_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sra_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sra_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sra_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sra_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sra__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn sra_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn swap_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn swap_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn swap_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn swap_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn swap_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn swap_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn swap__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn swap_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn srl_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn srl_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn srl_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn srl_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn srl_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn srl_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn srl__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn srl_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_0_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_0_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_0_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_0_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_0_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_0_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_0__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_0_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_1_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_1_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_1_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_1_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_1_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_1_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_1__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_1_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_2_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_2_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_2_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_2_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_2_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_2_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_2__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_2_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_3_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_3_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_3_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_3_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_3_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_3_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_3__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_3_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_4_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_4_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_4_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_4_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_4_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_4_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_4__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_4_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_5_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_5_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_5_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_5_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_5_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_5_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_5__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_5_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_6_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_6_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_6_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_6_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_6_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_6_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_6__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_6_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_7_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_7_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_7_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_7_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_7_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_7_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_7__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn bit_7_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_0_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_0_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_0_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_0_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_0_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_0_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_0__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_0_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_1_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_1_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_1_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_1_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_1_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_1_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_1__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_1_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_2_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_2_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_2_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_2_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_2_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_2_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_2__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_2_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_3_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_3_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_3_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_3_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_3_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_3_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_3__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_3_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_4_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_4_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_4_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_4_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_4_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_4_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_4__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_4_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_5_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_5_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_5_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_5_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_5_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_5_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_5__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_5_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_6_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_6_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_6_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_6_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_6_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_6_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_6__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_6_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_7_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_7_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_7_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_7_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_7_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_7_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_7__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn res_7_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_0_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_0_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_0_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_0_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_0_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_0_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_0__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_0_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_1_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_1_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_1_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_1_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_1_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_1_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_1__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_1_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_2_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_2_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_2_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_2_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_2_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_2_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_2__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_2_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_3_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_3_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_3_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_3_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_3_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_3_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_3__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_3_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_4_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_4_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_4_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_4_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_4_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_4_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_4__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_4_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_5_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_5_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_5_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_5_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_5_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_5_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_5__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_5_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_6_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_6_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_6_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_6_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_6_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_6_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_6__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_6_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_7_b(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_7_c(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_7_d(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_7_e(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_7_h(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_7_l(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_7__hl_(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }

  fn set_7_a(&mut self, _instr: u8) -> GbResult<()> {
    unimplemented!()
  }
}
