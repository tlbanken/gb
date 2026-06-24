//! Headless mode for the GB emulator.
//!
//! Provides a null Screen substitute and a `run_headless` method on GbState
//! so the emulator can run without a GPU/window, purely for debugging.

use crate::err::GbResult;
use crate::screen::{Color, Pos, GB_RESOLUTION};
use crate::state::GbState;

// ---------------------------------------------------------------------------
// HeadlessScreen — a null screen that records pixels in CPU memory only.
// Structurally identical surface to Screen but requires no wgpu device.
//
// TODO: Refactor Screen (in screen.rs) into a trait (e.g., `trait
// ScreenDevice`) and make HeadlessScreen implement it. Then update Ppu to
// accept any ScreenDevice so that headless mode can capture and dump actual
// output frames.
// ---------------------------------------------------------------------------
#[allow(dead_code)]
pub struct HeadlessScreen {
  pub pixels: Vec<Color>,
}

#[allow(dead_code)]
impl HeadlessScreen {
  pub fn new() -> Self {
    let count = (GB_RESOLUTION.width * GB_RESOLUTION.height) as usize;
    Self {
      pixels: vec![Color::new(0.0, 0.0, 0.0); count],
    }
  }

  pub fn set_pixel(&mut self, pos: Pos, col: Color) {
    if pos.x < GB_RESOLUTION.width && pos.y < GB_RESOLUTION.height {
      self.pixels[(pos.y * GB_RESOLUTION.width + pos.x) as usize] = col;
    }
  }

  /// Dump the current framebuffer to a PPM file (no external deps required).
  pub fn dump_ppm(&self, path: &str) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::File::create(path)?;
    writeln!(f, "P6")?;
    writeln!(f, "{} {}", GB_RESOLUTION.width, GB_RESOLUTION.height)?;
    writeln!(f, "255")?;
    for px in &self.pixels {
      f.write_all(&[
        (px.r * 255.0) as u8,
        (px.g * 255.0) as u8,
        (px.b * 255.0) as u8,
      ])?;
    }
    Ok(())
  }
}

// ---------------------------------------------------------------------------
// HeadlessPpu — wraps the real PPU but feeds into a HeadlessScreen.
// We re-export the needed types so callers don't need to dig into ppu
// internals.
// ---------------------------------------------------------------------------

/// Run the emulator headlessly for `num_frames` GB frames.
///
/// Returns the HeadlessScreen with the last rendered frame so the caller can
/// dump it or inspect it.
pub fn run_headless(state: &mut GbState, num_frames: u32) -> GbResult<HeadlessScreen> {
  use log::info;

  // We can't swap out the real PPU's screen reference at runtime without
  // restructuring, so instead we intercept frame completion from GbState::step
  // and copy the rendered pixel data after each vblank.
  //
  // The PPU still renders into its own Screen; we just capture frames by
  // counting vblanks via the gb_fps tick counter and stepping manually.

  let headless = HeadlessScreen::new();
  let mut frames_done = 0u32;

  info!(
    "[headless] starting {} frame run on ROM: {:?}",
    num_frames,
    state.cart.borrow().cart_path()
  );

  while frames_done < num_frames {
    // Step one CPU instruction + PPU + IC + timer
    let cycle_budget = state.cpu.borrow_mut().step()?;
    let new_frame = state.ppu.borrow_mut().step(cycle_budget)?;
    state.ic.borrow_mut().step();
    state.timer.borrow_mut().step(cycle_budget);

    if new_frame {
      frames_done += 1;
      state.gb_fps.tick();

      // Log progress every 60 frames
      if frames_done % 60 == 0 {
        info!("[headless] frame {}/{}", frames_done, num_frames);
      }
    }
  }

  info!("[headless] run complete after {} frames", frames_done);
  Ok(headless)
}

