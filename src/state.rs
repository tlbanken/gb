//! Gameboy state

use egui_winit::winit::event_loop::EventLoopProxy;
use std::{cell::RefCell, rc::Rc};

use crate::int::Interrupts;
use crate::screen::Screen;
use crate::tick_counter::TickCounter;
use crate::timer::Timer;
use crate::{
  bus::Bus, cart::Cartridge, cpu, cpu::Cpu, err::GbResult, joypad::Joypad, ppu::Ppu, ram::Ram,
};

use crate::event::UserEvent;
use log::{error, warn};

/// Alpha used when calculating the rolling average
const CLOCK_RATE_ALPHA: f32 = 0.9;
const GB_FPS_ALPHA: f32 = 0.9;

#[derive(Copy, Clone)]
pub struct EmuFlow {
  pub paused: bool,
  pub step: bool,
  pub speed: f32,
}

impl EmuFlow {
  pub fn new(paused: bool, step: bool, speed: f32) -> EmuFlow {
    EmuFlow {
      paused,
      step,
      speed,
    }
  }
}

pub struct GbState {
  pub bus: Rc<RefCell<Bus>>,
  pub wram: Rc<RefCell<Ram>>,
  pub hram: Rc<RefCell<Ram>>,
  pub cart: Rc<RefCell<Cartridge>>,
  pub cpu: Rc<RefCell<Cpu>>,
  pub ppu: Rc<RefCell<Ppu>>,
  pub ic: Rc<RefCell<Interrupts>>,
  pub timer: Rc<RefCell<Timer>>,
  pub joypad: Rc<RefCell<Joypad>>,
  pub flow: EmuFlow,
  pub cycles: TickCounter,
  pub gb_fps: TickCounter,
  pub clock_rate: f32,
  pub event_loop_proxy: Option<EventLoopProxy<UserEvent>>,
}

impl GbState {
  pub fn new(flow: EmuFlow) -> GbState {
    GbState {
      bus: Rc::new(RefCell::new(Bus::new())),
      wram: Rc::new(RefCell::new(Ram::new(8 * 1024))),
      hram: Rc::new(RefCell::new(Ram::new(127))),
      cart: Rc::new(RefCell::new(Cartridge::new())),
      cpu: Rc::new(RefCell::new(Cpu::new())),
      ppu: Rc::new(RefCell::new(Ppu::new())),
      ic: Rc::new(RefCell::new(Interrupts::new())),
      timer: Rc::new(RefCell::new(Timer::new())),
      joypad: Rc::new(RefCell::new(Joypad::new())),
      flow,
      cycles: TickCounter::new(CLOCK_RATE_ALPHA),
      gb_fps: TickCounter::new(GB_FPS_ALPHA),
      clock_rate: 0.0,
      event_loop_proxy: None,
    }
  }

  pub fn init(
    &mut self,
    screen: Rc<RefCell<Screen>>,
    event_loop_proxy: EventLoopProxy<UserEvent>,
  ) -> GbResult<()> {
    // TODO: load cartridge

    // connect PPU to screen
    self.ppu.borrow_mut().connect_screen(screen)?;

    // connect interrupts to cpu
    self.ic.borrow_mut().connect_cpu(self.cpu.clone())?;

    // connect Bus to memory
    self.bus.borrow_mut().connect_wram(self.wram.clone())?;
    self.bus.borrow_mut().connect_hram(self.hram.clone())?;
    self.bus.borrow_mut().connect_cartridge(self.cart.clone())?;
    self.bus.borrow_mut().connect_ppu(self.ppu.clone())?;
    self.bus.borrow_mut().connect_ic(self.ic.clone())?;
    self.bus.borrow_mut().connect_timer(self.timer.clone())?;
    self.bus.borrow_mut().connect_joypad(self.joypad.clone())?;

    // connect modules to bus
    self.cpu.borrow_mut().connect_bus(self.bus.clone())?;

    // connect modules to interrupt controller
    self.timer.borrow_mut().connect_ic(self.ic.clone())?;
    self.ppu.borrow_mut().connect_ic(self.ic.clone())?;

    // connect proxy
    self.event_loop_proxy = Some(event_loop_proxy);

    Ok(())
  }

  pub fn step(&mut self) -> GbResult<()> {
    if self.flow.paused && !self.flow.step {
      self.clock_rate = 0.0;
      return Ok(());
    }

    if self.flow.step {
      self.clock_rate = 0.0;
      self.step_one()?;
    } else {
      self.step_chunk()?;
    }

    self.flow.step = false;
    Ok(())
  }

  fn step_chunk(&mut self) -> GbResult<()> {
    // if we are running too fast, skip
    let clock_rate = self.cycles.tps();
    let target_pace = cpu::CLOCK_RATE * self.flow.speed;
    if clock_rate > target_pace {
      return Ok(());
    }
    // only show clock rate when we are doing work
    self.clock_rate = clock_rate;

    // how many steps in a chunk
    const CHUNK_SIZE: u32 = 4;

    for _ in 0..CHUNK_SIZE {
      self.step_one()?;
    }

    Ok(())
  }

  #[inline]
  fn step_one(&mut self) -> GbResult<()> {
    let cycle_budget = self.cpu.borrow_mut().step()?;
    for _ in 0..cycle_budget {
      self.cycles.tick();
    }
    if self.ppu.borrow_mut().step(cycle_budget)? {
      self.gb_fps.tick();
      match &self.event_loop_proxy {
        Some(elp) => elp.send_event(UserEvent::RequestRender).unwrap(),
        None => panic!(),
      }
    }
    self.ic.borrow_mut().step();
    self.timer.borrow_mut().step(cycle_budget);
    Ok(())
  }
}
