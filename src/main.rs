//! Gameboy Emulator entry point

extern crate core;

mod bus;
mod cart;
mod cpu;
mod dasm;
mod debug;
mod err;
mod event;
mod gb;
mod headless;
mod int;
mod joypad;
mod logger;
mod ppu;
mod ram;
mod screen;
mod state;
mod tick_counter;
mod timer;
mod ui;
mod util;
mod video;

use log::LevelFilter;
use state::{EmuFlow, GbState};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// CLI args
// ---------------------------------------------------------------------------

struct Args {
  headless: bool,
  rom: Option<PathBuf>,
  /// Number of GB frames to run in headless mode (default 120 = ~2 seconds)
  frames: u32,
  /// If true, run trace_boot instead of normal headless loop
  trace: bool,
  /// If true, run trace_boot_end watching for FF50 boot-disable
  trace_end: bool,
  /// If true, run trace_game_start after boot ROM completes
  trace_game: bool,
  /// Number of CPU steps for --trace (default 100_000)
  trace_steps: u64,
  /// If Some, write a VRAM tile-sheet PPM to this path after the run
  dump_vram: Option<String>,
  /// If true, print CPU + PPU register state after the run
  dump_state: bool,
}

impl Args {
  fn parse() -> Self {
    let mut args = std::env::args().skip(1); // skip binary name
    let mut out = Args {
      headless: false,
      rom: None,
      frames: 120,
      dump_vram: None,
      dump_state: false,
      trace: false,
      trace_end: false,
      trace_game: false,
      trace_steps: 100_000,
    };

    while let Some(arg) = args.next() {
      match arg.as_str() {
        "--headless" => out.headless = true,
        "--dump-state" => out.dump_state = true,
        "--rom" => {
          out.rom = args.next().map(PathBuf::from);
        }
        "--frames" => {
          if let Some(n) = args.next() {
            out.frames = n.parse().expect("--frames requires an integer");
          }
        }
        "--dump-vram" => {
          out.dump_vram = args.next();
        }
        "--trace" => {
          out.trace = true;
        }
        "--trace-end" => {
          out.trace_end = true;
        }
        "--trace-game" => {
          out.trace_game = true;
        }
        "--trace-steps" => {
          if let Some(n) = args.next() {
            out.trace_steps = n.parse().expect("--trace-steps requires an integer");
          }
        }
        other => {
          eprintln!("Unknown argument: {}", other);
          eprintln!("Usage: gb [--headless] [--rom <path>] [--frames <N>] [--dump-vram <out.ppm>] [--dump-state] [--trace] [--trace-end] [--trace-game] [--trace-steps <N>]");
          std::process::exit(1);
        }
      }
    }
    out
  }
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn main() {
  println!("~~~ Enter the Gameboy Emulation ~~~");

  let args = Args::parse();

  // set the max through compile time config in Cargo.toml
  let log_level_filter = LevelFilter::Info;

  if args.headless {
    run_headless_mode(args, log_level_filter);
  } else {
    run_gui_mode(args.rom, log_level_filter);
  }
}

// ---------------------------------------------------------------------------
// Headless mode
// ---------------------------------------------------------------------------

fn run_headless_mode(args: Args, log_level_filter: LevelFilter) {
  gb::init_logging(log_level_filter);

  let rom_path = match args.rom {
    Some(p) => p,
    None => {
      eprintln!("--headless requires --rom <path>");
      std::process::exit(1);
    }
  };

  println!("[headless] ROM:    {:?}", rom_path);
  println!("[headless] Frames: {}", args.frames);

  let mut state = GbState::new(EmuFlow::new(false, false, 1.0));
  state.init_headless(rom_path).expect("Failed to init headless state");

  if args.trace {
    println!("[headless] TRACE MODE: {} CPU steps", args.trace_steps);
    headless::trace_boot(&mut state, args.trace_steps).expect("Trace failed");
  } else if args.trace_end {
    println!("[headless] TRACE-END MODE: {} CPU steps", args.trace_steps);
    headless::trace_boot_end(&mut state, args.trace_steps).expect("Trace-end failed");
  } else if args.trace_game {
    println!("[headless] TRACE-GAME MODE: {} post-boot steps", args.trace_steps);
    headless::trace_game_start(&mut state, args.trace_steps).expect("Trace-game failed");
  } else {
    headless::run_headless(&mut state, args.frames).expect("Headless run failed");

    // Dump requested diagnostics
    if args.dump_state {
      println!();
      debug::dump_all(&state, args.dump_vram.as_deref());
    } else if let Some(ref vram_path) = args.dump_vram {
      debug::dump_vram_ppm(&state, vram_path).expect("Failed to write VRAM PPM");
    }
  }

  println!("[headless] done.");

}

// ---------------------------------------------------------------------------
// Normal GUI mode
// ---------------------------------------------------------------------------

fn run_gui_mode(rom_path: Option<PathBuf>, log_level_filter: LevelFilter) {
  let gameboy = gb::Gameboy::new(log_level_filter);
  gameboy.run_with_rom(rom_path).unwrap();
}
