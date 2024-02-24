//! Cartridge Header helper functions

use crate::cart::mapper::MapperType;
use crate::err::GbResult;

#[derive(Debug)]
pub enum GBCSupport {
  BackwardsCompatible,
  GBCOnly,
  Unknown,
}

impl From<u8> for GBCSupport {
  fn from(value: u8) -> Self {
    match value {
      0x80 => GBCSupport::BackwardsCompatible,
      0xC0 => GBCSupport::GBCOnly,
      _ => GBCSupport::Unknown,
    }
  }
}

struct CartridgeType {
  battery_present: bool,
  ram_present: bool,
  mapper_type: MapperType,
}

#[derive(Debug)]
pub struct Header {
  pub title: String,
  pub manufacturing_code: String,
  pub gbc_support: GBCSupport,
  pub publisher: String,
  pub mapper: MapperType,
  pub battery_present: bool,
  pub ram_present: bool,
  pub rom_banks: u32,
  pub ram_banks: u32,
  pub rom_version: u8,
  pub header_checksum: u8,
  pub global_checksum: u16,
}

impl Header {
  pub fn new() -> Self {
    Self {
      title: String::new(),
      manufacturing_code: String::new(),
      gbc_support: GBCSupport::Unknown,
      publisher: String::new(),
      mapper: MapperType::None,
      battery_present: false,
      ram_present: false,
      rom_banks: 0,
      ram_banks: 0,
      rom_version: 0,
      header_checksum: 0,
      global_checksum: 0,
    }
  }

  // Reads out the header from the given byte stream. The byte stream should start
  // at 0x100
  pub fn read_header(&mut self, bytes: &Vec<u8>) -> GbResult<()> {
    // $0134-$0143 Title
    self.title = String::from_utf8(Vec::from(&bytes[0x34..=0x43]))
      // if we fail, try only up to $013e as this is exclusive to the title
      .or(String::from_utf8(Vec::from(&bytes[0x34..=0x3e])))
      .unwrap();

    // $013F-$0142 Manufacturing Code (shared with title space)
    self.manufacturing_code = String::from_utf8(Vec::from(&bytes[0x3f..=0x42])).unwrap();

    // $0143 CGB Flag
    self.gbc_support = bytes[0x43].into();
    // TODO SGB Flag

    // Publisher
    // $014B Old Licensee Code
    // $0144-$0145 New Licensee Code
    let code = bytes[0x4b];
    self.publisher = if code == 0x33 {
      // use new licensee list
      // the code in the new licensee list is a two char ascii code
      let ascii_code = String::from_utf8(Vec::from(&bytes[0x44..=0x45])).unwrap();
      get_new_publisher(&*ascii_code)
    } else {
      // use old licensee list
      get_old_publisher(code)
    };

    // $0147 Cartridge Info
    let code = bytes[0x47];
    let info = get_cart_type(code);
    self.battery_present = info.battery_present;
    self.ram_present = info.ram_present;
    self.mapper = info.mapper_type;

    // $0148 ROM Size
    let code = bytes[0x48];
    self.rom_banks = get_rom_banks(code);

    // $0149 RAM Size
    let code = bytes[0x49];
    self.ram_banks = get_ram_banks(code);

    // TODO Dest code

    // $014C Mask Rom Version Number
    self.rom_version = bytes[0x4c];

    // $014D Header Checksum
    self.header_checksum = bytes[0x4d];

    // $014E-$014F Global Checksum
    self.global_checksum = u16::from_be_bytes([bytes[0x4e], bytes[0x4f]]);

    Ok(())
  }
}

fn get_ram_banks(code: u8) -> u32 {
  match code {
    0x00 => 0,
    // 0x01 not valid
    0x02 => 1,
    0x03 => 4,
    0x04 => 16,
    0x05 => 8,
    _ => panic!("Unsupported ram banks code [{:02X}]", code),
  }
}

fn get_rom_banks(code: u8) -> u32 {
  if code > 0x08 {
    panic!("Unsupported rom banks code [{:02X}]", code);
  }
  1 << code
}

