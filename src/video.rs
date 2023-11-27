//! Helper object for video rendering and drawing

use egui;
use egui_wgpu::renderer::ScreenDescriptor;
use egui_wgpu::wgpu::util::DeviceExt;
use egui_wgpu::wgpu::TextureView;
use egui_wgpu::{wgpu, WgpuConfiguration};
use egui_winit::winit;
use egui_winit::winit::event::WindowEvent;
use egui_winit::winit::window::Window;

use crate::fps::Fps;
use crate::screen::{Color, Pos, Resolution, Screen};
use crate::state::GbState;
use crate::ui::{Ui, UiState};

const CLEAR_COLOR: wgpu::Color = wgpu::Color {
  r: 0.0,
  g: 0.0,
  b: 0.0,
  a: 1.0,
};

pub struct Video {
  screen: Screen,
  surface: wgpu::Surface,
  device: wgpu::Device,
  queue: wgpu::Queue,
  config: wgpu::SurfaceConfiguration,
  size: Resolution,
  render_pipeline: wgpu::RenderPipeline,
  resolution_buffer: wgpu::Buffer,
  resolution_bind_group: wgpu::BindGroup,
  egui_renderer: egui_wgpu::Renderer,
  ui: Ui,
  egui_state: egui_winit::State,
  ui_state: UiState,
  fps: Fps,
  // The window must be declared after the surface so
  // it gets dropped after it as the surface contains
  // unsafe references to the window's resources.
  window: Window,
}

