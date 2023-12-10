//! Screen for the gameboy emulator

use egui_wgpu::wgpu;
use egui_wgpu::wgpu::util::DeviceExt;

const GB_RESOLUTION: Resolution = Resolution {
  width: 160,
  height: 144,
};

const NUM_PIXELS: usize = (GB_RESOLUTION.width * GB_RESOLUTION.height) as usize;

const PIXEL_CLEAR: Color = Color {
  r: 0.1,
  g: 0.1,
  b: 0.2,
  a: 1.0,
};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Resolution {
  pub width: u32,
  pub height: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Pos {
  pub x: u32,
  pub y: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Color {
  pub r: f32,
  pub g: f32,
  pub b: f32,
  // keep this for alignment
  pub a: f32,
}

impl Color {
  pub const fn new(r: f32, g: f32, b: f32) -> Self {
    Self { r, g, b, a: 1.0 }
  }
}

pub struct Screen {
  pixels: Vec<Color>,
  pixels_bind_group: wgpu::BindGroup,
  pixels_bind_group_layout: wgpu::BindGroupLayout,
  pixels_buffer: wgpu::Buffer,
}

impl Screen {
  pub fn new(device: &wgpu::Device) -> Self {
    // set up initial pixels
    let mut pixels = Vec::new();
    for _ in 0..GB_RESOLUTION.height {
      for _ in 0..GB_RESOLUTION.width {
        pixels.push(PIXEL_CLEAR);
      }
    }

    // set up storage buffer to pass screen colors to gpu
    let pixels_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
      label: Some("Pixels Storage Buffer"),
      contents: bytemuck::cast_slice(&pixels.as_slice()),
      usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    });

    // set up uniform buffer to pass gameboy screen resolution to gpu
    let screen_res_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
      label: Some("Screen Resolution Uniform Buffer"),
      contents: bytemuck::cast_slice(&[GB_RESOLUTION]),
      usage: wgpu::BufferUsages::UNIFORM,
    });

    let pixels_bind_group_layout =
      device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        entries: &[
          wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
              ty: wgpu::BufferBindingType::Storage { read_only: true },
              has_dynamic_offset: false,
              min_binding_size: None,
            },
            count: None,
          },
          wgpu::BindGroupLayoutEntry {
            binding: 1,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
              ty: wgpu::BufferBindingType::Uniform,
              has_dynamic_offset: false,
              min_binding_size: None,
            },
            count: None,
          },
        ],
        label: Some("pixels_bind_group_layout"),
      });

    let pixels_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
      label: Some("pixels_bind_group"),
      layout: &pixels_bind_group_layout,
      entries: &[
        wgpu::BindGroupEntry {
          binding: 0,
          resource: pixels_buffer.as_entire_binding(),
        },
        wgpu::BindGroupEntry {
          binding: 1,
          resource: screen_res_buffer.as_entire_binding(),
        },
      ],
    });

    Self {
      pixels,
      pixels_bind_group,
      pixels_bind_group_layout,
      pixels_buffer,
    }
  }

  pub fn group_layout(&self) -> &wgpu::BindGroupLayout {
    &self.pixels_bind_group_layout
  }

  pub fn bind_group(&mut self) -> &wgpu::BindGroup {
    &self.pixels_bind_group
  }

  pub fn write_buffer(&mut self, queue: &mut wgpu::Queue) {
    queue.write_buffer(
      &self.pixels_buffer,
      0,
      bytemuck::cast_slice(self.pixels.as_slice()),
    );
  }

  pub fn set_pixel(&mut self, pos: Pos, col: Color) {
    assert!(pos.x < GB_RESOLUTION.width);
    assert!(pos.y < GB_RESOLUTION.height);
    self.pixels[(pos.y * GB_RESOLUTION.width + pos.x) as usize] = col;
  }
}