fn get_cart_type(code: u8) -> CartridgeType {
  match code {
    0x00 => CartridgeType {
      battery_present: false,
      ram_present: false,
      mapper_type: MapperType::None,
    },
    0x01 => CartridgeType {
      battery_present: false,
      ram_present: false,
      mapper_type: MapperType::Mbc1,
    },
    0x02 => CartridgeType {
      battery_present: false,
      ram_present: true,
      mapper_type: MapperType::Mbc1,
    },
    0x03 => CartridgeType {
      battery_present: true,
      ram_present: true,
      mapper_type: MapperType::Mbc1,
    },
    0x05 => CartridgeType {
      battery_present: false,
      ram_present: false,
      mapper_type: MapperType::Mbc2,
    },
    0x06 => CartridgeType {
      battery_present: true,
      ram_present: false,
      mapper_type: MapperType::Mbc2,
    },
    0x08 => CartridgeType {
      battery_present: false,
      ram_present: true,
      mapper_type: MapperType::None,
    },
    0x09 => CartridgeType {
      battery_present: true,
      ram_present: true,
      mapper_type: MapperType::None,
    },
    0x0B => CartridgeType {
      battery_present: false,
      ram_present: false,
      mapper_type: MapperType::Mmm01,
    },
    0x0C => CartridgeType {
      battery_present: false,
      ram_present: true,
      mapper_type: MapperType::Mmm01,
    },
    0x0D => CartridgeType {
      battery_present: true,
      ram_present: true,
      mapper_type: MapperType::Mmm01,
    },
    0x11 => CartridgeType {
      battery_present: false,
      ram_present: false,
      mapper_type: MapperType::Mbc3,
    },
    0x12 => CartridgeType {
      battery_present: false,
      ram_present: true,
      mapper_type: MapperType::Mbc3,
    },
    0x13 => CartridgeType {
      battery_present: true,
      ram_present: true,
      mapper_type: MapperType::Mbc3,
    },
    0x19 => CartridgeType {
      battery_present: false,
      ram_present: false,
      mapper_type: MapperType::Mbc5,
    },
    0x1A => CartridgeType {
      battery_present: false,
      ram_present: true,
      mapper_type: MapperType::Mbc5,
    },
    0x1B => CartridgeType {
      battery_present: true,
      ram_present: true,
      mapper_type: MapperType::Mbc5,
    },
    0x20 => CartridgeType {
      battery_present: false,
      ram_present: false,
      mapper_type: MapperType::Mbc6,
    },
    0xFE => CartridgeType {
      battery_present: false,
      ram_present: false,
      mapper_type: MapperType::HuC3,
    },
    0xFF => CartridgeType {
      battery_present: true,
      ram_present: true,
      mapper_type: MapperType::HuC1,
    },
    // Note: Not supporting any carts with timers, sensors, or rumble
    _ => panic!("Unsupported cartridge type [{:02X}]", code),
  }
}

