//! Main gameboy system module

use egui_winit::winit::dpi::{LogicalSize, PhysicalSize};
#[allow(unused)]
use log::{debug, error, info, trace, warn, LevelFilter};

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Instant;

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

// target frame time (60 fps)
const TARGET_FRAME_TIME_MS: u128 = 1000 / 60;

pub struct Gameboy {
  is_init: bool,
  state: GbState,
  // video: Option<Video>,
}

impl Gameboy {
  pub fn new(level_filter: LevelFilter) -> Gameboy {
    init_logging(level_filter);

    let state = GbState::new(false);

    Gameboy {
      state,
      is_init: false,
      // video: None,
    }
  }

  pub fn run(mut self) -> GbResult<()> {
    info!("Starting emulation");

    // build event loop and window with custom event support
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let window = WindowBuilder::new()
      .with_decorations(true)
      .with_resizable(true)
      .with_transparent(false)
      .with_title("~ Enter the Gameboy Emulation ~")
      .with_inner_size(winit::dpi::PhysicalSize {
        width: INITIAL_WIDTH,
        height: INITIAL_HEIGHT,
      })
      .build(&event_loop)
      .unwrap();

    // setup ui
    let ui = Ui::new(event_loop.create_proxy());

    // setup render backend
    let mut video = pollster::block_on(Video::new(window, ui));
    // self.video = Some(pollster::block_on(Video::new(window, ui)));

    // initialize the gb state
    self.state.init(video.screen())?;

    let mut last_render = Instant::now();
    // run as fast as possible
    event_loop.run(move |event, _, control_flow| {
      // run as fast as possible
      control_flow.set_poll();

      self.handle_events(event, control_flow, &mut video).unwrap();

      // system step
      self.state.step().unwrap();

      // demo draw
      // for y in 0..144 {
      //   for x in 0..160 {
      //     self.video.as_mut().unwrap().set_pixel(
      //       Pos { x, y },
      //       Color::new(y as f32 / 144.0, x as f32 / 160.0, 0.0),
      //     );
      //   }
      // }

      // TODO: find better pace for rendering
      // draw the window at least every 1/60 of a second
      let now = Instant::now();
      let dtime = now - last_render;
      let should_redraw = if dtime.as_millis() > TARGET_FRAME_TIME_MS {
        last_render = now;
        true
      } else {
        false
      };

      if should_redraw {
        video.render(&mut self.state).unwrap();
      }
    });
    // no return
  }

  fn handle_events(
    &mut self,
    event: Event<UserEvent>,
    control_flow: &mut ControlFlow,
    video: &mut Video,
  ) -> GbResult<()> {
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
        video.handle_window_event(wevent);
      }
      Event::UserEvent(event) => match event {
        UserEvent::RequestResize(w, h) => {
          video
            .window()
            .set_inner_size(PhysicalSize::new(w as f32, h as f32));
        }
        UserEvent::EmuPause => self.state.flow.paused = true,
        UserEvent::EmuPlay => self.state.flow.paused = false,
        UserEvent::EmuStep => self.state.flow.step = true,
        UserEvent::EmuReset(path) => {
          let paused = self.state.flow.paused;
          self.state = GbState::new(paused);
          self.state.init(video.screen())?;
          if let Some(path_unwrapped) = path {
            self.state.cart.borrow_mut().load(path_unwrapped)?;
          }
        }
        _ => {}
      },
      _ => {}
    }
    Ok(())
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
