# Gameboy Emulator
A Gameboy emulator written in Rust. This is a hobby project and is not aiming to replace any existing emulators.

## Building

```
cargo build
```

## Running

```
cargo run --release
```

### Run with ROM

```
cargo run --release -- --rom <path-to-rom>
```

## Headless Mode & Debugging

The emulator supports a headless mode with built-in tracing and state inspection tools for debugging.

### Running Headless
To run the emulator without GUI rendering (e.g. for profiling or quick debugging), use the `--headless` flag with a specified ROM:
```bash
cargo run --release -- --headless --rom <path-to-rom> [options]
```

### Options
* `--frames <N>`: Number of GB frames to emulate (default: `120`, ~2 seconds).
* `--dump-state`: Prints CPU and PPU register states to stdout after completion.
* `--dump-vram <output.ppm>`: Renders the current VRAM tile-sheet to a PPM image.

### Diagnostic Tracing
The emulator includes three targeted tracing modes to debug CPU execution:
* **Boot ROM LY Trace** (`--trace`): Watches the CPU's reads of the `LY` register ($FF44) to debug VBlank wait loops.
* **Boot Exit Trace** (`--trace-end`): Monitors the end of the boot ROM sequence and the boot disable register write ($FF50).
* **Game Start Trace** (`--trace-game`): Bypasses the boot ROM execution, then prints register-by-register instruction traces of the game's start. Stops automatically on infinite/death loops (e.g., at `$0038`).
* `--trace-steps <N>`: Number of CPU steps/instructions to trace (default: `100,000`).

#### Examples
Trace the first 500 game instructions of a ROM after boot ROM exit:
```bash
cargo run --release -- --headless --rom "roms/game.gb" --trace-game --trace-steps 500
```

Dump a VRAM tile-sheet after 240 frames of emulation:
```bash
cargo run --release -- --headless --rom "roms/game.gb" --frames 240 --dump-vram "vram_dump.ppm"
```