fn get_old_publisher(byte: u8) -> String {
  match byte {
    0x00 => "None".into(),
    0x01 => "Nintendo".into(),
    0x08 => "Capcom".into(),
    0x09 => "Hot-B".into(),
    0x0a => "Jaleco".into(),
    0x0b => "Coconuts Japan".into(),
    0x0c => "Elite Systems".into(),
    0x13 => "EA (Electronic Arts)".into(),
    0x18 => "Hudsonsoft".into(),
    0x19 => "ITC Entertainment".into(),
    0x1A => "Yanoman".into(),
    0x1D => "Japan Clary".into(),
    0x1F => "Virgin Interactive".into(),
    0x24 => "PCM Complete".into(),
    0x25 => "San-X".into(),
    0x28 => "Kotobuki Systems".into(),
    0x29 => "Seta".into(),
    0x30 => "Infogrames".into(),
    0x31 => "Nintendo".into(),
    0x32 => "Bandai".into(),
    0x34 => "Konami".into(),
    0x35 => "HectorSoft".into(),
    0x38 => "Capcom".into(),
    0x39 => "Banpresto".into(),
    0x3C => ".Entertainment i".into(),
    0x3E => "Gremlin".into(),
    0x41 => "Ubisoft".into(),
    0x42 => "Atlus".into(),
    0x44 => "Malibu".into(),
    0x46 => "Angel".into(),
    0x47 => "Spectrum Holoby".into(),
    0x49 => "Irem".into(),
    0x4A => "Virgin Interactive".into(),
    0x4D => "Malibu".into(),
    0x4F => "U.S. Gold".into(),
    0x50 => "Absolute".into(),
    0x51 => "Acclaim".into(),
    0x52 => "Activision".into(),
    0x53 => "American Sammy".into(),
    0x54 => "GameTek".into(),
    0x55 => "Park Place".into(),
    0x56 => "LJN".into(),
    0x57 => "Matchbox".into(),
    0x59 => "Milton Bradley".into(),
    0x5A => "Mindscape".into(),
    0x5B => "Romstar".into(),
    0x5C => "Naxat Soft".into(),
    0x5D => "Tradewest".into(),
    0x60 => "Titus".into(),
    0x61 => "Virgin Interactive".into(),
    0x67 => "Ocean Interactive".into(),
    0x69 => "EA (Electronic Arts)".into(),
    0x6E => "Elite Systems".into(),
    0x6F => "Electro Brain".into(),
    0x70 => "Infogrames".into(),
    0x71 => "Interplay".into(),
    0x72 => "Broderbund".into(),
    0x73 => "Sculptered Soft".into(),
    0x75 => "The Sales Curve".into(),
    0x78 => "t.hq".into(),
    0x79 => "Accolade".into(),
    0x7A => "Triffix Entertainment".into(),
    0x7C => "Microprose".into(),
    0x7F => "Kemco".into(),
    0x80 => "Misawa Entertainment".into(),
    0x83 => "Lozc".into(),
    0x86 => "Tokuma Shoten Intermedia".into(),
    0x8B => "Bullet-Proof Software".into(),
    0x8C => "Vic Tokai".into(),
    0x8E => "Ape".into(),
    0x8F => "I’Max".into(),
    0x91 => "Chunsoft Co.".into(),
    0x92 => "Video System".into(),
    0x93 => "Tsubaraya Productions Co.".into(),
    0x95 => "Varie Corporation".into(),
    0x96 => "Yonezawa/S’Pal".into(),
    0x97 => "Kaneko".into(),
    0x99 => "Arc".into(),
    0x9A => "Nihon Bussan".into(),
    0x9B => "Tecmo".into(),
    0x9C => "Imagineer".into(),
    0x9D => "Banpresto".into(),
    0x9F => "Nova".into(),
    0xA1 => "Hori Electric".into(),
    0xA2 => "Bandai".into(),
    0xA4 => "Konami".into(),
    0xA6 => "Kawada".into(),
    0xA7 => "Takara".into(),
    0xA9 => "Technos Japan".into(),
    0xAA => "Broderbund".into(),
    0xAC => "Toei Animation".into(),
    0xAD => "Toho".into(),
    0xAF => "Namco".into(),
    0xB0 => "acclaim".into(),
    0xB1 => "ASCII or Nexsoft".into(),
    0xB2 => "Bandai".into(),
    0xB4 => "Square Enix".into(),
    0xB6 => "HAL Laboratory".into(),
    0xB7 => "SNK".into(),
    0xB9 => "Pony Canyon".into(),
    0xBA => "Culture Brain".into(),
    0xBB => "Sunsoft".into(),
    0xBD => "Sony Imagesoft".into(),
    0xBF => "Sammy".into(),
    0xC0 => "Taito".into(),
    0xC2 => "Kemco".into(),
    0xC3 => "Squaresoft".into(),
    0xC4 => "Tokuma Shoten Intermedia".into(),
    0xC5 => "Data East".into(),
    0xC6 => "Tonkinhouse".into(),
    0xC8 => "Koei".into(),
    0xC9 => "UFL".into(),
    0xCA => "Ultra".into(),
    0xCB => "Vap".into(),
    0xCC => "Use Corporation".into(),
    0xCD => "Meldac".into(),
    0xCE => ".Pony Canyon or".into(),
    0xCF => "Angel".into(),
    0xD0 => "Taito".into(),
    0xD1 => "Sofel".into(),
    0xD2 => "Quest".into(),
    0xD3 => "Sigma Enterprises".into(),
    0xD4 => "ASK Kodansha Co.".into(),
    0xD6 => "Naxat Soft".into(),
    0xD7 => "Copya System".into(),
    0xD9 => "Banpresto".into(),
    0xDA => "Tomy".into(),
    0xDB => "LJN".into(),
    0xDD => "NCS".into(),
    0xDE => "Human".into(),
    0xDF => "Altron".into(),
    0xE0 => "Jaleco".into(),
    0xE1 => "Towa Chiki".into(),
    0xE2 => "Yutaka".into(),
    0xE3 => "Varie".into(),
    0xE5 => "Epcoh".into(),
    0xE7 => "Athena".into(),
    0xE8 => "Asmik ACE Entertainment".into(),
    0xE9 => "Natsume".into(),
    0xEA => "King Records".into(),
    0xEB => "Atlus".into(),
    0xEC => "Epic/Sony Records".into(),
    0xEE => "IGS".into(),
    0xF0 => "A Wave".into(),
    0xF3 => "Extreme Entertainment".into(),
    0xFF => "LJN".into(),
    _ => format!("Unknown (OLD) [{:02X}]", byte),
  }
}