impl Video {
  pub async fn new(window: Window, ui: Ui) -> Self {
    let size = Resolution {
      width: window.inner_size().width,
      height: window.inner_size().height,
    };

    // the instance gives us a way to create handle to gpu and create surfaces
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
      backends: wgpu::Backends::all(),
      ..Default::default()
    });

    // Create surface. The surface needs to live as long as the window for this
    // to be safe.
    let surface = unsafe { instance.create_surface(&window) }.unwrap();

    // get handle to gpu
    let adapter = instance
      .request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: Some(&surface),
        force_fallback_adapter: false,
      })
      .await
      .unwrap();

    // create device and queue
    let (device, queue) = adapter
      .request_device(
        &wgpu::DeviceDescriptor {
          features: wgpu::Features::empty(),
          limits: wgpu::Limits::default(),
          label: None,
        },
        None,
      )
      .await
      .unwrap();

    // init the gb screen
    let screen = Screen::new(&device);

    // configure surface
    let surface_caps = surface.get_capabilities(&adapter);
    // configure for srgb display
    let surface_format = surface_caps
      .formats
      .iter()
      .copied()
      .find(|f| f.is_srgb())
      .unwrap_or(surface_caps.formats[0]);
    let config = wgpu::SurfaceConfiguration {
      usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
      format: surface_format,
      width: size.width,
      height: size.height,
      present_mode: surface_caps.present_modes[0],
      alpha_mode: surface_caps.alpha_modes[0],
      view_formats: vec![],
    };
    surface.configure(&device, &config);

    // load shaders
    let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

    // send our screen resolution to the shaders as well
    let resolution_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
      label: Some("Uniform Buffer"),
      contents: bytemuck::cast_slice(&[size]),
      usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });

    let resolution_bind_group_layout =
      device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        entries: &[wgpu::BindGroupLayoutEntry {
          binding: 0,
          visibility: wgpu::ShaderStages::FRAGMENT,
          ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
          },
          count: None,
        }],
        label: Some("resolution_bind_group_layout"),
      });

    let resolution_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("resolution_bind_group"),
      layout: &resolution_bind_group_layout,
      entries: &[wgpu::BindGroupEntry {
        binding: 0,
        resource: resolution_buffer.as_entire_binding(),
      }],
    });

    // create pipeline layout
    let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
      label: Some("Render Pipeline Layout"),
      bind_group_layouts: &[&resolution_bind_group_layout, screen.group_layout()],
      push_constant_ranges: &[],
    });

    // create render pipeline
    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("Render Pipeline"),
      layout: Some(&render_pipeline_layout),
      vertex: wgpu::VertexState {
        module: &shader,
        entry_point: "vs_main",
        buffers: &[],
      },
      fragment: Some(wgpu::FragmentState {
        module: &shader,
        entry_point: "fs_main",
        targets: &[Some(wgpu::ColorTargetState {
          format: config.format,
          blend: Some(wgpu::BlendState::REPLACE),
          write_mask: wgpu::ColorWrites::ALL,
        })],
      }),
      primitive: wgpu::PrimitiveState {
        topology: wgpu::PrimitiveTopology::TriangleList,
        strip_index_format: None,
        front_face: wgpu::FrontFace::Ccw,
        // no need for culling since we are in 2d
        cull_mode: None,
        polygon_mode: wgpu::PolygonMode::Fill,
        unclipped_depth: false,
        conservative: false,
      },
      depth_stencil: None,
      multisample: wgpu::MultisampleState {
        count: 1,
        mask: !0,
        alpha_to_coverage_enabled: false,
      },
      multiview: None,
    });

    // set up egui
    let egui_state = egui_winit::State::new(
      ui.context().viewport_id(),
      &window,
      ui.context().native_pixels_per_point(),
      None,
    );
    let egui_renderer = egui_wgpu::Renderer::new(&device, config.format, None, 1);
    let ui_state = UiState::new();

    let fps = Fps::new();

    Self {
      screen,
      window,
      surface,
      device,
      queue,
      config,
      size,
      render_pipeline,
      resolution_buffer,
      resolution_bind_group,
      egui_renderer,
      ui,
      ui_state,
      egui_state,
      fps,
    }
  }

  pub fn window(&self) -> &Window {
    &self.window
  }

  pub fn handle_window_event(&mut self, event: WindowEvent) -> bool {
    let gb_repaint = match event {
      WindowEvent::Resized(size) => {
        self.resize(size);
        true
      }
      _ => false,
    };
    let ui_repaint = self
      .egui_state
      .on_window_event(self.ui.context(), &event)
      .repaint;

    // repaint if either requests it
    gb_repaint || ui_repaint
  }

  pub fn set_pixel(&mut self, pos: Pos, col: Color) {
    self.screen.set_pixel(pos, col);
  }

  pub fn render(&mut self, gb_state: &mut GbState) -> Result<(), wgpu::SurfaceError> {
    self.fps.tick();

    // update screen colors from its buffer state
    self.screen.write_buffer(&mut self.queue);

    // first grab a frame to render
    let output = self.surface.get_current_texture()?;
    let view = output
      .texture
      .create_view(&wgpu::TextureViewDescriptor::default());

    // first render gameboy data
    self.render_gameboy(&view);

    // now render egui
    self.render_ui(&view, gb_state, self.fps.fps());

    // finally, draw to the screen
    output.present();
    Ok(())
  }

  fn render_gameboy(&mut self, view: &TextureView) {
    // build encoder for sending commands to the gpu
    let mut encoder = self
      .device
      .create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("Render Encoder"),
      });

    // create scope to drop the render pass. Avoids ownership issues with mut
    // borrowing on encoder
    {
      // create the render pass
      let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Main Render Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
          view: &view,
          resolve_target: None,
          ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(CLEAR_COLOR),
            store: wgpu::StoreOp::Store,
          },
        })],
        depth_stencil_attachment: None,
        ..Default::default()
      });

      render_pass.set_pipeline(&self.render_pipeline);
      render_pass.set_bind_group(0, &self.resolution_bind_group, &[]);
      render_pass.set_bind_group(1, &self.screen.bind_group(), &[]);
      render_pass.draw(0..6, 0..1);
    }

    // submit render requests to queue
    self.queue.submit(std::iter::once(encoder.finish()));
  }

  fn render_ui(&mut self, view: &TextureView, gb_state: &mut GbState, fps: u32) {
    let raw_input = self.egui_state.take_egui_input(&self.window);
    let full_output = self
      .ui
      .prepare(raw_input, &mut self.ui_state, gb_state, fps);
    for (id, delta) in &full_output.textures_delta.set {
      self
        .egui_renderer
        .update_texture(&self.device, &self.queue, *id, &delta);
    }
    self.egui_state.handle_platform_output(
      &self.window,
      self.ui.context(),
      full_output.platform_output,
    );
    let clipped_prims = &self
      .ui
      .context()
      .tessellate(full_output.shapes, self.ui.context().pixels_per_point());
    let screen_descriptor = ScreenDescriptor {
      size_in_pixels: [self.size.width as u32, self.size.height as u32],
      pixels_per_point: self.window.scale_factor() as f32,
    };
    let mut encoder = self
      .device
      .create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("UI Encoder"),
      });
    // ui render pass
    {
      self.egui_renderer.update_buffers(
        &self.device,
        &self.queue,
        &mut encoder,
        &clipped_prims,
        &screen_descriptor,
      );

      let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Egui Render Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
          view,
          resolve_target: None,
          ops: wgpu::Operations {
            load: wgpu::LoadOp::Load,
            store: wgpu::StoreOp::Store,
          },
        })],
        depth_stencil_attachment: None,
        ..Default::default()
      });
      self
        .egui_renderer
        .render(&mut render_pass, &clipped_prims, &screen_descriptor);
    }
    self.queue.submit(std::iter::once(encoder.finish()));
  }

  fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
    if new_size.width > 0 && new_size.height > 0 {
      self.size = Resolution {
        width: new_size.width,
        height: new_size.height,
      };
      self.config.width = new_size.width;
      self.config.height = new_size.height;
      self.surface.configure(&self.device, &self.config);

      // update gpu shader variables
      self.queue.write_buffer(
        &self.resolution_buffer,
        0,
        bytemuck::cast_slice(&[self.size]),
      );
    }
  }
}
