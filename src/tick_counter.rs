use std::time::Instant;

pub struct TickCounter {
  ticks: u64,
  avg_tps: f32,
  alpha: f32,
  last_calc: Instant,
  calc_rate: f32,
}

impl TickCounter {
  pub fn new(alpha: f32, calc_rate: f32) -> TickCounter {
    TickCounter {
      ticks: 0,
      avg_tps: 0.0,
      last_calc: Instant::now(),
      alpha,
      calc_rate,
    }
  }

  #[inline]
  pub fn tick(&mut self) {
    self.ticks = self.ticks.wrapping_add(1);
  }

  #[inline]
  pub fn tick_by(&mut self, amount: u64) {
    self.ticks = self.ticks.wrapping_add(amount);
  }

  /// Get Ticks per Second using a moving weighted average
  pub fn tps(&mut self) -> f32 {
    let now = Instant::now();
    let dtime = (now - self.last_calc).as_secs_f32();
    if dtime == 0.0 {
      return self.avg_tps;
    }
    if dtime >= self.calc_rate {
      let tps = self.ticks as f32 / dtime;
      self.last_calc = now;
      self.ticks = 0;
      // if first calculation, set directly to avoid startup lag
      if self.avg_tps == 0.0 {
        self.avg_tps = tps;
      } else {
        // moving weighted average
        self.avg_tps = self.alpha * self.avg_tps + (1.0 - self.alpha) * tps;
      }
    }
    self.avg_tps
  }
}
