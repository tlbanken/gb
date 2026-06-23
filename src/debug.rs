//! Debug inspection tools for the GB emulator.
//!
//! Provides human-readable dumps of:
//!   - CPU registers
//!   - PPU registers (LCDC, STAT, LY, SCX, SCY, BGP, WX, WY)
//!   - VRAM tile sheet as a PPM image (all 384 tiles laid out in a grid)
//!   - VRAM tile map (the 32x32 index grid the PPU uses)
//!   - OAM entries
//!   - MBC1 mapper state (current ROM/RAM bank)

use crate::state::GbState;
use std::io::{self, Write};

// ---------------------------------------------------------------------------
// CPU state dump
// ---------------------------------------------------------------------------

pub fn dump_cpu(state: &GbState) {
  let cpu = state.cpu.borrow();
  println!("=== CPU Registers ===");
  println!("  AF: {:04X}  (A={:02X} F={:02X})", cpu.af.hilo(), cpu.af.hi, cpu.af.lo);
  println!("  BC: {:04X}  (B={:02X} C={:02X})", cpu.bc.hilo(), cpu.bc.hi, cpu.bc.lo);
  println!("  DE: {:04X}  (D={:02X} E={:02X})", cpu.de.hilo(), cpu.de.hi, cpu.de.lo);
  println!("  HL: {:04X}  (H={:02X} L={:02X})", cpu.hl.hilo(), cpu.hl.hi, cpu.hl.lo);
  println!("  SP: {:04X}", cpu.sp);
  println!("  PC: {:04X}", cpu.pc);
  println!("  IME: {}  HALTED: {}", cpu.ime, cpu.halted);
  let f = cpu.af.lo;
  println!(
    "  Flags: Z={} N={} H={} C={}",
    (f >> 7) & 1, (f >> 6) & 1, (f >> 5) & 1, (f >> 4) & 1
  );
  println!("  Recent PC history: {:?}", cpu.history.entries());
}

// ---------------------------------------------------------------------------
// PPU register dump
// ---------------------------------------------------------------------------

pub fn dump_ppu_regs(state: &GbState) {
  let ppu = state.ppu.borrow();
  let lcdc: u8 = ppu.lcdc.into();
  println!("=== PPU Registers ===");
  println!("  LCDC: {:08b} ({:02X})", lcdc, lcdc);
  println!("    bit7 LCD enable:          {}", ppu.lcdc.ppu_enabled);
  println!("    bit6 Win tile map hi:      {}", ppu.lcdc.win_tile_map_hi);
  println!("    bit5 Win enable:           {}", ppu.lcdc.win_enabled);
  println!("    bit4 BG/Win data map lo:   {}", ppu.lcdc.win_and_bg_data_map_lo);
  println!("    bit3 BG tile map hi:       {}", ppu.lcdc.bg_tile_map_hi);
  println!("    bit2 OBJ size large:       {}", ppu.lcdc.obj_size_large);
  println!("    bit1 OBJ enable:           {}", ppu.lcdc.obj_enabled);
  println!("    bit0 BG+Win enable:        {}", ppu.lcdc.bg_win_enable);
  println!("  LY:  {:3}  LYC: {:3}", ppu.ly, ppu.lyc);
  println!("  SCX: {:3}  SCY: {:3}", ppu.scx, ppu.scy);
  println!("  WX:  {:3}  WY:  {:3}", ppu.wx, ppu.wy);
  println!("  BGP: {:08b} ({:02X})", ppu.bgp, ppu.bgp);
  println!("  OBP0:{:08b}  OBP1:{:08b}", ppu.obp[0], ppu.obp[1]);
  println!(
    "  Tile data base: {}",
    if ppu.lcdc.win_and_bg_data_map_lo { "$8000 (unsigned)" } else { "$8800 (signed, base $9000)" }
  );
  println!(
    "  BG tile map:    {}",
    if ppu.lcdc.bg_tile_map_hi { "$9C00" } else { "$9800" }
  );
}

// ---------------------------------------------------------------------------
// VRAM tile map dump — prints the 32x32 tile index grid as hex
// ---------------------------------------------------------------------------

pub fn dump_tile_map(state: &GbState) {
  let ppu = state.ppu.borrow();
  // Tile map 0 lives at VRAM offset 0x1800 (= $9800 - $8000)
  // Tile map 1 lives at VRAM offset 0x1C00 (= $9C00 - $8000)
  for (map_name, map_offset) in [("$9800 (lo)", 0x1800usize), ("$9C00 (hi)", 0x1C00usize)] {
    println!("=== Tile Map {} ===", map_name);
    for row in 0..32usize {
      print!("  row {:2}: ", row);
      for col in 0..32usize {
        let idx = ppu.vram[map_offset + row * 32 + col];
        print!("{:02X} ", idx);
      }
      println!();
    }
  }
}