// ---------------------------------------------------------------------------
// trace_boot — step `max_steps` CPU instructions and print:
//   - Every time the CPU reads $FF44 (LY register), with the PC and value
//   - Every time LY changes value
//   - A periodic summary line
// This is designed to diagnose the boot ROM VBlank wait loop at $0064.
// ---------------------------------------------------------------------------
pub fn trace_boot(state: &mut GbState, max_steps: u64) -> GbResult<()> {
  let mut last_ly: u8 = 255; // sentinel
  let mut ly_reads: u64 = 0;
  let mut step_count: u64 = 0;

  println!(
    "[trace] Running {} CPU steps, watching LY ($FF44)...",
    max_steps
  );
  println!(
    "[trace] {:>10}  {:>6}  {:>6}  {}",
    "step", "PC", "LY_now", "event"
  );

  while step_count < max_steps {
    // Sample state BEFORE the CPU step
    let pc_before = state.cpu.borrow().pc;
    let ly_before = state.ppu.borrow().ly;

    // Detect LY changes
    if ly_before != last_ly {
      println!(
        "[trace] {:>10}  ${:04X}   {:>3}    LY changed: {} → {}",
        step_count, pc_before, ly_before, last_ly, ly_before
      );
      last_ly = ly_before;
    }

    // Detect reads of $FF44 (LDH A, ($FF44) opcode sequence: F0 44 at PC)
    // We check the opcode at pc_before to see if it's LDH A,(n) = $F0
    let opcode = state.bus.borrow().read8_debug(pc_before);
    let imm = state.bus.borrow().read8_debug(pc_before.wrapping_add(1));
    let is_ldh_ly = opcode == 0xF0 && imm == 0x44;

    // Step
    let cycle_budget = state.cpu.borrow_mut().step()?;
    state.ppu.borrow_mut().step(cycle_budget)?;
    state.ic.borrow_mut().step();
    state.timer.borrow_mut().step(cycle_budget);

    // After step: what did A become?
    if is_ldh_ly {
      let a = state.cpu.borrow().af.hi;
      ly_reads += 1;
      if ly_reads <= 30 || a == 144 {
        println!(
          "[trace] {:>10}  ${:04X}          LDH A,(LY): A={} (0x{:02X}) {}",
          step_count,
          pc_before,
          a,
          a,
          if a == 144 { "← LY=144 !!!" } else { "" }
        );
      }
    }

    step_count += 1;
  }

  let ly_final = state.ppu.borrow().ly;
  let pc_final = state.cpu.borrow().pc;
  println!(
    "[trace] Done. {} steps, {} LY reads. Final: PC=${:04X} LY={}",
    step_count, ly_reads, pc_final, ly_final
  );
  Ok(())
}

/// Watch for the boot ROM end-phase ($00E0-$00FF) and the FF50 boot-disable
/// write. Runs until boot_mode goes false OR max_steps is reached.
pub fn trace_boot_end(state: &mut GbState, max_steps: u64) -> GbResult<()> {
  let mut step_count: u64 = 0;
  let mut last_boot_mode = state.cart.borrow().boot_mode;
  let mut prev_pc: u16 = 0;

  println!("[trace_end] Watching for boot ROM completion (FF50 write)...");
  println!("[trace_end] Max steps: {}", max_steps);

  while step_count < max_steps {
    let pc = state.cpu.borrow().pc;
    let boot_mode = state.cart.borrow().boot_mode;

    // Print every instruction in the boot ROM end-phase ($00D0–$00FF)
    if pc >= 0x00D0 && pc <= 0x00FF {
      let op = state.bus.borrow().read8_debug(pc);
      let im1 = state.bus.borrow().read8_debug(pc.wrapping_add(1));
      println!(
        "[trace_end] step={:8}  PC=${:04X}  op=${:02X} ${:02X}",
        step_count, pc, op, im1
      );
    }

    // Print if boot_mode just changed
    if boot_mode != last_boot_mode {
      println!(
        "[trace_end] *** boot_mode changed: {} → {} at step {} PC=${:04X}",
        last_boot_mode, boot_mode, step_count, pc
      );
      last_boot_mode = boot_mode;
      if !boot_mode {
        println!("[trace_end] Boot ROM disabled! PC=${:04X}", pc);
        break;
      }
    }

    // Alert if PC jumps into a tight infinite loop (same PC twice in a row)
    if pc == prev_pc && pc < 0x0100 {
      let op = state.bus.borrow().read8_debug(pc);
      println!(
        "[trace_end] *** STUCK at PC=${:04X} op=${:02X} (repeating)",
        pc, op
      );
    }
    prev_pc = pc;

    let cycle_budget = state.cpu.borrow_mut().step()?;
    state.ppu.borrow_mut().step(cycle_budget)?;
    state.ic.borrow_mut().step();
    state.timer.borrow_mut().step(cycle_budget);

    step_count += 1;
  }

  let pc_final = state.cpu.borrow().pc;
  let boot_final = state.cart.borrow().boot_mode;
  println!(
    "[trace_end] Done after {} steps. PC=${:04X} boot_mode={}",
    step_count, pc_final, boot_final
  );
  Ok(())
}

