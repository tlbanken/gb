//! Main gameboy system module

use log::{debug, error, info, trace, warn, LevelFilter};

use std::time::Instant;
use std::sync::Arc;

use crate::err::GbResult;
use crate::event::UserEvent;
use crate::keybinds::{Action, EmuControl};
use crate::logger::Logger;
use crate::state::{EmuFlow, GbState};
use crate::ui::Ui;
use crate::video::Video;

use egui_winit::winit;
use egui_winit::winit::application::ApplicationHandler;
use egui_winit::winit::event::{self, WindowEvent};
use egui_winit::winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use egui_winit::winit::window::{Window, WindowId};

static mut LOGGER: Logger = Logger::const_default();

// window constants
const SCALE_FACTOR: u32 = 10;
const INITIAL_WIDTH: u32 = 160 * SCALE_FACTOR;
const INITIAL_HEIGHT: u32 = 144 * SCALE_FACTOR;

// target frame time (60 fps)
const TARGET_FRAME_TIME_MS: u128 = 1000 / 60;

pub struct Gameboy {
  state: GbState,
  last_render: Instant,
  rom_path: Option<std::path::PathBuf>,
  video: Option<Video>,
  event_loop_proxy: Option<winit::event_loop::EventLoopProxy<UserEvent>>,
}

impl Gameboy {
  pub fn new(level_filter: LevelFilter) -> Gameboy {
    init_logging(level_filter);

    let state = GbState::new(EmuFlow::new(false, false, 1.0));

    Gameboy {
      state,
      last_render: Instant::now(),
      rom_path: None,
      video: None,
      event_loop_proxy: None,
    }
  }

  #[allow(dead_code)]
  pub fn run(self) -> GbResult<()> {
    self.run_with_rom(None)
  }

  pub fn run_with_rom(mut self, rom_path: Option<std::path::PathBuf>) -> GbResult<()> {
    info!("Starting emulation");
    self.rom_path = rom_path;

    // Build the event loop with custom event support. In winit 0.30, this uses the builder
    // pattern on the EventLoop struct. We also create the EventLoopProxy here to store it
    // on Gameboy, as the ActiveEventLoop inside resumed() does not allow proxy generation.
    let event_loop = EventLoop::<UserEvent>::with_user_event().build().unwrap();
    self.event_loop_proxy = Some(event_loop.create_proxy());

    event_loop.run_app(&mut self).unwrap();
    Ok(())
  }

  fn handle_keyboard_input(&self, event: &event::KeyEvent) {
    if let winit::keyboard::PhysicalKey::Code(keycode) = event.physical_key {
      if let Some(action) = self.state.keybinds.translate(keycode) {
        match action {
          Action::Joypad(joypad_input) => {
            match event.state {
              event::ElementState::Pressed => self.state.joypad.borrow_mut().set_input(joypad_input),
              event::ElementState::Released => self.state.joypad.borrow_mut().clear_input(joypad_input),
            }
          }
          Action::Control(emu_control) => {
            // Placeholder: Emulation control features can be handled here in the future
            if event.state == event::ElementState::Pressed {
              match emu_control {
                EmuControl::Pause => {}
                EmuControl::Reset => {}
              }
            }
          }
        }
      }
    }
  }
}

// Implement ApplicationHandler to handle winit 0.30's event loop callbacks.
impl ApplicationHandler<UserEvent> for Gameboy {
  // resumed() is called when the application starts or is resumed. Under winit 0.30,
  // we must create the Window and configure the graphics backend inside this callback
  // instead of synchronously before the event loop starts.
  fn resumed(&mut self, event_loop: &ActiveEventLoop) {
    if self.video.is_none() {
      event_loop.set_control_flow(ControlFlow::Poll);

      // Create window using the ActiveEventLoop instance.
      let window_attributes = Window::default_attributes()
        .with_decorations(true)
        .with_resizable(true)
        .with_transparent(false)
        .with_title("~ Enter the Gameboy Emulation ~")
        .with_inner_size(winit::dpi::PhysicalSize {
          width: INITIAL_WIDTH,
          height: INITIAL_HEIGHT,
        });
      let window = Arc::new(event_loop.create_window(window_attributes).unwrap());

      let proxy = self.event_loop_proxy.clone().unwrap();

      // setup ui
      let ui = Ui::new(proxy.clone());

      // setup render backend
      let video = pollster::block_on(Video::new(window, ui));

      // initialize the gb state
      self.state.init(video.screen(), proxy, self.rom_path.take()).unwrap();

      self.video = Some(video);
      self.last_render = Instant::now();
    }
  }

  fn window_event(
    &mut self,
    event_loop: &ActiveEventLoop,
    _window_id: WindowId,
    event: WindowEvent,
  ) {
    match &event {
      WindowEvent::KeyboardInput { event: key_event, .. } => {
        self.handle_keyboard_input(key_event);
      }
      WindowEvent::CloseRequested => {
        event_loop.exit();
      }
      _ => (),
    }
    if let Some(video) = &mut self.video {
      video.handle_window_event(&event);
    }
  }

  fn user_event(&mut self, _event_loop: &ActiveEventLoop, event: UserEvent) {
    let video = self.video.as_mut().unwrap();
    match event {
      UserEvent::RequestResize(w, h) => {
        let _ = video
          .window()
          .request_inner_size(winit::dpi::PhysicalSize::new(w, h));
      }
      UserEvent::RequestRender => {
        self.last_render = Instant::now();
        video.render(&mut self.state).unwrap();
      }
      UserEvent::EmuPause => self.state.flow.paused = true,
      UserEvent::EmuPlay => self.state.flow.paused = false,
      UserEvent::EmuStep => self.state.flow.step = true,
      UserEvent::EmuReset(path) => {
        let flow = self.state.flow;
        let elp = self.state.event_loop_proxy.clone();
        self.state = GbState::new(flow);
        self.state.init(video.screen(), elp.unwrap(), None).unwrap();
        if let Some(path_unwrapped) = path {
          self.state.cart.borrow_mut().load(path_unwrapped).unwrap();
        }
      }
    }
  }

  fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
    // system step
    self.state.step().unwrap();

    // draw the window at least every 1/60 of a second
    let now = Instant::now();
    let dtime = now - self.last_render;
    let should_redraw = dtime.as_millis() > TARGET_FRAME_TIME_MS;
    if should_redraw {
      self.last_render = now;
      if let Some(video) = &mut self.video {
        video.render(&mut self.state).unwrap();
      }
    }
  }
}

// Initialize logging and set the level filter
pub fn init_logging(level_filter: LevelFilter) {
  log::set_max_level(level_filter);
  unsafe {
    LOGGER = Logger::new(level_filter);
    match log::set_logger(&*std::ptr::addr_of!(LOGGER)) {
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
