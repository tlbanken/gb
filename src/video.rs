//! Helper object for video rendering and drawing

use egui_wgpu::wgpu;
use egui_wgpu::wgpu::util::DeviceExt;
use egui_wgpu::wgpu::TextureView;
use egui_wgpu::ScreenDescriptor;
use egui_winit::winit;
use egui_winit::winit::event::WindowEvent;
use egui_winit::winit::window::Window;
use std::cell::RefCell;
use std::rc::Rc;

use crate::screen::{Resolution, Screen};
use crate::state::GbState;
use crate::tick_counter::TickCounter;
use crate::ui::{Ui, UiState};

const FPS_ALPHA: f32 = 0.9;

const CLEAR_COLOR: wgpu::Color = wgpu::Color {
  r: 0.0,
  g: 0.0,
  b: 0.0,
  a: 1.0,
};

pub struct Video {
  screen: Rc<RefCell<Screen>>,
  surface: wgpu::Surface<'static>,
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
  fps: TickCounter,
  // Wrap the window in std::sync::Arc to share ownership with the wgpu Surface.
  // This allows the Surface to have a 'static lifetime, bypassing self-referential
  // lifetime errors between the window and surface inside the Video struct.
  // The window must be declared after the surface so it gets dropped after it
  // as the surface contains references to the window's resources.
  window: std::sync::Arc<Window>,
}

impl Video {
  pub async fn new(window: std::sync::Arc<Window>, ui: Ui) -> Self {
    let size = Resolution {
      width: window.inner_size().width,
      height: window.inner_size().height,
    };

    // the instance gives us a way to create handle to gpu and create surfaces
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
      backends: wgpu::Backends::all(),
      ..Default::default()
    });

    // Create surface.
    let surface = instance.create_surface(window.clone()).unwrap();

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
      .request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        label: None,
        ..Default::default()
      })
      .await
      .unwrap();

    // init the gb screen
    let screen = Rc::new(RefCell::new(Screen::new(&device)));

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
      desired_maximum_frame_latency: 2,
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
      bind_group_layouts: &[
        &resolution_bind_group_layout,
        screen.borrow().group_layout(),
      ],
      push_constant_ranges: &[],
    });

    // create render pipeline
    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("Render Pipeline"),
      layout: Some(&render_pipeline_layout),
      vertex: wgpu::VertexState {
        module: &shader,
        entry_point: Some("vs_main"),
        buffers: &[],
        compilation_options: Default::default(),
      },
      fragment: Some(wgpu::FragmentState {
        module: &shader,
        entry_point: Some("fs_main"),
        targets: &[Some(wgpu::ColorTargetState {
          format: config.format,
          blend: Some(wgpu::BlendState::REPLACE),
          write_mask: wgpu::ColorWrites::ALL,
        })],
        compilation_options: Default::default(),
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
      cache: None,
    });

    // set up egui
    let egui_state = egui_winit::State::new(
      ui.context().clone(),
      ui.context().viewport_id(),
      &window,
      Some(window.scale_factor() as f32),
      None,
      None,
    );
    let egui_renderer = egui_wgpu::Renderer::new(
      &device,
      config.format,
      egui_wgpu::RendererOptions {
        msaa_samples: 1,
        depth_stencil_format: None,
        ..Default::default()
      },
    );
    let ui_state = UiState::new();

    let fps = TickCounter::new(FPS_ALPHA);

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

  pub fn screen(&self) -> Rc<RefCell<Screen>> {
    self.screen.clone()
  }

  pub fn handle_window_event(&mut self, event: &WindowEvent) -> bool {
    let gb_repaint = match event {
      WindowEvent::Resized(size) => {
        self.resize(*size);
        true
      }
      _ => false,
    };
    let ui_repaint = self.egui_state.on_window_event(&self.window, event).repaint;

    // repaint if either requests it
    gb_repaint || ui_repaint
  }

  pub fn render(&mut self, gb_state: &mut GbState) -> Result<(), wgpu::SurfaceError> {
    self.fps.tick();

    // update screen colors from its buffer state
    self.screen.borrow_mut().write_buffer(&mut self.queue);

    // first grab a frame to render
    let output = self.surface.get_current_texture()?;
    let view = output
      .texture
      .create_view(&wgpu::TextureViewDescriptor::default());

    // first render gameboy data
    self.render_gameboy(&view);

    // now render egui
    let fps = self.fps.tps();
    // self.fps.lap();
    self.render_ui(&view, gb_state, fps);

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
    let mut screen = self.screen.borrow_mut();
    {
      // create the render pass
      let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Main Render Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
          view,
          resolve_target: None,
          ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(CLEAR_COLOR),
            store: wgpu::StoreOp::Store,
          },
          depth_slice: None,
        })],
        depth_stencil_attachment: None,
        ..Default::default()
      });

      render_pass.set_pipeline(&self.render_pipeline);
      render_pass.set_bind_group(0, &self.resolution_bind_group, &[]);
      render_pass.set_bind_group(1, screen.bind_group(), &[]);
      render_pass.draw(0..6, 0..1);
    }

    // submit render requests to queue
    self.queue.submit(std::iter::once(encoder.finish()));
  }

  fn render_ui(&mut self, view: &TextureView, gb_state: &mut GbState, fps: f32) {
    let raw_input = self.egui_state.take_egui_input(&self.window);
    let full_output = self
      .ui
      .prepare(raw_input, &mut self.ui_state, gb_state, fps);
    for (id, delta) in &full_output.textures_delta.set {
      self
        .egui_renderer
        .update_texture(&self.device, &self.queue, *id, delta);
    }
    self
      .egui_state
      .handle_platform_output(&self.window, full_output.platform_output);
    let clipped_prims = &self
      .ui
      .context()
      .tessellate(full_output.shapes, self.window.scale_factor() as f32);
    let screen_descriptor = ScreenDescriptor {
      size_in_pixels: [self.size.width, self.size.height],
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
        clipped_prims,
        &screen_descriptor,
      );

      let render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("Egui Render Pass"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
          view,
          resolve_target: None,
          ops: wgpu::Operations {
            load: wgpu::LoadOp::Load,
            store: wgpu::StoreOp::Store,
          },
          depth_slice: None,
        })],
        depth_stencil_attachment: None,
        ..Default::default()
      });
      // Call forget_lifetime() to detach the RenderPass from the local
      // CommandEncoder's lifetime borrow. This converts the render pass to
      // RenderPass<'static>, which is required by egui_wgpu::Renderer::render
      // to perform internal rendering operations, and permits submitting the
      // command encoder afterwards.
      let mut render_pass = render_pass.forget_lifetime();
      self
        .egui_renderer
        .render(&mut render_pass, clipped_prims, &screen_descriptor);
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