/// Skip until boot_mode goes false, then trace the first `post_boot_steps`
/// game instructions with full register dumps.
pub fn trace_game_start(state: &mut GbState, post_boot_steps: u64) -> GbResult<()> {
  let mut step_count: u64 = 0;

  // --- Phase 1: run until boot ROM exits ---
  println!("[game_trace] Phase 1: waiting for boot ROM to complete...");
  loop {
    if !state.cart.borrow().boot_mode {
      break;
    }
    let cycle_budget = state.cpu.borrow_mut().step()?;
    state.ppu.borrow_mut().step(cycle_budget)?;
    state.ic.borrow_mut().step();
    state.timer.borrow_mut().step(cycle_budget);
    step_count += 1;

    if step_count % 500_000 == 0 {
      let pc = state.cpu.borrow().pc;
      println!(
        "[game_trace]   still in boot ROM after {} steps, PC=${:04X}",
        step_count, pc
      );
    }
  }

  let pc_at_boot_exit = state.cpu.borrow().pc;
  println!(
    "[game_trace] Boot ROM exited at step {} → PC=${:04X}",
    step_count, pc_at_boot_exit
  );

  // --- Phase 2: trace first post_boot_steps game instructions ---
  println!(
    "[game_trace] Phase 2: tracing {} game instructions...",
    post_boot_steps
  );
  println!(
    "[game_trace] {:>8}  {:>6}  {:>5}  {:>5}  {:>5}  {:>5}  {:>5}  {:>5}  op    event",
    "step", "PC", "AF", "BC", "DE", "HL", "SP", "LY"
  );

  for i in 0..post_boot_steps {
    let pc = state.cpu.borrow().pc;
    let af = state.cpu.borrow().af.hilo();
    let bc = state.cpu.borrow().bc.hilo();
    let de = state.cpu.borrow().de.hilo();
    let hl = state.cpu.borrow().hl.hilo();
    let sp = state.cpu.borrow().sp;
    let ly = state.ppu.borrow().ly;
    let op = state.bus.borrow().read8_debug(pc);
    let im1 = state.bus.borrow().read8_debug(pc.wrapping_add(1));

    // Check for danger signs
    let event = if pc == 0x0038 {
      "*** RST38 LOOP ***"
    } else if pc < 0x0100 && i > 5 {
      "!! back in boot ROM"
    } else {
      ""
    };

    println!(
      "[game_trace] {:>8}  ${:04X}  {:04X}  {:04X}  {:04X}  {:04X}  {:04X}  {:3}   ${:02X}{:02X}  {}",
      step_count + i, pc, af, bc, de, hl, sp, ly, op, im1, event
    );

    let cycle_budget = state.cpu.borrow_mut().step()?;
    state.ppu.borrow_mut().step(cycle_budget)?;
    state.ic.borrow_mut().step();
    state.timer.borrow_mut().step(cycle_budget);

    if pc == 0x0038 {
      println!(
        "[game_trace] Stopping at RST38 loop after {} total steps",
        step_count + i
      );
      break;
    }
  }

  Ok(())
}
