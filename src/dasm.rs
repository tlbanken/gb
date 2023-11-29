//! Disassembler for the Gameboy cpu. This can be used for displaying debug
//! info.

use crate::err::GbResult;

const PREFIX_CB_OP: u8 = 0xcb;

#[derive(Clone, Copy)]
/// Info to describe the immediate name and the argument placement.
enum ImmInfo {
  D8,
  D16,
  A8,
  A16,
  R8,
}

#[derive(Clone, Copy)]
struct InstrEntry {
  name: &'static str,
  size: u32,
  info: Option<ImmInfo>,
}

struct InstrDesc {
  bytes: Vec<u8>,
}

impl InstrDesc {
  pub fn new() -> Self {
    Self { bytes: Vec::new() }
  }

  pub fn push(&mut self, byte: u8) {
    self.bytes.push(byte);
  }

  pub fn clear(&mut self) {
    self.bytes.clear();
  }

  pub fn d8(&self) -> u8 {
    self.bytes[1]
  }

  pub fn d16(&self) -> u16 {
    // construct as LE
    (self.bytes[2] as u16) << 8 | (self.bytes[1] as u16)
  }

  pub fn a8(&self) -> u8 {
    self.d8()
  }

  pub fn a16(&self) -> u16 {
    self.d16()
  }

  pub fn r8(&self) -> i8 {
    self.d8() as i8
  }
}

/// The disassembler
pub struct Dasm {
  bytes_left: u32,
  name: String,
  index: u8,
  imm16: u16,
  imm_info: Option<ImmInfo>,
  instr_entries: Vec<InstrEntry>,
  instr_cb_entries: Vec<InstrEntry>,
  instr_desc: InstrDesc,
  cb_mode: bool,
}

impl Dasm {
  pub fn new() -> Dasm {
    Dasm {
      bytes_left: 0,
      name: String::new(),
      index: 0,
      imm16: 0,
      imm_info: None,
      instr_entries: Self::build_instr_entry_table(),
      instr_cb_entries: Self::build_instr_cb_entry_table(),
      instr_desc: InstrDesc::new(),
      cb_mode: false,
    }
  }

  pub fn munch(&mut self, byte: u8) -> Option<String> {
    // cb instructions are a special case
    if self.cb_mode {
      let entry = &self.instr_cb_entries[byte as usize];
      self.cb_mode = false;
      // we should have already consumed the "cb" byte. Now just return the name since
      // all cb instructions are 2 bytes long.
      return Some(String::from(entry.name));
    }

    if self.bytes_left == 0 {
      // new instruction start
      if byte == PREFIX_CB_OP {
        self.cb_mode = true;
        // need next byte to start decoding
        return None;
      }

      let entry = &self.instr_entries[byte as usize];

      // initialize new state from entry
      self.instr_desc.clear();
      self.name = String::from(entry.name);
      self.imm16 = 0;
      self.bytes_left = entry.size;
      self.imm_info = entry.info;
    }

    // update state
    self.instr_desc.push(byte);
    self.bytes_left -= 1;

    if self.bytes_left == 0 {
      return Some(match self.imm_info {
        None => self.name.clone(),
        Some(info) => match info {
          ImmInfo::D8 => self
            .name
            .replace("d8", format!("{}", self.instr_desc.d8()).as_str()),
          ImmInfo::D16 => self
            .name
            .replace("d16", format!("{}", self.instr_desc.d16()).as_str()),
          ImmInfo::A8 => self
            .name
            .replace("a8", format!("${:02X}", self.instr_desc.a8()).as_str()),
          ImmInfo::A16 => self
            .name
            .replace("a16", format!("${:04X}", self.instr_desc.a16()).as_str()),
          ImmInfo::R8 => self
            .name
            .replace("r8", format!("{}", self.instr_desc.r8()).as_str()),
        },
      });
    }
    None
  }

