//! Base class for all mappers

use crate::err::GbResult;

#[derive(Debug)]
pub enum MapperType {
  None,
  Mbc1,
  Mbc2,
  Mbc3,
  Mbc4,
  Mbc5,
  Mbc6,
  Mbc7,
  Mmm01,
  M161,
  HuC1,
  HuC3,
  Other,
}

pub trait Mapper {
  fn read(&self, addr: u16) -> GbResult<u8>;
  fn write(&mut self, addr: u16, val: u8) -> GbResult<()>;
}
