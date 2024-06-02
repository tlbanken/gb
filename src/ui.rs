//! Debug ui for the emulator

use egui::{
  self, epaint::Shadow, Align2, Color32, Context, FullOutput, RawInput, RichText, Style, Visuals,
};
use egui_winit::winit::event_loop::EventLoopProxy;
use rfd::FileDialog;
use std::path::PathBuf;

use crate::bus::Bus;
use crate::cart::Cartridge;
use crate::dasm::Dasm;
use crate::ppu::{self, ObjectAttribute, Ppu, OAM_SIZE};
use crate::timer::Timer;
use crate::util::LazyDref;
use crate::{cpu, cpu::Cpu, event::UserEvent, state::GbState};

pub struct UiState {
  pub show_menu_bar: bool,
  pub show_cpu_reg_window: bool,
  pub show_cpu_dasm_window: bool,
  pub show_mem_window: bool,
  pub show_stat_window: bool,
  pub show_ppu_reg_window: bool,
  pub show_ppu_palette_window: bool,
  pub show_ppu_oam_window: bool,
  pub show_timer_window: bool,
  pub show_cart_info_window: bool,
  pub show_joypad_window: bool,
}

impl UiState {
  pub fn new() -> UiState {
    UiState {
      show_menu_bar: true,
      show_cpu_reg_window: false,
      show_cpu_dasm_window: false,
      show_mem_window: false,
      show_stat_window: false,
      show_ppu_reg_window: false,
      show_ppu_palette_window: false,
      show_ppu_oam_window: false,
      show_timer_window: false,
      show_cart_info_window: false,
      show_joypad_window: false,
    }
  }

  pub fn hide_all(&mut self) {
    *self = UiState::new();
  }
}

pub struct Ui {
  context: Context,
  event_loop_proxy: EventLoopProxy<UserEvent>,
}

impl Ui {
  pub fn new(event_loop_proxy: EventLoopProxy<UserEvent>) -> Self {
    let mut context = Context::default();

    // remove shadows
    Self::set_default_style(&context);

    Self {
      context,
      event_loop_proxy,
    }
  }

  pub fn context(&self) -> &Context {
    &self.context
  }

  pub fn prepare(
    &mut self,
    raw_input: RawInput,
    ui_state: &mut UiState,
    gb_state: &mut GbState,
    fps: f32,
  ) -> FullOutput {
    self.context.run(raw_input, |ctx| {
      self.ui(ctx, ui_state, gb_state, fps);
    })
  }

