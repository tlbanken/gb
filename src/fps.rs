use std::time::{Duration, Instant};

pub struct Fps {
  frames: u32,
  fps: u32,
  last_calc: Instant,
}

impl Fps {
  pub fn new() -> Fps {
    Fps {
      frames: 0,
      fps: 0,
      last_calc: Instant::now(),
    }
  }

  pub fn tick(&mut self) {
    self.frames += 1;
    let now = Instant::now();
    if (now - self.last_calc).as_secs_f32() > 1.0 {
      self.fps = self.frames;
      self.frames = 0;
      self.last_calc = now;
    }
  }

  pub fn fps(&self) -> u32 {
    self.fps
  }
}
