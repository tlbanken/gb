//! Helper object for video rendering and drawing

use winit::window::Window;

const CLEAR_COLOR: wgpu::Color = wgpu::Color {
  r: 0.1,
  g: 0.2,
  b: 0.3,
  a: 1.0,
};

pub struct Video {
  surface: wgpu::Surface,
  device: wgpu::Device,
  queue: wgpu::Queue,
  config: wgpu::SurfaceConfiguration,
  size: winit::dpi::PhysicalSize<u32>,
  // The window must be declared after the surface so
  // it gets dropped after it as the surface contains
  // unsafe references to the window's resources.
  window: Window,
}

impl Video {
  pub async fn new(window: Window) -> Self {
    let size = window.inner_size();

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

    Self {
      window,
      surface,
      device,
      queue,
      config,
      size,
    }
  }

  pub fn window(&self) -> &Window {
    &self.window
  }

  pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
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
      let _render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
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
    }

    // draw to screen
    self.queue.submit(std::iter::once(encoder.finish()));
    output.present();

    Ok(())
  }
}