  fn ui(&self, ctx: &Context, ui_state: &mut UiState, gb_state: &mut GbState, fps: f32) {
    // ui layout
    if ui_state.show_menu_bar {
      egui::TopBottomPanel::top(egui::Id::new("top panel")).show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
          // resolutions
          self.ui_reso(ui);
          // menu for debug views
          ui.menu_button("Debug Views", |ui| {
            ui.menu_button("CPU", |ui| {
              // registers
              if ui.button("Registers").clicked() {
                ui_state.show_cpu_reg_window = !ui_state.show_cpu_reg_window;
                ui.close_menu();
              }
              // disassembly
              if ui.button("Disassembly").clicked() {
                ui_state.show_cpu_dasm_window = !ui_state.show_cpu_dasm_window;
                ui.close_menu();
              }
            });
            ui.menu_button("PPU", |ui| {
              // registers
              if ui.button("Registers").clicked() {
                ui_state.show_ppu_reg_window = !ui_state.show_ppu_reg_window;
                ui.close_menu();
              }
              if ui.button("Palettes").clicked() {
                ui_state.show_ppu_palette_window = !ui_state.show_ppu_palette_window;
                ui.close_menu();
              }
              if ui.button("OAM").clicked() {
                ui_state.show_ppu_oam_window = !ui_state.show_ppu_oam_window;
                ui.close_menu();
              }
            });
            if ui.button("Memory").clicked() {
              ui_state.show_mem_window = !ui_state.show_mem_window;
              ui.close_menu();
            }
            if ui.button("Timer").clicked() {
              ui_state.show_timer_window = !ui_state.show_timer_window;
              ui.close_menu();
            }
            if ui.button("Cartridge Info").clicked() {
              ui_state.show_cart_info_window = !ui_state.show_cart_info_window;
              ui.close_menu();
            }
            if ui.button("Joypad").clicked() {
              ui_state.show_joypad_window = !ui_state.show_joypad_window;
              ui.close_menu();
            }
          });

          if ui.button("Load Cartridge").clicked() {
            let start_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            let file_option = FileDialog::new().set_directory(start_dir).pick_file();
            if let Some(file) = file_option {
              // reset to load the cartridge
              self
                .event_loop_proxy
                .send_event(UserEvent::EmuReset(Some(file)))
                .unwrap();
            }
          }

          // control flow buttons
          ui.monospace("  |  ");
          if gb_state.flow.paused && ui.button("Play").clicked() {
            self
              .event_loop_proxy
              .send_event(UserEvent::EmuPlay)
              .unwrap();
          }
          if gb_state.flow.paused && ui.button("Step").clicked() {
            self
              .event_loop_proxy
              .send_event(UserEvent::EmuStep)
              .unwrap();
          }
          if !gb_state.flow.paused && ui.button("Pause").clicked() {
            self
              .event_loop_proxy
              .send_event(UserEvent::EmuPause)
              .unwrap();
          }
          if ui.button("Reset").clicked() {
            self
              .event_loop_proxy
              .send_event(UserEvent::EmuReset(gb_state.cart.borrow().cart_path()))
              .unwrap();
          }
          ui.menu_button("Speed", |ui| {
            if ui.button(".01%").clicked() {
              gb_state.flow.speed = 0.0001;
              ui.close_menu();
            }
            if ui.button("1%").clicked() {
              gb_state.flow.speed = 0.01;
              ui.close_menu();
            }
            if ui.button("25%").clicked() {
              gb_state.flow.speed = 0.25;
              ui.close_menu();
            }
            if ui.button("50%").clicked() {
              gb_state.flow.speed = 0.50;
              ui.close_menu();
            }
            if ui.button("75%").clicked() {
              gb_state.flow.speed = 0.75;
              ui.close_menu();
            }
            if ui.button("100%").clicked() {
              gb_state.flow.speed = 1.00;
              ui.close_menu();
            }
            if ui.button("200%").clicked() {
              gb_state.flow.speed = 2.00;
              ui.close_menu();
            }
            if ui.button("400%").clicked() {
              gb_state.flow.speed = 4.00;
              ui.close_menu();
            }
            if ui.button("800%").clicked() {
              gb_state.flow.speed = 8.00;
              ui.close_menu();
            }
          });
          ui.monospace("  |  ");

          // stats
          ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Stats").clicked() {
              ui_state.show_stat_window = !ui_state.show_stat_window;
            }
            // hide menu bar
            if ui.button("Hide All").clicked() {
              ui_state.hide_all();
            }
          });
        });
      });
    }

    // show debug windows
    if ui_state.show_cpu_reg_window {
      self.ui_cpu_reg(ctx, &mut gb_state.cpu.borrow_mut());
    }
    if ui_state.show_cpu_dasm_window {
      self.ui_cpu_dasm(ctx, &gb_state.cpu.borrow());
    }
    if ui_state.show_mem_window {
      self.ui_mem(ctx, &mut gb_state.bus.borrow_mut());
    }
    if ui_state.show_stat_window {
      self.ui_stat(ctx, fps, gb_state);
    }
    if ui_state.show_ppu_reg_window {
      self.ui_ppu_reg(ctx, &mut gb_state.ppu.borrow_mut());
    }
    if ui_state.show_ppu_palette_window {
      self.ui_ppu_palettes(ctx, &mut gb_state.ppu.borrow_mut());
    }
    if ui_state.show_ppu_oam_window {
      self.ui_ppu_oam(ctx, &mut gb_state.ppu.borrow_mut());
    }
    if ui_state.show_timer_window {
      self.ui_timer(ctx, &mut gb_state.timer.borrow_mut());
    }
    if ui_state.show_cart_info_window {
      self.ui_cart_info(ctx, &mut gb_state.cart.borrow_mut());
    }
    if ui_state.show_joypad_window {
      self.ui_joypad(ctx, gb_state);
    }
  }

  fn ui_stat(&self, ctx: &Context, fps: f32, gb_state: &mut GbState) {
    ctx.style_mut(|style| {
      style.visuals.window_fill = Color32::BLACK.gamma_multiply(0.50);
      style.visuals.window_stroke = egui::Stroke::new(0.0, Color32::TRANSPARENT);
    });
    egui::Window::new("Stats")
      .resizable(false)
      .anchor(Align2::RIGHT_TOP, [0.0, 0.0])
      .title_bar(false)
      .show(ctx, |ui| {
        ui.visuals_mut().override_text_color = Some(Color32::YELLOW);
        let clock_rate_mhz = gb_state.clock_rate / 1_000_000.0;
        let percent = (clock_rate_mhz / cpu::CLOCK_RATE_MHZ) * 100.0;
        ui.monospace(format!(
          "Clock Speed: {:01.04} MHz ({:3.0}%)",
          clock_rate_mhz, percent
        ));
        ui.monospace(format!("UI FPS: {:.0}", fps));
        ui.monospace(format!("GB FPS: {:.0}", gb_state.gb_fps.tps()));
      });

    // reset style
    Self::set_default_style(ctx);
  }

  fn ui_joypad(&self, ctx: &Context, gb_state: &mut GbState) {
    egui::Window::new("Joypad").show(ctx, |ui| {
      ui.monospace(format!(
        "Buttons: {:02x}, {}",
        gb_state.joypad.borrow().buttons_state,
        gb_state.joypad.borrow().button_mode
      ));
      ui.monospace(format!(
        "DPad: {:02x}, {}",
        gb_state.joypad.borrow().dpad_state,
        gb_state.joypad.borrow().dpad_mode
      ));
    });
  }

  fn ui_cart_info(&self, ctx: &Context, cart: &mut Cartridge) {
    egui::Window::new("Cartridge Info")
      .resizable(false)
      .show(ctx, |ui| {
        ui.monospace(format!("Loaded: {}", cart.loaded));
        ui.monospace("--- Header ---");
        ui.monospace(format!("Title: {}", cart.header.title));
        ui.monospace(format!(
          "Manufacturing Code: {}",
          cart.header.manufacturing_code
        ));
        ui.monospace(format!("GBC Support: {:?}", cart.header.gbc_support));
        ui.monospace(format!("Publisher: {}", cart.header.publisher));
        ui.monospace(format!("Mapper: {:?}", cart.header.mapper));
        ui.monospace(format!("Battery Present: {}", cart.header.battery_present));
        ui.monospace(format!("Ram Present: {}", cart.header.ram_present));
        ui.monospace(format!("Num ROM Banks: {}", cart.header.rom_banks));
        ui.monospace(format!("Num RAM Banks: {}", cart.header.ram_banks));
        ui.monospace(format!("ROM Version: {}", cart.header.rom_version));
        ui.monospace(format!(
          "Header Checksum: 0x{:02X}",
          cart.header.header_checksum
        ));
        ui.monospace(format!(
          "Global Checksum: 0x{:04X}",
          cart.header.global_checksum
        ));
        // TODO
      });
  }

  fn ui_cpu_reg(&self, ctx: &Context, cpu: &mut Cpu) {
    egui::Window::new("CPU Registers")
      .resizable(false)
      .show(ctx, |ui| {
        ui.monospace(format!("[PC] {:04x}", cpu.pc));
        ui.monospace(format!("[SP] {:04x}", cpu.sp));
        ui.monospace("");
        ui.monospace(format!("[A]  {:02x}  [F] {:02x}", cpu.af.hi, cpu.af.lo));
        ui.monospace(format!("[B]  {:02x}  [C] {:02x}", cpu.bc.hi, cpu.bc.lo));
        ui.monospace(format!("[D]  {:02x}  [D] {:02x}", cpu.de.hi, cpu.de.lo));
        ui.monospace(format!("[H]  {:02x}  [L] {:02x}", cpu.hl.hi, cpu.hl.lo));
        ui.monospace("");
        let f = cpu.af.lo;
        let z = if f & crate::cpu::FLAG_Z > 0 { 1 } else { 0 };
        let n = if f & crate::cpu::FLAG_N > 0 { 1 } else { 0 };
        let h = if f & crate::cpu::FLAG_H > 0 { 1 } else { 0 };
        let c = if f & crate::cpu::FLAG_C > 0 { 1 } else { 0 };
        ui.monospace(format!("Z:{}  N:{}  H:{}  C:{}", z, n, h, c));
      });
  }

  fn ui_cpu_dasm(&self, ctx: &Context, cpu: &Cpu) {
    egui::Window::new("Disassembly")
      .resizable(false)
      .show(ctx, |ui| {
        let mut vpc = cpu.pc;
        let mut dasm = Dasm::new();

        // first print history
        for _ in 0..(cpu.history.cap() - cpu.history.len()) {
          // empty line
          ui.monospace("");
        }
        for pc in cpu.history.entries() {
          let output = self.build_dasm_line(cpu, &mut pc.clone(), &mut dasm);
          ui.monospace(RichText::from(output).color(Color32::DARK_GRAY));
        }

        // print current instruction
        let output = self.build_dasm_line(cpu, &mut vpc, &mut dasm);
        ui.monospace(RichText::from(output).color(Color32::LIGHT_YELLOW));

        for i in 0..cpu.history.cap() {
          let output = self.build_dasm_line(cpu, &mut vpc, &mut dasm);
          ui.monospace(RichText::from(output).color(Color32::DARK_GRAY));
        }
      });
  }

  fn build_dasm_line(&self, cpu: &Cpu, vpc: &mut u16, dasm: &mut Dasm) -> String {
    let mut raw_bytes = Vec::<u8>::new();
    let mut output = format!(" PC:{:04X}  ", *vpc);
    loop {
      let byte = cpu.bus.lazy_dref().read8(*vpc).unwrap();
      raw_bytes.push(byte);
      *vpc += 1;
      if let Some(instr) = dasm.munch(byte) {
        let mut raw_bytes_str = String::new();
        for b in raw_bytes {
          raw_bytes_str.push_str(format!("{:02X} ", b).as_str());
        }
        output.push_str(format!("{:9} ", raw_bytes_str).as_str());
        output.push_str(format!("{:12} ", instr).as_str());
        break output;
      }
    }
  }

  fn ui_ppu_palettes(&self, ctx: &Context, ppu: &mut Ppu) {
    egui::Window::new("Palettes").show(ctx, |ui| {
      if ui.button("GRAY").clicked() {
        ppu.palette = ppu::PALETTE_GRAY;
      }
      if ui.button("GREEN").clicked() {
        ppu.palette = ppu::PALETTE_GREEN;
      }
      if ui.button("BLUE").clicked() {
        ppu.palette = ppu::PALETTE_BLUE;
      }
    });
  }

  fn ui_ppu_oam(&self, ctx: &Context, ppu: &mut Ppu) {
    egui::Window::new("OAM").resizable(true).show(ctx, |ui| {
      ui.monospace(format!("Cached Objects: {}", ppu.oam_cache.len()));
      ui.monospace("---------------");
      egui::ScrollArea::vertical().show(ui, |ui| {
        for offset in (0..OAM_SIZE).step_by(4) {
          ui.monospace(format!("Object #{}", offset / 4));
          ui.monospace("---------------");
          let obj_bytes = [
            ppu.oam[offset + 0],
            ppu.oam[offset + 1],
            ppu.oam[offset + 2],
            ppu.oam[offset + 3],
          ];
          let attr = ObjectAttribute::from(obj_bytes);
          ui.monospace(format!("Y Pos: {}", attr.y_pos));
          ui.monospace(format!("X Pos: {}", attr.x_pos));
          ui.monospace(format!("Tile IDX: {}", attr.tile_idx));
          ui.monospace(format!("Low Priority: {}", attr.flags.low_priority));
          ui.monospace(format!("Flip Y: {}", attr.flags.flip_y));
          ui.monospace(format!("Flip X: {}", attr.flags.flip_x));
          ui.monospace(format!("Palette Idx: {}", attr.flags.palette_idx));
          ui.monospace("---------------");
        }
      });
    });
  }

  fn ui_ppu_reg(&self, ctx: &Context, ppu: &mut Ppu) {
    egui::Window::new("PPU Registers").show(ctx, |ui| {
      ui.monospace(format!("LY: {}", ppu.ly));
      ui.monospace(format!("SCX: {}", ppu.scx));
      ui.monospace(format!("SCY: {}", ppu.scy));
      ui.monospace(format!("LCDC.BG_WIN_PRIORITY: {}", ppu.lcdc.bg_win_enable));
      ui.monospace(format!("LCDC.OBJ_ENABLE: {}", ppu.lcdc.obj_enabled));
      ui.monospace(format!("LCDC.LARGE_OBJ_SIZE: {}", ppu.lcdc.obj_size_large));
      ui.monospace(format!("LCDC.BG_TILE_HI: {}", ppu.lcdc.bg_tile_map_hi));
      ui.monospace(format!(
        "LCDC.BG_WIN_TILE_LO: {}",
        ppu.lcdc.win_and_bg_data_map_lo
      ));
      ui.monospace(format!("LCDC.WIN_ENABLE: {}", ppu.lcdc.win_enabled));
      ui.monospace(format!(
        "LCDC.WIN_TILE_MAP_HI: {}",
        ppu.lcdc.win_tile_map_hi
      ));
      ui.monospace(format!("LCDC.LCD_ENABLE: {}", ppu.lcdc.ppu_enabled));
    });
  }

  fn ui_mem(&self, ctx: &Context, bus: &mut Bus) {
    egui::Window::new("Memory Dump")
      .resizable(true)
      .show(ctx, |ui| {
        // set up starting state
        let num_cols = 8;
        let total_mem_size = 0x1_0000;

        let text_style = egui::TextStyle::Monospace;
        let row_height = ui.text_style_height(&text_style);
        let num_rows = total_mem_size / num_cols;
        egui::ScrollArea::both().auto_shrink(false).show_rows(
          ui,
          row_height,
          num_rows,
          |ui, row_range| {
            ui.style_mut().wrap = Some(false);
            // memory dump
            for row in row_range {
              let row_addr = row * num_cols;
              let mut row_str = String::from(format!("{:04X}  ", row_addr));
              let mut as_char_str = String::from(" | ");
              for col in 0..num_cols {
                let addr = row_addr + col;
                let byte = bus.read8(addr as u16).unwrap();
                row_str.push_str(format!("{:02X} ", byte).as_str());
                let c = if (33..126).contains(&byte) {
                  byte as char
                } else {
                  '.'
                };
                as_char_str.push(c);
              }
              as_char_str.push_str(" |");
              row_str.push_str(as_char_str.as_str());
              ui.monospace(row_str);
            }
          },
        );
      });
  }

  fn ui_timer(&self, ctx: &Context, timer: &mut Timer) {
    egui::Window::new("Timer Registers").show(ctx, |ui| {
      ui.monospace(format!("DIV: 0x{:02X}", timer.div));
      ui.monospace(format!("TIMA: 0x{:02X}", timer.tima));
      ui.monospace(format!("TMA: 0x{:02X}", timer.tma));
      ui.monospace(format!("TAC: 0x{:02X}", u8::from(timer.tac)));
    });
  }

  fn ui_reso(&self, ui: &mut egui::Ui) {
    ui.menu_button("Screen Size", |ui| {
      if ui.button("160 x 144 (x1)").clicked() {
        self
          .event_loop_proxy
          .send_event(UserEvent::RequestResize(160, 144))
          .unwrap();
        ui.close_menu();
      }
      if ui.button("480 x 432 (x3)").clicked() {
        self
          .event_loop_proxy
          .send_event(UserEvent::RequestResize(480, 432))
          .unwrap();
        ui.close_menu();
      }
      if ui.button("800 x 720 (x5)").clicked() {
        self
          .event_loop_proxy
          .send_event(UserEvent::RequestResize(800, 720))
          .unwrap();
        ui.close_menu();
      }
      if ui.button("1280 x 1152 (x8)").clicked() {
        self
          .event_loop_proxy
          .send_event(UserEvent::RequestResize(1280, 1152))
          .unwrap();
        ui.close_menu();
      }
      if ui.button("1600 x 1440 (x10)").clicked() {
        self
          .event_loop_proxy
          .send_event(UserEvent::RequestResize(1600, 1440))
          .unwrap();
        ui.close_menu();
      }
      if ui.button("2400 x 2160 (x15)").clicked() {
        self
          .event_loop_proxy
          .send_event(UserEvent::RequestResize(2400, 2160))
          .unwrap();
        ui.close_menu();
      }
    });
  }

  fn set_default_style(ctx: &Context) {
    ctx.set_style(Style {
      visuals: Visuals {
        window_shadow: Shadow::NONE,
        panel_fill: egui::Color32::BLACK.gamma_multiply(0.85),
        window_fill: egui::Color32::BLACK.gamma_multiply(0.95),
        ..Default::default()
      },
      ..Default::default()
    });
  }
}
