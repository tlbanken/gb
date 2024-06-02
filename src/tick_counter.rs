use log::{error, info};
use std::time::{Duration, Instant};

pub struct TickCounter {
  ticks: u64,
  avg_tps: f32,
  alpha: f32,
  last_calc: Instant,
}

impl TickCounter {
  pub fn new(alpha: f32) -> TickCounter {
    TickCounter {
      ticks: 0,
      avg_tps: 1.0,
      last_calc: Instant::now(),
      alpha,
    }
  }

  pub fn tick(&mut self) {
    self.ticks = self.ticks.wrapping_add(1);
  }

  /// Get Ticks per Second using a moving weighted average
  pub fn tps(&mut self) -> f32 {
    let now = Instant::now();
    let dtime = (now - self.last_calc).as_secs_f32();
    if dtime == 0.0 {
      self.avg_tps = 60.0;
      return self.avg_tps;
    }
    let calc_rate = 0.01;
    if dtime >= calc_rate {
      // only update after 1 second
      let tps = self.ticks as f32 / dtime;
      self.last_calc = now;
      self.ticks = 0;
      // moving weighted average
      self.avg_tps = self.alpha * self.avg_tps + (1.0 - self.alpha) * tps;
    }
    self.avg_tps
  }
}