  fn build_instr_entry_table() -> Vec<InstrEntry> {
    use ImmInfo::*;
    vec![
      /* 00 */ InstrEntry {
        name: "nop",
        size: 1,
        info: None,
      },
      /* 01 */
      InstrEntry {
        name: "ld bc d16",
        size: 3,
        info: Some(D16),
      },
      /* 02 */
      InstrEntry {
        name: "ld (bc) a",
        size: 1,
        info: None,
      },
      /* 03 */ InstrEntry {
        name: "inc bc",
        size: 1,
        info: None,
      },
      /* 04 */ InstrEntry {
        name: "inc b",
        size: 1,
        info: None,
      },
      /* 05 */ InstrEntry {
        name: "dec b",
        size: 1,
        info: None,
      },
      /* 06 */
      InstrEntry {
        name: "ld b d8",
        size: 2,
        info: Some(D8),
      },
      /* 07 */ InstrEntry {
        name: "rlca",
        size: 1,
        info: None,
      },
      /* 08 */
      InstrEntry {
        name: "ld (a16) sp",
        size: 3,
        info: Some(A16),
      },
      /* 09 */
      InstrEntry {
        name: "add hl bc",
        size: 1,
        info: None,
      },
      /* 0A */
      InstrEntry {
        name: "ld a (bc)",
        size: 1,
        info: None,
      },
      /* 0B */ InstrEntry {
        name: "dec bc",
        size: 1,
        info: None,
      },
      /* 0C */ InstrEntry {
        name: "inc c",
        size: 1,
        info: None,
      },
      /* 0D */ InstrEntry {
        name: "dec c",
        size: 1,
        info: None,
      },
      /* 0E */
      InstrEntry {
        name: "ld c d8",
        size: 2,
        info: Some(D8),
      },
      /* 0F */ InstrEntry {
        name: "rrca",
        size: 1,
        info: None,
      },
      /* 10 */ InstrEntry {
        name: "stop",
        size: 2,
        info: None,
      },
      /* 11 */
      InstrEntry {
        name: "ld de d16",
        size: 3,
        info: Some(D16),
      },
      /* 12 */
      InstrEntry {
        name: "ld (de) a",
        size: 1,
        info: None,
      },
      /* 13 */ InstrEntry {
        name: "inc de",
        size: 1,
        info: None,
      },
      /* 14 */ InstrEntry {
        name: "inc d",
        size: 1,
        info: None,
      },
      /* 15 */ InstrEntry {
        name: "dec d",
        size: 1,
        info: None,
      },
      /* 16 */
      InstrEntry {
        name: "ld d d8",
        size: 2,
        info: Some(D8),
      },
      /* 17 */ InstrEntry {
        name: "rla",
        size: 1,
        info: None,
      },
      /* 18 */
      InstrEntry {
        name: "jr r8",
        size: 2,
        info: Some(R8),
      },
      /* 19 */
      InstrEntry {
        name: "add hl de",
        size: 1,
        info: None,
      },
      /* 1A */
      InstrEntry {
        name: "ld a (de)",
        size: 1,
        info: None,
      },
      /* 1B */ InstrEntry {
        name: "dec de",
        size: 1,
        info: None,
      },
      /* 1C */ InstrEntry {
        name: "inc e",
        size: 1,
        info: None,
      },
      /* 1D */ InstrEntry {
        name: "dec e",
        size: 1,
        info: None,
      },
      /* 1E */
      InstrEntry {
        name: "ld e d8",
        size: 2,
        info: Some(D8),
      },
      /* 1F */ InstrEntry {
        name: "rra",
        size: 1,
        info: None,
      },
      /* 20 */
      InstrEntry {
        name: "jr nz r8",
        size: 2,
        info: Some(R8),
      },
      /* 21 */
      InstrEntry {
        name: "ld hl d16",
        size: 3,
        info: Some(D16),
      },
      /* 22 */
      InstrEntry {
        name: "ld (hl+) a",
        size: 1,
        info: None,
      },
      /* 23 */ InstrEntry {
        name: "inc hl",
        size: 1,
        info: None,
      },
      /* 24 */ InstrEntry {
        name: "inc h",
        size: 1,
        info: None,
      },
      /* 25 */ InstrEntry {
        name: "dec h",
        size: 1,
        info: None,
      },
      /* 26 */
      InstrEntry {
        name: "ld h d8",
        size: 2,
        info: Some(D8),
      },
      /* 27 */ InstrEntry {
        name: "daa",
        size: 1,
        info: None,
      },
      /* 28 */
      InstrEntry {
        name: "jr z r8",
        size: 2,
        info: Some(R8),
      },
      /* 29 */
      InstrEntry {
        name: "add hl hl",
        size: 1,
        info: None,
      },
      /* 2A */
      InstrEntry {
        name: "ld a (hl+)",
        size: 1,
        info: None,
      },
      /* 2B */ InstrEntry {
        name: "dec hl",
        size: 1,
        info: None,
      },
      /* 2C */ InstrEntry {
        name: "inc l",
        size: 1,
        info: None,
      },
      /* 2D */ InstrEntry {
        name: "dec l",
        size: 1,
        info: None,
      },
      /* 2E */
      InstrEntry {
        name: "ld l d8",
        size: 2,
        info: Some(D8),
      },
      /* 2F */ InstrEntry {
        name: "cpl",
        size: 1,
        info: None,
      },
      /* 30 */
      InstrEntry {
        name: "jr nc r8",
        size: 2,
        info: Some(R8),
      },
      /* 31 */
      InstrEntry {
        name: "ld sp d16",
        size: 3,
        info: Some(D16),
      },
      /* 32 */
      InstrEntry {
        name: "ld (hl-) a",
        size: 1,
        info: None,
      },
      /* 33 */ InstrEntry {
        name: "inc sp",
        size: 1,
        info: None,
      },
      /* 34 */
      InstrEntry {
        name: "inc (hl)",
        size: 1,
        info: None,
      },
      /* 35 */
      InstrEntry {
        name: "dec (hl)",
        size: 1,
        info: None,
      },
      /* 36 */
      InstrEntry {
        name: "ld (hl) d8",
        size: 2,
        info: Some(D8),
      },
      /* 37 */ InstrEntry {
        name: "scf",
        size: 1,
        info: None,
      },
      /* 38 */
      InstrEntry {
        name: "jr c r8",
        size: 2,
        info: Some(R8),
      },
      /* 39 */
      InstrEntry {
        name: "add hl sp",
        size: 1,
        info: None,
      },
      /* 3A */
      InstrEntry {
        name: "ld a (hl-)",
        size: 1,
        info: None,
      },
      /* 3B */ InstrEntry {
        name: "dec sp",
        size: 1,
        info: None,
      },
      /* 3C */ InstrEntry {
        name: "inc a",
        size: 1,
        info: None,
      },
      /* 3D */ InstrEntry {
        name: "dec a",
        size: 1,
        info: None,
      },
      /* 3E */
      InstrEntry {
        name: "ld a d8",
        size: 2,
        info: Some(D8),
      },
      /* 3F */ InstrEntry {
        name: "ccf",
        size: 1,
        info: None,
      },
      /* 40 */ InstrEntry {
        name: "ld b b",
        size: 1,
        info: None,
      },
      /* 41 */ InstrEntry {
        name: "ld b c",
        size: 1,
        info: None,
      },
      /* 42 */ InstrEntry {
        name: "ld b d",
        size: 1,
        info: None,
      },
      /* 43 */ InstrEntry {
        name: "ld b e",
        size: 1,
        info: None,
      },
      /* 44 */ InstrEntry {
        name: "ld b h",
        size: 1,
        info: None,
      },
      /* 45 */ InstrEntry {
        name: "ld b l",
        size: 1,
        info: None,
      },
      /* 46 */
      InstrEntry {
        name: "ld b (hl)",
        size: 1,
        info: None,
      },
      /* 47 */ InstrEntry {
        name: "ld b a",
        size: 1,
        info: None,
      },
      /* 48 */ InstrEntry {
        name: "ld c b",
        size: 1,
        info: None,
      },
      /* 49 */ InstrEntry {
        name: "ld c c",
        size: 1,
        info: None,
      },
      /* 4A */ InstrEntry {
        name: "ld c d",
        size: 1,
        info: None,
      },
      /* 4B */ InstrEntry {
        name: "ld c e",
        size: 1,
        info: None,
      },
      /* 4C */ InstrEntry {
        name: "ld c h",
        size: 1,
        info: None,
      },
      /* 4D */ InstrEntry {
        name: "ld c l",
        size: 1,
        info: None,
      },
      /* 4E */
      InstrEntry {
        name: "ld c (hl)",
        size: 1,
        info: None,
      },
      /* 4F */ InstrEntry {
        name: "ld c a",
        size: 1,
        info: None,
      },
      /* 50 */ InstrEntry {
        name: "ld d b",
        size: 1,
        info: None,
      },
      /* 51 */ InstrEntry {
        name: "ld d c",
        size: 1,
        info: None,
      },
      /* 52 */ InstrEntry {
        name: "ld d d",
        size: 1,
        info: None,
      },
      /* 53 */ InstrEntry {
        name: "ld d e",
        size: 1,
        info: None,
      },
      /* 54 */ InstrEntry {
        name: "ld d h",
        size: 1,
        info: None,
      },
      /* 55 */ InstrEntry {
        name: "ld d l",
        size: 1,
        info: None,
      },
      /* 56 */
      InstrEntry {
        name: "ld d (hl)",
        size: 1,
        info: None,
      },
      /* 57 */ InstrEntry {
        name: "ld d a",
        size: 1,
        info: None,
      },
      /* 58 */ InstrEntry {
        name: "ld e b",
        size: 1,
        info: None,
      },
      /* 59 */ InstrEntry {
        name: "ld e c",
        size: 1,
        info: None,
      },
      /* 5A */ InstrEntry {
        name: "ld e d",
        size: 1,
        info: None,
      },
      /* 5B */ InstrEntry {
        name: "ld e e",
        size: 1,
        info: None,
      },
      /* 5C */ InstrEntry {
        name: "ld e h",
        size: 1,
        info: None,
      },
      /* 5D */ InstrEntry {
        name: "ld e l",
        size: 1,
        info: None,
      },
      /* 5E */
      InstrEntry {
        name: "ld e (hl)",
        size: 1,
        info: None,
      },
      /* 5F */ InstrEntry {
        name: "ld e a",
        size: 1,
        info: None,
      },
      /* 60 */ InstrEntry {
        name: "ld h b",
        size: 1,
        info: None,
      },
      /* 61 */ InstrEntry {
        name: "ld h c",
        size: 1,
        info: None,
      },
      /* 62 */ InstrEntry {
        name: "ld h d",
        size: 1,
        info: None,
      },
      /* 63 */ InstrEntry {
        name: "ld h e",
        size: 1,
        info: None,
      },
      /* 64 */ InstrEntry {
        name: "ld h h",
        size: 1,
        info: None,
      },
      /* 65 */ InstrEntry {
        name: "ld h l",
        size: 1,
        info: None,
      },
      /* 66 */
      InstrEntry {
        name: "ld h (hl)",
        size: 1,
        info: None,
      },
      /* 67 */ InstrEntry {
        name: "ld h a",
        size: 1,
        info: None,
      },
      /* 68 */ InstrEntry {
        name: "ld l b",
        size: 1,
        info: None,
      },
      /* 69 */ InstrEntry {
        name: "ld l c",
        size: 1,
        info: None,
      },
      /* 6A */ InstrEntry {
        name: "ld l d",
        size: 1,
        info: None,
      },
      /* 6B */ InstrEntry {
        name: "ld l e",
        size: 1,
        info: None,
      },
      /* 6C */ InstrEntry {
        name: "ld l h",
        size: 1,
        info: None,
      },
      /* 6D */ InstrEntry {
        name: "ld l l",
        size: 1,
        info: None,
      },
      /* 6E */
      InstrEntry {
        name: "ld l (hl)",
        size: 1,
        info: None,
      },
      /* 6F */ InstrEntry {
        name: "ld l a",
        size: 1,
        info: None,
      },
      /* 70 */
      InstrEntry {
        name: "ld (hl) b",
        size: 1,
        info: None,
      },
      /* 71 */
      InstrEntry {
        name: "ld (hl) c",
        size: 1,
        info: None,
      },
      /* 72 */
      InstrEntry {
        name: "ld (hl) d",
        size: 1,
        info: None,
      },
      /* 73 */
      InstrEntry {
        name: "ld (hl) e",
        size: 1,
        info: None,
      },
      /* 74 */
      InstrEntry {
        name: "ld (hl) h",
        size: 1,
        info: None,
      },
      /* 75 */
      InstrEntry {
        name: "ld (hl) l",
        size: 1,
        info: None,
      },
      /* 76 */ InstrEntry {
        name: "halt",
        size: 1,
        info: None,
      },
      /* 77 */
      InstrEntry {
        name: "ld (hl) a",
        size: 1,
        info: None,
      },
      /* 78 */ InstrEntry {
        name: "ld a b",
        size: 1,
        info: None,
      },
      /* 79 */ InstrEntry {
        name: "ld a c",
        size: 1,
        info: None,
      },
      /* 7A */ InstrEntry {
        name: "ld a d",
        size: 1,
        info: None,
      },
      /* 7B */ InstrEntry {
        name: "ld a e",
        size: 1,
        info: None,
      },
      /* 7C */ InstrEntry {
        name: "ld a h",
        size: 1,
        info: None,
      },
      /* 7D */ InstrEntry {
        name: "ld a l",
        size: 1,
        info: None,
      },
      /* 7E */
      InstrEntry {
        name: "ld a (hl)",
        size: 1,
        info: None,
      },
      /* 7F */ InstrEntry {
        name: "ld a a",
        size: 1,
        info: None,
      },
      /* 80 */
      InstrEntry {
        name: "add a b",
        size: 1,
        info: None,
      },
      /* 81 */
      InstrEntry {
        name: "add a c",
        size: 1,
        info: None,
      },
      /* 82 */
      InstrEntry {
        name: "add a d",
        size: 1,
        info: None,
      },
      /* 83 */
      InstrEntry {
        name: "add a e",
        size: 1,
        info: None,
      },
      /* 84 */
      InstrEntry {
        name: "add a h",
        size: 1,
        info: None,
      },
      /* 85 */
      InstrEntry {
        name: "add a l",
        size: 1,
        info: None,
      },
      /* 86 */
      InstrEntry {
        name: "add a (hl)",
        size: 1,
        info: None,
      },
      /* 87 */
      InstrEntry {
        name: "add a a",
        size: 1,
        info: None,
      },
      /* 88 */
      InstrEntry {
        name: "adc a b",
        size: 1,
        info: None,
      },
      /* 89 */
      InstrEntry {
        name: "adc a c",
        size: 1,
        info: None,
      },
      /* 8A */
      InstrEntry {
        name: "adc a d",
        size: 1,
        info: None,
      },
      /* 8B */
      InstrEntry {
        name: "adc a e",
        size: 1,
        info: None,
      },
      /* 8C */
      InstrEntry {
        name: "adc a h",
        size: 1,
        info: None,
      },
      /* 8D */
      InstrEntry {
        name: "adc a l",
        size: 1,
        info: None,
      },
      /* 8E */
      InstrEntry {
        name: "adc a (hl)",
        size: 1,
        info: None,
      },
      /* 8F */
      InstrEntry {
        name: "adc a a",
        size: 1,
        info: None,
      },
      /* 90 */ InstrEntry {
        name: "sub b",
        size: 1,
        info: None,
      },
      /* 91 */ InstrEntry {
        name: "sub c",
        size: 1,
        info: None,
      },
      /* 92 */ InstrEntry {
        name: "sub d",
        size: 1,
        info: None,
      },
      /* 93 */ InstrEntry {
        name: "sub e",
        size: 1,
        info: None,
      },
      /* 94 */ InstrEntry {
        name: "sub h",
        size: 1,
        info: None,
      },
      /* 95 */ InstrEntry {
        name: "sub l",
        size: 1,
        info: None,
      },
      /* 96 */
      InstrEntry {
        name: "sub (hl)",
        size: 1,
        info: None,
      },
      /* 97 */ InstrEntry {
        name: "sub a",
        size: 1,
        info: None,
      },
      /* 98 */
      InstrEntry {
        name: "sbc a b",
        size: 1,
        info: None,
      },
      /* 99 */
      InstrEntry {
        name: "sbc a c",
        size: 1,
        info: None,
      },
      /* 9A */
      InstrEntry {
        name: "sbc a d",
        size: 1,
        info: None,
      },
      /* 9B */
      InstrEntry {
        name: "sbc a e",
        size: 1,
        info: None,
      },
      /* 9C */
      InstrEntry {
        name: "sbc a h",
        size: 1,
        info: None,
      },
      /* 9D */
      InstrEntry {
        name: "sbc a l",
        size: 1,
        info: None,
      },
      /* 9E */
      InstrEntry {
        name: "sbc a (hl)",
        size: 1,
        info: None,
      },
      /* 9F */
      InstrEntry {
        name: "sbc a a",
        size: 1,
        info: None,
      },
      /* A0 */ InstrEntry {
        name: "and b",
        size: 1,
        info: None,
      },
      /* A1 */ InstrEntry {
        name: "and c",
        size: 1,
        info: None,
      },
      /* A2 */ InstrEntry {
        name: "and d",
        size: 1,
        info: None,
      },
      /* A3 */ InstrEntry {
        name: "and e",
        size: 1,
        info: None,
      },
      /* A4 */ InstrEntry {
        name: "and h",
        size: 1,
        info: None,
      },
      /* A5 */ InstrEntry {
        name: "and l",
        size: 1,
        info: None,
      },
      /* A6 */
      InstrEntry {
        name: "and (hl)",
        size: 1,
        info: None,
      },
      /* A7 */ InstrEntry {
        name: "and a",
        size: 1,
        info: None,
      },
      /* A8 */ InstrEntry {
        name: "xor b",
        size: 1,
        info: None,
      },
      /* A9 */ InstrEntry {
        name: "xor c",
        size: 1,
        info: None,
      },
      /* AA */ InstrEntry {
        name: "xor d",
        size: 1,
        info: None,
      },
      /* AB */ InstrEntry {
        name: "xor e",
        size: 1,
        info: None,
      },
      /* AC */ InstrEntry {
        name: "xor h",
        size: 1,
        info: None,
      },
      /* AD */ InstrEntry {
        name: "xor l",
        size: 1,
        info: None,
      },
      /* AE */
      InstrEntry {
        name: "xor (hl)",
        size: 1,
        info: None,
      },
      /* AF */ InstrEntry {
        name: "xor a",
        size: 1,
        info: None,
      },
      /* B0 */ InstrEntry {
        name: "or b",
        size: 1,
        info: None,
      },
      /* B1 */ InstrEntry {
        name: "or c",
        size: 1,
        info: None,
      },
      /* B2 */ InstrEntry {
        name: "or d",
        size: 1,
        info: None,
      },
      /* B3 */ InstrEntry {
        name: "or e",
        size: 1,
        info: None,
      },
      /* B4 */ InstrEntry {
        name: "or h",
        size: 1,
        info: None,
      },
      /* B5 */ InstrEntry {
        name: "or l",
        size: 1,
        info: None,
      },
      /* B6 */
      InstrEntry {
        name: "or (hl)",
        size: 1,
        info: None,
      },
      /* B7 */ InstrEntry {
        name: "or a",
        size: 1,
        info: None,
      },
      /* B8 */ InstrEntry {
        name: "cp b",
        size: 1,
        info: None,
      },
      /* B9 */ InstrEntry {
        name: "cp c",
        size: 1,
        info: None,
      },
      /* BA */ InstrEntry {
        name: "cp d",
        size: 1,
        info: None,
      },
      /* BB */ InstrEntry {
        name: "cp e",
        size: 1,
        info: None,
      },
      /* BC */ InstrEntry {
        name: "cp h",
        size: 1,
        info: None,
      },
      /* BD */ InstrEntry {
        name: "cp l",
        size: 1,
        info: None,
      },
      /* BE */
      InstrEntry {
        name: "cp (hl)",
        size: 1,
        info: None,
      },
      /* BF */ InstrEntry {
        name: "cp a",
        size: 1,
        info: None,
      },
      /* C0 */ InstrEntry {
        name: "req nz",
        size: 1,
        info: None,
      },
      /* C1 */ InstrEntry {
        name: "pop bc",
        size: 1,
        info: None,
      },
      /* C2 */
      InstrEntry {
        name: "jp nz a16",
        size: 3,
        info: Some(A16),
      },
      /* C3 */
      InstrEntry {
        name: "jp a16",
        size: 3,
        info: Some(A16),
      },
      /* C4 */
      InstrEntry {
        name: "call nz a16",
        size: 3,
        info: Some(A16),
      },
      /* C5 */
      InstrEntry {
        name: "push bc",
        size: 1,
        info: None,
      },
      /* C6 */
      InstrEntry {
        name: "add a d8",
        size: 2,
        info: Some(D8),
      },
      /* C7 */
      InstrEntry {
        name: "rst 00h",
        size: 1,
        info: None,
      },
      /* C8 */ InstrEntry {
        name: "ret z",
        size: 1,
        info: None,
      },
      /* C9 */ InstrEntry {
        name: "ret",
        size: 1,
        info: None,
      },
      /* CA */
      InstrEntry {
        name: "jp z a16",
        size: 3,
        info: Some(A16),
      },
      /* CB */
      InstrEntry {
        name: "prefix_cb",
        size: 1,
        info: None,
      },
      /* CC */
      InstrEntry {
        name: "call z a16",
        size: 3,
        info: Some(A16),
      },
      /* CD */
      InstrEntry {
        name: "call a16",
        size: 3,
        info: Some(A16),
      },
      /* CE */
      InstrEntry {
        name: "adc a d8",
        size: 2,
        info: Some(D8),
      },
      /* CF */
      InstrEntry {
        name: "rst 08h",
        size: 1,
        info: None,
      },
      /* D0 */ InstrEntry {
        name: "ret nc",
        size: 1,
        info: None,
      },
      /* D1 */ InstrEntry {
        name: "pop de",
        size: 1,
        info: None,
      },
      /* D2 */
      InstrEntry {
        name: "jp nc a16",
        size: 3,
        info: Some(A16),
      },
      /* D3 */ InstrEntry {
        name: "???",
        size: 1,
        info: None,
      },
      /* D4 */
      InstrEntry {
        name: "call nc a16",
        size: 3,
        info: Some(A16),
      },
      /* D5 */
      InstrEntry {
        name: "push de",
        size: 1,
        info: None,
      },
      /* D6 */
      InstrEntry {
        name: "sub d8",
        size: 2,
        info: Some(D8),
      },
      /* D7 */
      InstrEntry {
        name: "rst 10h",
        size: 1,
        info: None,
      },
      /* D8 */ InstrEntry {
        name: "ret c",
        size: 1,
        info: None,
      },
      /* D9 */ InstrEntry {
        name: "reti",
        size: 1,
        info: None,
      },
      /* DA */
      InstrEntry {
        name: "jp c a16",
        size: 3,
        info: Some(A16),
      },
      /* DB */ InstrEntry {
        name: "???",
        size: 1,
        info: None,
      },
      /* DC */
      InstrEntry {
        name: "call c a16",
        size: 3,
        info: Some(A16),
      },
      /* DD */ InstrEntry {
        name: "???",
        size: 1,
        info: None,
      },
      /* DE */
      InstrEntry {
        name: "sbc a d8",
        size: 2,
        info: Some(D8),
      },
      /* DF */
      InstrEntry {
        name: "rst 18h",
        size: 1,
        info: None,
      },
      /* E0 */
      InstrEntry {
        name: "ldh (a8) a",
        size: 2,
        info: Some(A8),
      },
      /* E1 */ InstrEntry {
        name: "pop hl",
        size: 1,
        info: None,
      },
      /* E2 */
      InstrEntry {
        name: "ld (c) a",
        size: 1,
        info: None,
      },
      /* E3 */ InstrEntry {
        name: "???",
        size: 1,
        info: None,
      },
      /* E4 */ InstrEntry {
        name: "???",
        size: 1,
        info: None,
      },
      /* E5 */
      InstrEntry {
        name: "push hl",
        size: 1,
        info: None,
      },
      /* E6 */
      InstrEntry {
        name: "and d8",
        size: 2,
        info: Some(D8),
      },
      /* E7 */
      InstrEntry {
        name: "rst 20h",
        size: 1,
        info: None,
      },
      /* E8 */
      InstrEntry {
        name: "add sp r8",
        size: 2,
        info: Some(R8),
      },
      /* E9 */
      InstrEntry {
        name: "jp (hl)",
        size: 1,
        info: None,
      },
      /* EA */
      InstrEntry {
        name: "ld (a16) a",
        size: 3,
        info: Some(A16),
      },
      /* EB */ InstrEntry {
        name: "???",
        size: 1,
        info: None,
      },
      /* EC */ InstrEntry {
        name: "???",
        size: 1,
        info: None,
      },
      /* ED */ InstrEntry {
        name: "???",
        size: 1,
        info: None,
      },
      /* EE */
      InstrEntry {
        name: "xor d8",
        size: 2,
        info: Some(D8),
      },
      /* EF */
      InstrEntry {
        name: "rst 28h",
        size: 1,
        info: None,
      },
      /* F0 */
      InstrEntry {
        name: "ldh a (a8)",
        size: 2,
        info: Some(A8),
      },
      /* F1 */ InstrEntry {
        name: "pop af",
        size: 1,
        info: None,
      },
      /* F2 */
      InstrEntry {
        name: "ld a (c)",
        size: 1,
        info: None,
      },
      /* F3 */ InstrEntry {
        name: "di",
        size: 1,
        info: None,
      },
      /* F4 */ InstrEntry {
        name: "???",
        size: 1,
        info: None,
      },
      /* F5 */
      InstrEntry {
        name: "push af",
        size: 1,
        info: None,
      },
      /* F6 */
      InstrEntry {
        name: "or d8",
        size: 2,
        info: Some(D8),
      },
      /* F7 */
      InstrEntry {
        name: "rst 30h",
        size: 1,
        info: None,
      },
      /* F8 */
      InstrEntry {
        name: "ld hl sp+r8",
        size: 2,
        info: Some(R8),
      },
      /* F9 */
      InstrEntry {
        name: "ld sp hl",
        size: 1,
        info: None,
      },
      /* FA */
      InstrEntry {
        name: "ld a (a16)",
        size: 3,
        info: Some(A16),
      },
      /* FB */ InstrEntry {
        name: "ei",
        size: 1,
        info: None,
      },
      /* FC */ InstrEntry {
        name: "???",
        size: 1,
        info: None,
      },
      /* FD */ InstrEntry {
        name: "???",
        size: 1,
        info: None,
      },
      /* FE */
      InstrEntry {
        name: "cp d8",
        size: 2,
        info: Some(D8),
      },
      /* FF */
      InstrEntry {
        name: "rst 38h",
        size: 1,
        info: None,
      },
    ]
  }