fn get_new_publisher(code: &str) -> String {
  match &*code.to_uppercase() {
    "00" => "None".into(),
    "01" => "Nintendo R&D1".into(),
    "08" => "Capcom".into(),
    "13" => "Electronic Arts".into(),
    "18" => "Hudson Soft".into(),
    "19" => "b-ai".into(),
    "20" => "kss".into(),
    "22" => "pow".into(),
    "24" => "PCM Complete".into(),
    "25" => "san-x".into(),
    "28" => "Kemco Japan".into(),
    "29" => "seta".into(),
    "30" => "Viacom".into(),
    "31" => "Nintendo".into(),
    "32" => "Bandai".into(),
    "33" => "Ocean/Acclaim".into(),
    "34" => "Konami".into(),
    "35" => "Hector".into(),
    "37" => "Taito".into(),
    "38" => "Hudson".into(),
    "39" => "Banpresto".into(),
    "41" => "Ubi Soft".into(),
    "42" => "Atlus".into(),
    "44" => "Malibu".into(),
    "46" => "angel".into(),
    "47" => "Bullet-Proof".into(),
    "49" => "irem".into(),
    "50" => "Absolute".into(),
    "51" => "Acclaim".into(),
    "52" => "Activision".into(),
    "53" => "American sammy".into(),
    "54" => "Konami".into(),
    "55" => "Hi tech entertainment".into(),
    "56" => "LJN".into(),
    "57" => "Matchbox".into(),
    "58" => "Mattel".into(),
    "59" => "Milton Bradley".into(),
    "60" => "Titus".into(),
    "61" => "Virgin".into(),
    "64" => "LucasArts".into(),
    "67" => "Ocean".into(),
    "69" => "Electronic Arts".into(),
    "70" => "Infogrames".into(),
    "71" => "Interplay".into(),
    "72" => "Broderbund".into(),
    "73" => "sculptured".into(),
    "75" => "sci".into(),
    "78" => "THQ".into(),
    "79" => "Accolade".into(),
    "80" => "misawa".into(),
    "83" => "lozc".into(),
    "86" => "Tokuma Shoten Intermedia".into(),
    "87" => "Tsukuda Original".into(),
    "91" => "Chunsoft".into(),
    "92" => "Video system".into(),
    "93" => "Ocean/Acclaim".into(),
    "95" => "Varie".into(),
    "96" => "Yonezawa/s’pal".into(),
    "97" => "Kaneko".into(),
    "99" => "Pack in soft".into(),
    "9H" => "Bottom Up".into(),
    "A4" => "Konami (Yu-Gi-Oh!)".into(),
    _ => format!("Unknown (NEW) [\"{}\"]", code),
  }
}
