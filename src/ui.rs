//! Debug ui for the emulator

use egui::{
  self, epaint::Shadow, Color32, Context, FullOutput, RawInput, RichText, Style, Visuals,
};
use egui_winit::winit::event_loop::EventLoopProxy;
use std::collections::VecDeque;

use crate::dasm::Dasm;
use crate::util::LazyDref;
use crate::{cpu::Cpu, event::UserEvent, state::GbState};

pub struct UiState {
  pub show_menu_bar: bool,
  pub show_cpu_reg_window: bool,
  pub show_cpu_dasm_window: bool,
  pub show_wram_window: bool,
  pub show_eram_window: bool,
}

impl UiState {
  pub fn new() -> UiState {
    UiState {
      show_menu_bar: true,
      show_cpu_reg_window: false,
      show_cpu_dasm_window: false,
      show_eram_window: false,
      show_wram_window: false,
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
    let context = Context::default();

    // remove shadows
    context.set_style(Style {
      visuals: Visuals {
        window_shadow: Shadow::NONE,
        panel_fill: egui::Color32::BLACK.gamma_multiply(0.85),
        window_fill: egui::Color32::BLACK.gamma_multiply(0.95),
        ..Default::default()
      },
      ..Default::default()
    });

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
    fps: u32,
  ) -> FullOutput {
    self.context.run(raw_input, |ctx| {
      self.ui(ctx, ui_state, gb_state, fps);
    })
  }

  fn ui(&self, ctx: &Context, ui_state: &mut UiState, gb_state: &mut GbState, fps: u32) {
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
            if ui.button("ERAM").clicked() {
              ui_state.show_eram_window = !ui_state.show_eram_window;
              ui.close_menu();
            }
            if ui.button("WRAM").clicked() {
              ui_state.show_wram_window = !ui_state.show_wram_window;
              ui.close_menu();
            }
          });

          if ui.button("Load Cartridge").clicked() {
            todo!("load cartridge")
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
          ui.monospace("  |  ");

          // fps
          ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.monospace(format!("| {:4} fps", fps));
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
    if ui_state.show_eram_window {
      self.ui_eram(ctx);
    }
    if ui_state.show_wram_window {
      self.ui_wram(ctx);
    }
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

  fn ui_eram(&self, ctx: &Context) {
    egui::Window::new("ERAM Dump").show(ctx, |ui| {
      // TODO
      ui.monospace("I am a ERAM");
    });
  }

  fn ui_wram(&self, ctx: &Context) {
    egui::Window::new("WRAM Dump").show(ctx, |ui| {
      // TODO
      ui.monospace("I am a WRAM");
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
}
