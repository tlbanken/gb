//! Helper object for video rendering and drawing

use wgpu::util::DeviceExt;
use winit::window::Window;

use crate::{
  geometry::{Color, Pos, Resolution, Vertex},
  screen::Screen,
};

const CLEAR_COLOR: wgpu::Color = wgpu::Color {
  r: 0.1,
  g: 0.2,
  b: 0.3,
  a: 1.0,
};

// Demo vertices
const VERTICES: &[Vertex] = &[
  Vertex {
    pos: Pos { x: 0, y: 0 },
    col: Color::new(1.0, 0.0, 0.0),
  },
  Vertex {
    pos: Pos { x: 0, y: 300 },
    col: Color::new(0.0, 1.0, 0.0),
  },
  Vertex {
    pos: Pos { x: 400, y: 0 },
    col: Color::new(0.0, 0.0, 1.0),
  },
];

pub struct Video {
  screen: Screen,
  surface: wgpu::Surface,
  device: wgpu::Device,
  queue: wgpu::Queue,
  config: wgpu::SurfaceConfiguration,
  size: Resolution,
  render_pipeline: wgpu::RenderPipeline,
  vertex_buffer: wgpu::Buffer,
  num_vertices: u32,
  resolution_buffer: wgpu::Buffer,
  resolution_bind_group: wgpu::BindGroup,
  // The window must be declared after the surface so
  // it gets dropped after it as the surface contains
  // unsafe references to the window's resources.
  window: Window,
}

impl Video {
  pub async fn new(window: Window) -> Self {
    let size = Resolution {
      width: window.inner_size().width,
      height: window.inner_size().height,
    };

    // init the gb screen
    let screen = Screen::new(size);

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

    // TODO: this may not be needed if we aren't writing shader code
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
      usage: wgpu::BufferUsages::UNIFORM,
    });

    let resolution_bind_group_layout =
      device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        entries: &[wgpu::BindGroupLayoutEntry {
          binding: 0,
          visibility: wgpu::ShaderStages::VERTEX,
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
      bind_group_layouts: &[&resolution_bind_group_layout],
      push_constant_ranges: &[],
    });

    // create render pipeline
    let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
      label: Some("Render Pipeline"),
      layout: Some(&render_pipeline_layout),
      vertex: wgpu::VertexState {
        module: &shader,
        entry_point: "vs_main",
        buffers: &[Vertex::desc()],
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

    // create our vertex buffer from our screen pixels
    let vertices = screen.vertices();
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
      label: Some("Vertex Buffer"),
      contents: bytemuck::cast_slice(vertices),
      usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
    });
    let num_vertices = vertices.len() as u32;

    Self {
      screen,
      window,
      surface,
      device,
      queue,
      config,
      size,
      render_pipeline,
      vertex_buffer,
      num_vertices,
      resolution_buffer,
      resolution_bind_group,
    }
  }

  pub fn window(&self) -> &Window {
    &self.window
  }

  pub fn set_pixel(&mut self, pos: Pos, col: Color) {
    self.screen.set_pixel(pos, col);
  }

  pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
    // update our vertex buffer with latest screen state
    self.queue.write_buffer(
      &self.vertex_buffer,
      0,
      bytemuck::cast_slice(self.screen.vertices()),
    );

    // first grab a frame to render
    let output = self.surface.get_current_texture()?;
    let view = output
      .texture
      .create_view(&wgpu::TextureViewDescriptor::default());
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
        label: Some("Render Pass"),
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
      render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
      render_pass.draw(0..self.num_vertices, 0..1);
    }

    // draw to screen
    self.queue.submit(std::iter::once(encoder.finish()));
    output.present();

    Ok(())
  }
}
