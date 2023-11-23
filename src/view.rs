//! Frontend window for the gameboy

use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::Color;
use sdl2::render::Canvas;
use sdl2::video::Window;
use sdl2::{Sdl, VideoSubsystem};

use crate::cpu::Cpu;
use crate::err::GbResult;
use crate::ram::Ram;

const WIN_WIDTH: u32 = 800;
const WIN_HEIGHT: u32 = 600;

pub struct DebugState {
  pub halt: bool,
  pub step: bool,
}

pub struct View {
  sdl_ctx: Sdl,
  canvas: Canvas<Window>,
  video: VideoSubsystem,
}

impl View {
  pub fn new() -> View {
    // set up sdl window
    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();
    let window = video_subsystem
      .window("Gameboy Emulator", WIN_WIDTH, WIN_HEIGHT)
      .position_centered()
      .build()
      .unwrap();

    View {
      sdl_ctx: sdl_context,
      video: video_subsystem,
      canvas: window.into_canvas().build().unwrap(),
    }
  }

  pub fn present(&mut self) -> GbResult<()> {
    self.canvas.set_draw_color(Color::RGB(20, 20, 20));
    self.canvas.clear();
    self.canvas.present();
    Ok(())
  }

  pub fn should_quit(&self) -> bool {
    let mut event_pump = self.sdl_ctx.event_pump().unwrap();
    for event in event_pump.poll_iter() {
      match event {
        Event::Quit { .. }
        | Event::KeyDown {
          keycode: Some(Keycode::Escape),
          ..
        } => return true,
        _ => {}
      }
    }
    return false;
  }

  pub fn update_debug(
    &mut self,
    cpu: &mut Cpu,
    eram: &mut Ram,
    wram: &mut Ram,
  ) -> GbResult<(DebugState)> {
    Ok(DebugState {
      halt: true,
      step: false,
    })
  }
}
