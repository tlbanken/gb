//! Debug ui for the emulator

use egui::{self, epaint::Shadow, Context, FullOutput, RawInput, Style, Visuals};
use egui_winit::winit::event_loop::EventLoopProxy;

use crate::event::UserEvent;

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
        ..Default::default()
      },
      ..Default::default()
    });

    // TODO: Set up fonts?

    Self {
      context,
      event_loop_proxy,
    }
  }

  pub fn context(&self) -> &Context {
    &self.context
  }

  pub fn prepare(&mut self, raw_input: RawInput) -> FullOutput {
    self.context.run(raw_input, |ctx| {
      self.ui(ctx);
    })
  }

  fn ui(&self, ctx: &Context) {
    // TODO: layout the ui
    egui::Window::new("Test Window")
      .resizable(true)
      .show(ctx, |ui| {
        ui.label("I am a label");
      });
  }
}
