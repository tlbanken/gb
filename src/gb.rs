//! Main gameboy system module

#[allow(unused)]
use log::{debug, error, info, trace, warn, LevelFilter};

use std::cell::RefCell;
use std::rc::Rc;

use crate::bus::*;
use crate::cart::Cartridge;
use crate::cpu::Cpu;
use crate::err::{GbError, GbErrorType, GbResult};
use crate::event::UserEvent;
use crate::gb_err;
use crate::logger::Logger;
use crate::ram::*;
use crate::screen::{Color, Pos};
use crate::state::GbState;
use crate::ui::Ui;
use crate::video::Video;

use egui;
use egui_winit::winit;
use egui_winit::winit::event_loop::{EventLoopBuilder, EventLoopWindowTarget};
use egui_winit::winit::{
  event::{Event, WindowEvent},
  event_loop::ControlFlow,
  window::{Window, WindowBuilder},
};

static mut LOGGER: Logger = Logger::const_default();

// window constants
const SCALE_FACTOR: u32 = 10;
const INITIAL_WIDTH: u32 = 160 * SCALE_FACTOR;
const INITIAL_HEIGHT: u32 = 144 * SCALE_FACTOR;

pub struct Gameboy {
  is_init: bool,
  state: GbState,
  video: Option<Video>,
}

impl Gameboy {
  pub fn new(level_filter: LevelFilter) -> Gameboy {
    init_logging(level_filter);

    let state = GbState {
      bus: Rc::new(RefCell::new(Bus::new())),
      eram: Rc::new(RefCell::new(Ram::new(8 * 1024))),
      wram: Rc::new(RefCell::new(Ram::new(8 * 1024))),
      cart: Rc::new(RefCell::new(Cartridge::new())),
      cpu: Rc::new(RefCell::new(Cpu::new())),
    };

    Gameboy {
      state,
      is_init: false,
      video: None,
    }
  }

  pub fn init(&mut self) -> GbResult<()> {
    info!("Initializing system");

    self.state.init();

    self.is_init = true;
    Ok(())
  }

  pub fn run(mut self) -> GbResult<()> {
    if !self.is_init {
      return gb_err!(GbErrorType::NotInitialized);
    }
    info!("Starting emulation");

    // build event loop and window with custom event support
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let window = WindowBuilder::new()
      .with_decorations(true)
      .with_resizable(true)
      .with_transparent(false)
      .with_title("Gameboy Emulator")
      .with_inner_size(winit::dpi::PhysicalSize {
        width: INITIAL_WIDTH,
        height: INITIAL_HEIGHT,
      })
      .build(&event_loop)
      .unwrap();

    // setup ui
    let ui = Ui::new(event_loop.create_proxy());

    // setup render backend
    self.video = Some(pollster::block_on(Video::new(window, ui)));

    // run as fast as possible
    event_loop.run(move |event, _, control_flow| {
      // run as fast as possible
      control_flow.set_poll();

      let should_redraw = self.handle_events(event, control_flow);

      // system step
      // self.state.step().unwrap();

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
      if should_redraw {
        self.video.as_mut().unwrap().render().unwrap();
      }
    });
    // no return
  }

  fn handle_events<T>(&mut self, event: Event<T>, control_flow: &mut ControlFlow) -> bool {
    match event {
      // window events
      Event::WindowEvent {
        event: wevent,
        window_id: _,
      } => {
        match wevent {
          WindowEvent::CloseRequested => {
            control_flow.set_exit();
          }
          _ => (),
        };
        self.video.as_mut().unwrap().handle_window_event(wevent)
      }
      Event::RedrawRequested(_) => true,
      _ => false,
    }
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