// ---------------------------------------------------------------------------
// VRAM raw hex dump — first N bytes
// ---------------------------------------------------------------------------

pub fn dump_vram_hex(state: &GbState, num_bytes: usize) {
  let ppu = state.ppu.borrow();
  let count = num_bytes.min(ppu.vram.len());
  println!("=== VRAM hex dump (first {} bytes) ===", count);
  for (i, chunk) in ppu.vram[..count].chunks(16).enumerate() {
    print!("  {:04X}: ", i * 16);
    for b in chunk {
      print!("{:02X} ", b);
    }
    println!();
  }
}

// ---------------------------------------------------------------------------
// VRAM tile sheet dump — renders all tiles to a PPM image.
//
// Layout: 16 tiles per row, 24 rows = 384 tiles (the full 8 KB VRAM).
// Each tile is 8×8 pixels; image is 128×192 px.
// ---------------------------------------------------------------------------

pub fn dump_vram_ppm(state: &GbState, path: &str) -> io::Result<()> {
  let ppu = state.ppu.borrow();

  const TILES_PER_ROW: usize = 16;
  const TOTAL_TILES: usize = 384; // 8192 bytes / 16 bytes per tile
  const TILE_ROWS: usize = TOTAL_TILES / TILES_PER_ROW; // 24
  const IMG_W: usize = TILES_PER_ROW * 8;
  const IMG_H: usize = TILE_ROWS * 8;

  // Grayscale palette: color index 0=white … 3=black
  let palette: [(u8, u8, u8); 4] = [
    (255, 255, 255),
    (170, 170, 170),
    (85, 85, 85),
    (0, 0, 0),
  ];

  let mut img = vec![(255u8, 255u8, 255u8); IMG_W * IMG_H];

  for tile_idx in 0..TOTAL_TILES {
    let tile_base = tile_idx * 16; // 16 bytes per tile
    let tile_col = tile_idx % TILES_PER_ROW;
    let tile_row = tile_idx / TILES_PER_ROW;

    for py in 0..8usize {
      let lo = ppu.vram[tile_base + py * 2];
      let hi = ppu.vram[tile_base + py * 2 + 1];
      for px in 0..8usize {
        let bit = 7 - px;
        let color_idx = ((lo >> bit) & 1) | (((hi >> bit) & 1) << 1);
        let img_x = tile_col * 8 + px;
        let img_y = tile_row * 8 + py;
        img[img_y * IMG_W + img_x] = palette[color_idx as usize];
      }
    }
  }

  let mut f = std::fs::File::create(path)?;
  writeln!(f, "P6")?;
  writeln!(f, "{} {}", IMG_W, IMG_H)?;
  writeln!(f, "255")?;
  for (r, g, b) in &img {
    f.write_all(&[*r, *g, *b])?;
  }

  println!("[debug] VRAM tile sheet written to: {}", path);
  Ok(())
}

// ---------------------------------------------------------------------------
// OAM dump
// ---------------------------------------------------------------------------

pub fn dump_oam(state: &GbState) {
  let ppu = state.ppu.borrow();
  println!("=== OAM ({} entries, max 40) ===", ppu.oam.len() / 4);
  let mut any = false;
  for i in 0..40usize {
    let base = i * 4;
    let y = ppu.oam[base];
    let x = ppu.oam[base + 1];
    let tile = ppu.oam[base + 2];
    let flags = ppu.oam[base + 3];
    if y != 0 || x != 0 {
      println!(
        "  OBJ {:2}: y={:3} x={:3} tile={:3} flags={:08b}",
        i, y, x, tile, flags
      );
      any = true;
    }
  }
  if !any {
    println!("  (all OAM entries are zero / hidden)");
  }
}

// ---------------------------------------------------------------------------
// Cart / MBC state dump
// ---------------------------------------------------------------------------

pub fn dump_cart(state: &GbState) {
  let cart = state.cart.borrow();
  println!("=== Cart ===");
  println!("  Loaded:    {}", cart.loaded);
  println!("  Boot mode: {}", cart.boot_mode);
  println!("  Path:      {:?}", cart.cart_path());
  println!("  Header:    {:?}", cart.header);
}

// ---------------------------------------------------------------------------
// Full state dump — convenience wrapper
// ---------------------------------------------------------------------------

pub fn dump_all(state: &GbState, vram_ppm_path: Option<&str>) {
  dump_cpu(state);
  println!();
  dump_ppu_regs(state);
  println!();
  dump_cart(state);
  println!();
  dump_oam(state);
  println!();
  dump_vram_hex(state, 256); // first 256 bytes = first 16 tiles
  println!();
  dump_tile_map(state);

  if let Some(path) = vram_ppm_path {
    println!();
    if let Err(e) = dump_vram_ppm(state, path) {
      eprintln!("[debug] Failed to write VRAM PPM: {}", e);
    }
  }
}