  fn build_instr_cb_entry_table() -> Vec<InstrEntry> {
    vec![
      /* 00 */ InstrEntry {
        name: "rlc b",
        size: 2,
        info: None,
      },
      /* 01 */ InstrEntry {
        name: "rlc c",
        size: 2,
        info: None,
      },
      /* 02 */ InstrEntry {
        name: "rlc d",
        size: 2,
        info: None,
      },
      /* 03 */ InstrEntry {
        name: "rlc e",
        size: 2,
        info: None,
      },
      /* 04 */ InstrEntry {
        name: "rlc h",
        size: 2,
        info: None,
      },
      /* 05 */ InstrEntry {
        name: "rlc l",
        size: 2,
        info: None,
      },
      /* 06 */
      InstrEntry {
        name: "rlc (hl)",
        size: 2,
        info: None,
      },
      /* 07 */ InstrEntry {
        name: "rlc a",
        size: 2,
        info: None,
      },
      /* 08 */ InstrEntry {
        name: "rrc b",
        size: 2,
        info: None,
      },
      /* 09 */ InstrEntry {
        name: "rrc c",
        size: 2,
        info: None,
      },
      /* 0A */ InstrEntry {
        name: "rrc d",
        size: 2,
        info: None,
      },
      /* 0B */ InstrEntry {
        name: "rrc e",
        size: 2,
        info: None,
      },
      /* 0C */ InstrEntry {
        name: "rrc h",
        size: 2,
        info: None,
      },
      /* 0D */ InstrEntry {
        name: "rrc l",
        size: 2,
        info: None,
      },
      /* 0E */
      InstrEntry {
        name: "rrc (hl)",
        size: 2,
        info: None,
      },
      /* 0F */ InstrEntry {
        name: "rrc a",
        size: 2,
        info: None,
      },
      /* 10 */ InstrEntry {
        name: "rl b",
        size: 2,
        info: None,
      },
      /* 11 */ InstrEntry {
        name: "rl c",
        size: 2,
        info: None,
      },
      /* 12 */ InstrEntry {
        name: "rl d",
        size: 2,
        info: None,
      },
      /* 13 */ InstrEntry {
        name: "rl e",
        size: 2,
        info: None,
      },
      /* 14 */ InstrEntry {
        name: "rl h",
        size: 2,
        info: None,
      },
      /* 15 */ InstrEntry {
        name: "rl l",
        size: 2,
        info: None,
      },
      /* 16 */
      InstrEntry {
        name: "rl (hl)",
        size: 2,
        info: None,
      },
      /* 17 */ InstrEntry {
        name: "rl a",
        size: 2,
        info: None,
      },
      /* 18 */ InstrEntry {
        name: "rr b",
        size: 2,
        info: None,
      },
      /* 19 */ InstrEntry {
        name: "rr c",
        size: 2,
        info: None,
      },
      /* 1A */ InstrEntry {
        name: "rr d",
        size: 2,
        info: None,
      },
      /* 1B */ InstrEntry {
        name: "rr e",
        size: 2,
        info: None,
      },
      /* 1C */ InstrEntry {
        name: "rr h",
        size: 2,
        info: None,
      },
      /* 1D */ InstrEntry {
        name: "rr l",
        size: 2,
        info: None,
      },
      /* 1E */
      InstrEntry {
        name: "rr (hl)",
        size: 2,
        info: None,
      },
      /* 1F */ InstrEntry {
        name: "rr a",
        size: 2,
        info: None,
      },
      /* 20 */ InstrEntry {
        name: "sla b",
        size: 2,
        info: None,
      },
      /* 21 */ InstrEntry {
        name: "sla c",
        size: 2,
        info: None,
      },
      /* 22 */ InstrEntry {
        name: "sla d",
        size: 2,
        info: None,
      },
      /* 23 */ InstrEntry {
        name: "sla e",
        size: 2,
        info: None,
      },
      /* 24 */ InstrEntry {
        name: "sla h",
        size: 2,
        info: None,
      },
      /* 25 */ InstrEntry {
        name: "sla l",
        size: 2,
        info: None,
      },
      /* 26 */
      InstrEntry {
        name: "sla (hl)",
        size: 2,
        info: None,
      },
      /* 27 */ InstrEntry {
        name: "sla a",
        size: 2,
        info: None,
      },
      /* 28 */ InstrEntry {
        name: "sra b",
        size: 2,
        info: None,
      },
      /* 29 */ InstrEntry {
        name: "sra c",
        size: 2,
        info: None,
      },
      /* 2A */ InstrEntry {
        name: "sra d",
        size: 2,
        info: None,
      },
      /* 2B */ InstrEntry {
        name: "sra e",
        size: 2,
        info: None,
      },
      /* 2C */ InstrEntry {
        name: "sra h",
        size: 2,
        info: None,
      },
      /* 2D */ InstrEntry {
        name: "sra l",
        size: 2,
        info: None,
      },
      /* 2E */
      InstrEntry {
        name: "sra (hl)",
        size: 2,
        info: None,
      },
      /* 2F */ InstrEntry {
        name: "sra a",
        size: 2,
        info: None,
      },
      /* 30 */ InstrEntry {
        name: "swap b",
        size: 2,
        info: None,
      },
      /* 31 */ InstrEntry {
        name: "swap c",
        size: 2,
        info: None,
      },
      /* 32 */ InstrEntry {
        name: "swap d",
        size: 2,
        info: None,
      },
      /* 33 */ InstrEntry {
        name: "swap e",
        size: 2,
        info: None,
      },
      /* 34 */ InstrEntry {
        name: "swap h",
        size: 2,
        info: None,
      },
      /* 35 */ InstrEntry {
        name: "swap l",
        size: 2,
        info: None,
      },
      /* 36 */
      InstrEntry {
        name: "swap (hl)",
        size: 2,
        info: None,
      },
      /* 37 */ InstrEntry {
        name: "swap a",
        size: 2,
        info: None,
      },
      /* 38 */ InstrEntry {
        name: "srl b",
        size: 2,
        info: None,
      },
      /* 39 */ InstrEntry {
        name: "srl c",
        size: 2,
        info: None,
      },
      /* 3A */ InstrEntry {
        name: "srl d",
        size: 2,
        info: None,
      },
      /* 3B */ InstrEntry {
        name: "srl e",
        size: 2,
        info: None,
      },
      /* 3C */ InstrEntry {
        name: "srl h",
        size: 2,
        info: None,
      },
      /* 3D */ InstrEntry {
        name: "srl l",
        size: 2,
        info: None,
      },
      /* 3E */
      InstrEntry {
        name: "srl (hl)",
        size: 2,
        info: None,
      },
      /* 3F */ InstrEntry {
        name: "srl a",
        size: 2,
        info: None,
      },
      /* 40 */
      InstrEntry {
        name: "bit 0 b",
        size: 2,
        info: None,
      },
      /* 41 */
      InstrEntry {
        name: "bit 0 c",
        size: 2,
        info: None,
      },
      /* 42 */
      InstrEntry {
        name: "bit 0 d",
        size: 2,
        info: None,
      },
      /* 43 */
      InstrEntry {
        name: "bit 0 e",
        size: 2,
        info: None,
      },
      /* 44 */
      InstrEntry {
        name: "bit 0 h",
        size: 2,
        info: None,
      },
      /* 45 */
      InstrEntry {
        name: "bit 0 l",
        size: 2,
        info: None,
      },
      /* 46 */
      InstrEntry {
        name: "bit 0 (hl)",
        size: 2,
        info: None,
      },
      /* 47 */
      InstrEntry {
        name: "bit 0 a",
        size: 2,
        info: None,
      },
      /* 48 */
      InstrEntry {
        name: "bit 1 b",
        size: 2,
        info: None,
      },
      /* 49 */
      InstrEntry {
        name: "bit 1 c",
        size: 2,
        info: None,
      },
      /* 4A */
      InstrEntry {
        name: "bit 1 d",
        size: 2,
        info: None,
      },
      /* 4B */
      InstrEntry {
        name: "bit 1 e",
        size: 2,
        info: None,
      },
      /* 4C */
      InstrEntry {
        name: "bit 1 h",
        size: 2,
        info: None,
      },
      /* 4D */
      InstrEntry {
        name: "bit 1 l",
        size: 2,
        info: None,
      },
      /* 4E */
      InstrEntry {
        name: "bit 1 (hl)",
        size: 2,
        info: None,
      },
      /* 4F */
      InstrEntry {
        name: "bit 1 a",
        size: 2,
        info: None,
      },
      /* 50 */
      InstrEntry {
        name: "bit 2 b",
        size: 2,
        info: None,
      },
      /* 51 */
      InstrEntry {
        name: "bit 2 c",
        size: 2,
        info: None,
      },
      /* 52 */
      InstrEntry {
        name: "bit 2 d",
        size: 2,
        info: None,
      },
      /* 53 */
      InstrEntry {
        name: "bit 2 e",
        size: 2,
        info: None,
      },
      /* 54 */
      InstrEntry {
        name: "bit 2 h",
        size: 2,
        info: None,
      },
      /* 55 */
      InstrEntry {
        name: "bit 2 l",
        size: 2,
        info: None,
      },
      /* 56 */
      InstrEntry {
        name: "bit 2 (hl)",
        size: 2,
        info: None,
      },
      /* 57 */
      InstrEntry {
        name: "bit 2 a",
        size: 2,
        info: None,
      },
      /* 58 */
      InstrEntry {
        name: "bit 3 b",
        size: 2,
        info: None,
      },
      /* 59 */
      InstrEntry {
        name: "bit 3 c",
        size: 2,
        info: None,
      },
      /* 5A */
      InstrEntry {
        name: "bit 3 d",
        size: 2,
        info: None,
      },
      /* 5B */
      InstrEntry {
        name: "bit 3 e",
        size: 2,
        info: None,
      },
      /* 5C */
      InstrEntry {
        name: "bit 3 h",
        size: 2,
        info: None,
      },
      /* 5D */
      InstrEntry {
        name: "bit 3 l",
        size: 2,
        info: None,
      },
      /* 5E */
      InstrEntry {
        name: "bit 3 (hl)",
        size: 2,
        info: None,
      },
      /* 5F */
      InstrEntry {
        name: "bit 3 a",
        size: 2,
        info: None,
      },
      /* 60 */
      InstrEntry {
        name: "bit 4 b",
        size: 2,
        info: None,
      },
      /* 61 */
      InstrEntry {
        name: "bit 4 c",
        size: 2,
        info: None,
      },
      /* 62 */
      InstrEntry {
        name: "bit 4 d",
        size: 2,
        info: None,
      },
      /* 63 */
      InstrEntry {
        name: "bit 4 e",
        size: 2,
        info: None,
      },
      /* 64 */
      InstrEntry {
        name: "bit 4 h",
        size: 2,
        info: None,
      },
      /* 65 */
      InstrEntry {
        name: "bit 4 l",
        size: 2,
        info: None,
      },
      /* 66 */
      InstrEntry {
        name: "bit 4 (hl)",
        size: 2,
        info: None,
      },
      /* 67 */
      InstrEntry {
        name: "bit 4 a",
        size: 2,
        info: None,
      },
      /* 68 */
      InstrEntry {
        name: "bit 5 b",
        size: 2,
        info: None,
      },
      /* 69 */
      InstrEntry {
        name: "bit 5 c",
        size: 2,
        info: None,
      },
      /* 6A */
      InstrEntry {
        name: "bit 5 d",
        size: 2,
        info: None,
      },
      /* 6B */
      InstrEntry {
        name: "bit 5 e",
        size: 2,
        info: None,
      },
      /* 6C */
      InstrEntry {
        name: "bit 5 h",
        size: 2,
        info: None,
      },
      /* 6D */
      InstrEntry {
        name: "bit 5 l",
        size: 2,
        info: None,
      },
      /* 6E */
      InstrEntry {
        name: "bit 5 (hl)",
        size: 2,
        info: None,
      },
      /* 6F */
      InstrEntry {
        name: "bit 5 a",
        size: 2,
        info: None,
      },
      /* 70 */
      InstrEntry {
        name: "bit 6 b",
        size: 2,
        info: None,
      },
      /* 71 */
      InstrEntry {
        name: "bit 6 c",
        size: 2,
        info: None,
      },
      /* 72 */
      InstrEntry {
        name: "bit 6 d",
        size: 2,
        info: None,
      },
      /* 73 */
      InstrEntry {
        name: "bit 6 e",
        size: 2,
        info: None,
      },
      /* 74 */
      InstrEntry {
        name: "bit 6 h",
        size: 2,
        info: None,
      },
      /* 75 */
      InstrEntry {
        name: "bit 6 l",
        size: 2,
        info: None,
      },
      /* 76 */
      InstrEntry {
        name: "bit 6 (hl)",
        size: 2,
        info: None,
      },
      /* 77 */
      InstrEntry {
        name: "bit 6 a",
        size: 2,
        info: None,
      },
      /* 78 */
      InstrEntry {
        name: "bit 7 b",
        size: 2,
        info: None,
      },
      /* 79 */
      InstrEntry {
        name: "bit 7 c",
        size: 2,
        info: None,
      },
      /* 7A */
      InstrEntry {
        name: "bit 7 d",
        size: 2,
        info: None,
      },
      /* 7B */
      InstrEntry {
        name: "bit 7 e",
        size: 2,
        info: None,
      },
      /* 7C */
      InstrEntry {
        name: "bit 7 h",
        size: 2,
        info: None,
      },
      /* 7D */
      InstrEntry {
        name: "bit 7 l",
        size: 2,
        info: None,
      },
      /* 7E */
      InstrEntry {
        name: "bit 7 (hl)",
        size: 2,
        info: None,
      },
      /* 7F */
      InstrEntry {
        name: "bit 7 a",
        size: 2,
        info: None,
      },
      /* 80 */
      InstrEntry {
        name: "res 0 b",
        size: 2,
        info: None,
      },
      /* 81 */
      InstrEntry {
        name: "res 0 c",
        size: 2,
        info: None,
      },
      /* 82 */
      InstrEntry {
        name: "res 0 d",
        size: 2,
        info: None,
      },
      /* 83 */
      InstrEntry {
        name: "res 0 e",
        size: 2,
        info: None,
      },
      /* 84 */
      InstrEntry {
        name: "res 0 h",
        size: 2,
        info: None,
      },
      /* 85 */
      InstrEntry {
        name: "res 0 l",
        size: 2,
        info: None,
      },
      /* 86 */
      InstrEntry {
        name: "res 0 (hl)",
        size: 2,
        info: None,
      },
      /* 87 */
      InstrEntry {
        name: "res 0 a",
        size: 2,
        info: None,
      },
      /* 88 */
      InstrEntry {
        name: "res 1 b",
        size: 2,
        info: None,
      },
      /* 89 */
      InstrEntry {
        name: "res 1 c",
        size: 2,
        info: None,
      },
      /* 8A */
      InstrEntry {
        name: "res 1 d",
        size: 2,
        info: None,
      },
      /* 8B */
      InstrEntry {
        name: "res 1 e",
        size: 2,
        info: None,
      },
      /* 8C */
      InstrEntry {
        name: "res 1 h",
        size: 2,
        info: None,
      },
      /* 8D */
      InstrEntry {
        name: "res 1 l",
        size: 2,
        info: None,
      },
      /* 8E */
      InstrEntry {
        name: "res 1 (hl)",
        size: 2,
        info: None,
      },
      /* 8F */
      InstrEntry {
        name: "res 1 a",
        size: 2,
        info: None,
      },
      /* 90 */
      InstrEntry {
        name: "res 2 b",
        size: 2,
        info: None,
      },
      /* 91 */
      InstrEntry {
        name: "res 2 c",
        size: 2,
        info: None,
      },
      /* 92 */
      InstrEntry {
        name: "res 2 d",
        size: 2,
        info: None,
      },
      /* 93 */
      InstrEntry {
        name: "res 2 e",
        size: 2,
        info: None,
      },
      /* 94 */
      InstrEntry {
        name: "res 2 h",
        size: 2,
        info: None,
      },
      /* 95 */
      InstrEntry {
        name: "res 2 l",
        size: 2,
        info: None,
      },
      /* 96 */
      InstrEntry {
        name: "res 2 (hl)",
        size: 2,
        info: None,
      },
      /* 97 */
      InstrEntry {
        name: "res 2 a",
        size: 2,
        info: None,
      },
      /* 98 */
      InstrEntry {
        name: "res 3 b",
        size: 2,
        info: None,
      },
      /* 99 */
      InstrEntry {
        name: "res 3 c",
        size: 2,
        info: None,
      },
      /* 9A */
      InstrEntry {
        name: "res 3 d",
        size: 2,
        info: None,
      },
      /* 9B */
      InstrEntry {
        name: "res 3 e",
        size: 2,
        info: None,
      },
      /* 9C */
      InstrEntry {
        name: "res 3 h",
        size: 2,
        info: None,
      },
      /* 9D */
      InstrEntry {
        name: "res 3 l",
        size: 2,
        info: None,
      },
      /* 9E */
      InstrEntry {
        name: "res 3 (hl)",
        size: 2,
        info: None,
      },
      /* 9F */
      InstrEntry {
        name: "res 3 a",
        size: 2,
        info: None,
      },
      /* A0 */
      InstrEntry {
        name: "res 4 b",
        size: 2,
        info: None,
      },
      /* A1 */
      InstrEntry {
        name: "res 4 c",
        size: 2,
        info: None,
      },
      /* A2 */
      InstrEntry {
        name: "res 4 d",
        size: 2,
        info: None,
      },
      /* A3 */
      InstrEntry {
        name: "res 4 e",
        size: 2,
        info: None,
      },
      /* A4 */
      InstrEntry {
        name: "res 4 h",
        size: 2,
        info: None,
      },
      /* A5 */
      InstrEntry {
        name: "res 4 l",
        size: 2,
        info: None,
      },
      /* A6 */
      InstrEntry {
        name: "res 4 (hl)",
        size: 2,
        info: None,
      },
      /* A7 */
      InstrEntry {
        name: "res 4 a",
        size: 2,
        info: None,
      },
      /* A8 */
      InstrEntry {
        name: "res 5 b",
        size: 2,
        info: None,
      },
      /* A9 */
      InstrEntry {
        name: "res 5 c",
        size: 2,
        info: None,
      },
      /* AA */
      InstrEntry {
        name: "res 5 d",
        size: 2,
        info: None,
      },
      /* AB */
      InstrEntry {
        name: "res 5 e",
        size: 2,
        info: None,
      },
      /* AC */
      InstrEntry {
        name: "res 5 h",
        size: 2,
        info: None,
      },
      /* AD */
      InstrEntry {
        name: "res 5 l",
        size: 2,
        info: None,
      },
      /* AE */
      InstrEntry {
        name: "res 5 (hl)",
        size: 2,
        info: None,
      },
      /* AF */
      InstrEntry {
        name: "res 5 a",
        size: 2,
        info: None,
      },
      /* B0 */
      InstrEntry {
        name: "res 6 b",
        size: 2,
        info: None,
      },
      /* B1 */
      InstrEntry {
        name: "res 6 c",
        size: 2,
        info: None,
      },
      /* B2 */
      InstrEntry {
        name: "res 6 d",
        size: 2,
        info: None,
      },
      /* B3 */
      InstrEntry {
        name: "res 6 e",
        size: 2,
        info: None,
      },
      /* B4 */
      InstrEntry {
        name: "res 6 h",
        size: 2,
        info: None,
      },
      /* B5 */
      InstrEntry {
        name: "res 6 l",
        size: 2,
        info: None,
      },
      /* B6 */
      InstrEntry {
        name: "res 6 (hl)",
        size: 2,
        info: None,
      },
      /* B7 */
      InstrEntry {
        name: "res 6 a",
        size: 2,
        info: None,
      },
      /* B8 */
      InstrEntry {
        name: "res 7 b",
        size: 2,
        info: None,
      },
      /* B9 */
      InstrEntry {
        name: "res 7 c",
        size: 2,
        info: None,
      },
      /* BA */
      InstrEntry {
        name: "res 7 d",
        size: 2,
        info: None,
      },
      /* BB */
      InstrEntry {
        name: "res 7 e",
        size: 2,
        info: None,
      },
      /* BC */
      InstrEntry {
        name: "res 7 h",
        size: 2,
        info: None,
      },
      /* BD */
      InstrEntry {
        name: "res 7 l",
        size: 2,
        info: None,
      },
      /* BE */
      InstrEntry {
        name: "res 7 (hl)",
        size: 2,
        info: None,
      },
      /* BF */
      InstrEntry {
        name: "res 7 a",
        size: 2,
        info: None,
      },
      /* C0 */
      InstrEntry {
        name: "set 0 b",
        size: 2,
        info: None,
      },
      /* C1 */
      InstrEntry {
        name: "set 0 c",
        size: 2,
        info: None,
      },
      /* C2 */
      InstrEntry {
        name: "set 0 d",
        size: 2,
        info: None,
      },
      /* C3 */
      InstrEntry {
        name: "set 0 e",
        size: 2,
        info: None,
      },
      /* C4 */
      InstrEntry {
        name: "set 0 h",
        size: 2,
        info: None,
      },
      /* C5 */
      InstrEntry {
        name: "set 0 l",
        size: 2,
        info: None,
      },
      /* C6 */
      InstrEntry {
        name: "set 0 (hl)",
        size: 2,
        info: None,
      },
      /* C7 */
      InstrEntry {
        name: "set 0 a",
        size: 2,
        info: None,
      },
      /* C8 */
      InstrEntry {
        name: "set 1 b",
        size: 2,
        info: None,
      },
      /* C9 */
      InstrEntry {
        name: "set 1 c",
        size: 2,
        info: None,
      },
      /* CA */
      InstrEntry {
        name: "set 1 d",
        size: 2,
        info: None,
      },
      /* CB */
      InstrEntry {
        name: "set 1 e",
        size: 2,
        info: None,
      },
      /* CC */
      InstrEntry {
        name: "set 1 h",
        size: 2,
        info: None,
      },
      /* CD */
      InstrEntry {
        name: "set 1 l",
        size: 2,
        info: None,
      },
      /* CE */
      InstrEntry {
        name: "set 1 (hl)",
        size: 2,
        info: None,
      },
      /* CF */
      InstrEntry {
        name: "set 1 a",
        size: 2,
        info: None,
      },
      /* D0 */
      InstrEntry {
        name: "set 2 b",
        size: 2,
        info: None,
      },
      /* D1 */
      InstrEntry {
        name: "set 2 c",
        size: 2,
        info: None,
      },
      /* D2 */
      InstrEntry {
        name: "set 2 d",
        size: 2,
        info: None,
      },
      /* D3 */
      InstrEntry {
        name: "set 2 e",
        size: 2,
        info: None,
      },
      /* D4 */
      InstrEntry {
        name: "set 2 h",
        size: 2,
        info: None,
      },
      /* D5 */
      InstrEntry {
        name: "set 2 l",
        size: 2,
        info: None,
      },
      /* D6 */
      InstrEntry {
        name: "set 2 (hl)",
        size: 2,
        info: None,
      },
      /* D7 */
      InstrEntry {
        name: "set 2 a",
        size: 2,
        info: None,
      },
      /* D8 */
      InstrEntry {
        name: "set 3 b",
        size: 2,
        info: None,
      },
      /* D9 */
      InstrEntry {
        name: "set 3 c",
        size: 2,
        info: None,
      },
      /* DA */
      InstrEntry {
        name: "set 3 d",
        size: 2,
        info: None,
      },
      /* DB */
      InstrEntry {
        name: "set 3 e",
        size: 2,
        info: None,
      },
      /* DC */
      InstrEntry {
        name: "set 3 h",
        size: 2,
        info: None,
      },
      /* DD */
      InstrEntry {
        name: "set 3 l",
        size: 2,
        info: None,
      },
      /* DE */
      InstrEntry {
        name: "set 3 (hl)",
        size: 2,
        info: None,
      },
      /* DF */
      InstrEntry {
        name: "set 3 a",
        size: 2,
        info: None,
      },
      /* E0 */
      InstrEntry {
        name: "set 4 b",
        size: 2,
        info: None,
      },
      /* E1 */
      InstrEntry {
        name: "set 4 c",
        size: 2,
        info: None,
      },
      /* E2 */
      InstrEntry {
        name: "set 4 d",
        size: 2,
        info: None,
      },
      /* E3 */
      InstrEntry {
        name: "set 4 e",
        size: 2,
        info: None,
      },
      /* E4 */
      InstrEntry {
        name: "set 4 h",
        size: 2,
        info: None,
      },
      /* E5 */
      InstrEntry {
        name: "set 4 l",
        size: 2,
        info: None,
      },
      /* E6 */
      InstrEntry {
        name: "set 4 (hl)",
        size: 2,
        info: None,
      },
      /* E7 */
      InstrEntry {
        name: "set 4 a",
        size: 2,
        info: None,
      },
      /* E8 */
      InstrEntry {
        name: "set 5 b",
        size: 2,
        info: None,
      },
      /* E9 */
      InstrEntry {
        name: "set 5 c",
        size: 2,
        info: None,
      },
      /* EA */
      InstrEntry {
        name: "set 5 d",
        size: 2,
        info: None,
      },
      /* EB */
      InstrEntry {
        name: "set 5 e",
        size: 2,
        info: None,
      },
      /* EC */
      InstrEntry {
        name: "set 5 h",
        size: 2,
        info: None,
      },
      /* ED */
      InstrEntry {
        name: "set 5 l",
        size: 2,
        info: None,
      },
      /* EE */
      InstrEntry {
        name: "set 5 (hl)",
        size: 2,
        info: None,
      },
      /* EF */
      InstrEntry {
        name: "set 5 a",
        size: 2,
        info: None,
      },
      /* F0 */
      InstrEntry {
        name: "set 6 b",
        size: 2,
        info: None,
      },
      /* F1 */
      InstrEntry {
        name: "set 6 c",
        size: 2,
        info: None,
      },
      /* F2 */
      InstrEntry {
        name: "set 6 d",
        size: 2,
        info: None,
      },
      /* F3 */
      InstrEntry {
        name: "set 6 e",
        size: 2,
        info: None,
      },
      /* F4 */
      InstrEntry {
        name: "set 6 h",
        size: 2,
        info: None,
      },
      /* F5 */
      InstrEntry {
        name: "set 6 l",
        size: 2,
        info: None,
      },
      /* F6 */
      InstrEntry {
        name: "set 6 (hl)",
        size: 2,
        info: None,
      },
      /* F7 */
      InstrEntry {
        name: "set 6 a",
        size: 2,
        info: None,
      },
      /* F8 */
      InstrEntry {
        name: "set 7 b",
        size: 2,
        info: None,
      },
      /* F9 */
      InstrEntry {
        name: "set 7 c",
        size: 2,
        info: None,
      },
      /* FA */
      InstrEntry {
        name: "set 7 d",
        size: 2,
        info: None,
      },
      /* FB */
      InstrEntry {
        name: "set 7 e",
        size: 2,
        info: None,
      },
      /* FC */
      InstrEntry {
        name: "set 7 h",
        size: 2,
        info: None,
      },
      /* FD */
      InstrEntry {
        name: "set 7 l",
        size: 2,
        info: None,
      },
      /* FE */
      InstrEntry {
        name: "set 7 (hl)",
        size: 2,
        info: None,
      },
      /* FF */
      InstrEntry {
        name: "set 7 a",
        size: 2,
        info: None,
      },
    ]
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::collections::VecDeque;

  #[test]
  fn test_dasm_nop() {
    let mut dasm = Dasm::new();
    let mut bytes = VecDeque::from([0x00u8]);
    let instr = dasm.munch(bytes.pop_front().unwrap());
    assert!(instr.is_some());
    assert_eq!(instr.unwrap(), "nop");
  }

  #[test]
  fn test_dasm_size_1_no_imm() {
    let mut dasm = Dasm::new();
    let mut bytes = VecDeque::from([0x05, 0x03, 0x04, 0x07, 0x58, 0x93, 0xe3]);
    // dec b
    let instr = dasm.munch(bytes.pop_front().unwrap());
    assert!(instr.is_some());
    assert_eq!(instr.unwrap(), "dec b");
    // inc bc
    let instr = dasm.munch(bytes.pop_front().unwrap());
    assert!(instr.is_some());
    assert_eq!(instr.unwrap(), "inc bc");
    // inc b
    let instr = dasm.munch(bytes.pop_front().unwrap());
    assert!(instr.is_some());
    assert_eq!(instr.unwrap(), "inc b");
    // rlca
    let instr = dasm.munch(bytes.pop_front().unwrap());
    assert!(instr.is_some());
    assert_eq!(instr.unwrap(), "rlca");
    // ld e b
    let instr = dasm.munch(bytes.pop_front().unwrap());
    assert!(instr.is_some());
    assert_eq!(instr.unwrap(), "ld e b");
    // sub e
    let instr = dasm.munch(bytes.pop_front().unwrap());
    assert!(instr.is_some());
    assert_eq!(instr.unwrap(), "sub e");
    // invalid
    let instr = dasm.munch(bytes.pop_front().unwrap());
    assert!(instr.is_some());
    assert_eq!(instr.unwrap(), "???");
  }

  #[test]
  fn test_dasm_size_2_no_imm() {
    let mut dasm = Dasm::new();
    let mut bytes = VecDeque::from([0xcb, 0x10, 0xcb, 0x75, 0xcb, 0xcb]);
    // rl b
    let mut instr = None;
    while let val = dasm.munch(bytes.pop_front().unwrap()) {
      if val.is_some() {
        instr = val;
        break;
      }
    }
    assert_eq!(instr.unwrap(), "rl b");
    // bit 6 l
    let mut instr = None;
    while let val = dasm.munch(bytes.pop_front().unwrap()) {
      if val.is_some() {
        instr = val;
        break;
      }
    }
    assert_eq!(instr.unwrap(), "bit 6 l");
    // set 1 e
    let mut instr = None;
    while let val = dasm.munch(bytes.pop_front().unwrap()) {
      if val.is_some() {
        instr = val;
        break;
      }
    }
    assert_eq!(instr.unwrap(), "set 1 e");
  }

  #[test]
  fn test_dasm_size_2_imm8() {
    let mut dasm = Dasm::new();
    let mut bytes = VecDeque::from([0x06, 100]);
    // ld b 100
    let mut instr = None;
    while let val = dasm.munch(bytes.pop_front().unwrap()) {
      if val.is_some() {
        instr = val;
        break;
      }
    }
    assert_eq!(instr.unwrap(), "ld b 100");
  }

  #[test]
  fn test_dasm_size_3() {
    let mut dasm = Dasm::new();
    let mut bytes = VecDeque::from([0x01, 10, 0x00, 0xea, 0x34, 0x12]);
    // 01: ld bc 10
    let mut instr = None;
    while let val = dasm.munch(bytes.pop_front().unwrap()) {
      if val.is_some() {
        instr = val;
        break;
      }
    }
    assert_eq!(instr.unwrap(), "ld bc 10");
    // EA: ld ($1234) a
    let mut instr = None;
    while let val = dasm.munch(bytes.pop_front().unwrap()) {
      if val.is_some() {
        instr = val;
        break;
      }
    }
    assert_eq!(instr.unwrap(), "ld ($1234) a");
  }

  #[test]
  fn test_dasm_any() {
    let mut dasm = Dasm::new();
    let mut bytes = VecDeque::from([0x10, 0x00, 0x55, 0x26, 0xff, 0xcc, 0xad, 0xde]);
    // 10: stop
    let mut instr = None;
    while let val = dasm.munch(bytes.pop_front().unwrap()) {
      if val.is_some() {
        instr = val;
        break;
      }
    }
    assert_eq!(instr.unwrap(), "stop");
    // 55: ld d l
    let mut instr = None;
    while let val = dasm.munch(bytes.pop_front().unwrap()) {
      if val.is_some() {
        instr = val;
        break;
      }
    }
    assert_eq!(instr.unwrap(), "ld d l");
    // 26: ld h 255
    let mut instr = None;
    while let val = dasm.munch(bytes.pop_front().unwrap()) {
      if val.is_some() {
        instr = val;
        break;
      }
    }
    assert_eq!(instr.unwrap(), "ld h 255");
    // CC: call z $DEAD
    let mut instr = None;
    while let val = dasm.munch(bytes.pop_front().unwrap()) {
      if val.is_some() {
        instr = val;
        break;
      }
    }
    assert_eq!(instr.unwrap(), "call z $DEAD");
  }
}
