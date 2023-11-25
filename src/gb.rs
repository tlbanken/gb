//! Main gameboy system module

#[allow(unused)]
use log::{debug, error, info, trace, warn, LevelFilter};
// use wgpu::{Backends, Instance, InstanceDescriptor};
// use winit::raw_window_handle::HasWindowHandle;

use std::cell::RefCell;
use std::rc::Rc;

use crate::bus::*;
use crate::cart::Cartridge;
use crate::cpu::Cpu;
use crate::err::{GbError, GbErrorType, GbResult};
use crate::gb_err;
use crate::logger::Logger;
use crate::ram::*;
use crate::screen::{Color, Pos};
use crate::video::Video;

use winit::event_loop::{EventLoopBuilder, EventLoopWindowTarget};
// use winit::window::Window;
use winit::{
  event::{Event, WindowEvent},
  event_loop::ControlFlow,
  window::WindowBuilder,
};

static mut LOGGER: Logger = Logger::const_default();

// window constants
// const INITIAL_WIDTH: u32 = 1920;
// const INITIAL_HEIGHT: u32 = 1080;
const SCALE_FACTOR: u32 = 10;
const INITIAL_WIDTH: u32 = 160 * SCALE_FACTOR;
const INITIAL_HEIGHT: u32 = 144 * SCALE_FACTOR;

struct DebugState {
  pub halt: bool,
  pub step: bool,
}

// custom events for emulation flow
enum GbEvent {
  RequestRedraw,
}

pub struct Gameboy {
  is_init: bool,
  bus: Rc<RefCell<Bus>>,
  eram: Rc<RefCell<Ram>>,
  wram: Rc<RefCell<Ram>>,
  cart: Rc<RefCell<Cartridge>>,
  cpu: Cpu,
  video: Option<Video>,
}

impl Gameboy {
  pub fn new(level_filter: LevelFilter) -> Gameboy {
    init_logging(level_filter);

    Gameboy {
      is_init: false,
      bus: Rc::new(RefCell::new(Bus::new())),
      eram: Rc::new(RefCell::new(Ram::new(8 * 1024))),
      wram: Rc::new(RefCell::new(Ram::new(8 * 1024))),
      cart: Rc::new(RefCell::new(Cartridge::new())),
      cpu: Cpu::new(),
      video: None,
    }
  }

  pub fn init(&mut self) -> GbResult<()> {
    info!("Initializing system");

    // TODO: load cartridge

    // connect Bus to memory
    self.bus.borrow_mut().connect_eram(self.eram.clone())?;
    self.bus.borrow_mut().connect_wram(self.wram.clone())?;
    self.bus.borrow_mut().connect_cartridge(self.cart.clone())?;

    // connect modules to bus
    self.cpu.connect_bus(self.bus.clone())?;

    self.is_init = true;
    Ok(())
  }

  pub async fn run(mut self) -> GbResult<()> {
    if !self.is_init {
      return gb_err!(GbErrorType::NotInitialized);
    }
    info!("Starting emulation");

    // build event loop and window with custom event support
    let event_loop = EventLoopBuilder::<GbEvent>::with_user_event()
      .build()
      .unwrap();
    let window = WindowBuilder::new()
      .with_decorations(true)
      .with_resizable(false)
      .with_transparent(false)
      .with_title("Gameboy Emulator")
      .with_inner_size(winit::dpi::PhysicalSize {
        width: INITIAL_WIDTH,
        height: INITIAL_HEIGHT,
      })
      .build(&event_loop)
      .unwrap();

    self.video = Some(Video::new(window).await);

    // run as fast as possible
    event_loop.set_control_flow(ControlFlow::Poll);
    event_loop
      .run(|event, elwt| {
        self.handle_events(event, elwt);

        // update the debug view
        let debug_state = self.update_debug().unwrap();

        // system step
        if !debug_state.halt || (debug_state.halt && debug_state.step) {
          self.step().unwrap();
        }

        // demo draw
        for y in 0..144 {
          for x in 0..160 {
            self.video.as_mut().unwrap().set_pixel(
              Pos { x, y },
              Color::new(y as f32 / 144.0, x as f32 / 160.0, 0.0),
            );
          }
        }

        // draw the window
        self.video.as_mut().unwrap().render().unwrap();
      })
      .unwrap();

    info!("Exiting emulation :)");
    Ok(())
  }

  fn step(&mut self) -> GbResult<()> {
    self.cpu.step()?;
    Ok(())
  }

  fn handle_events<T>(&mut self, event: Event<T>, elwt: &EventLoopWindowTarget<T>) {
    match event {
      Event::WindowEvent {
        event: WindowEvent::CloseRequested,
        ..
      } => {
        elwt.exit();
      }
      _ => (),
    }
  }

  fn update_debug(&mut self) -> GbResult<DebugState> {
    Ok(DebugState {
      halt: true,
      step: false,
    })
  }
}

// Initialize logging and set the level filter
fn init_logging(level_filter: LevelFilter) {
  log::set_max_level(level_filter);
  unsafe {
    LOGGER = Logger::new(level_filter);
    match log::set_logger(&LOGGER) {
      Ok(()) => {}
      Err(msg) => panic!("Failed to initialize logging: {}", msg),
    }
  }
  error!("Log Level ERROR Enabled!");
  warn!("Log Level WARN Enabled!");
  info!("Log Level INFO Enabled!");
  debug!("Log Level DEBUG Enabled!");
  trace!("Log Level TRACE Enabled!");
}
