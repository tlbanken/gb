//! Screen for the gameboy emulator

use crate::geometry::{Color, Pos, Resolution, Triangle, Vertex};

const GB_RESOLUTION: Resolution = Resolution {
  width: 160,
  height: 144,
};

const NUM_PIXELS: usize = (GB_RESOLUTION.width * GB_RESOLUTION.height) as usize;

// 2 triangles with 3 vertices each
const NUM_VERTICES_IN_PIXEL: usize = 2 * 3;

// monochrome colors
const PIXEL_CLEAR: Color = Color {
  r: 1.0,
  g: 0.0,
  b: 0.0,
};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
/// A gameboy pixel is a square, which is made up of 2 triangles.
struct Pixel {
  tris: [Triangle; 2],
}

impl Pixel {
  pub fn new(screen_pos: Pos, col: Color, sx: f32, sy: f32) -> Self {
    let scaled_pos = Pos {
      x: (screen_pos.x as f32 * sx) as u32,
      y: (screen_pos.y as f32 * sy) as u32,
    };
    // TODO: not sure if this ceil() overlap pixels when screen is not multiple of
    // gb resolution
    let sx = sx.ceil();
    let sy = sy.ceil();
    let bot_left = Pos {
      x: scaled_pos.x,
      y: scaled_pos.y,
    };
    let top_left = Pos {
      x: scaled_pos.x,
      y: scaled_pos.y + sy as u32,
    };
    let top_right = Pos {
      x: scaled_pos.x + sx as u32,
      y: scaled_pos.y + sy as u32,
    };
    let bot_right = Pos {
      x: scaled_pos.x + sx as u32,
      y: scaled_pos.y,
    };
    let tri1 = Triangle {
      vertices: [
        Vertex { pos: top_left, col },
        Vertex { pos: bot_left, col },
        Vertex {
          pos: bot_right,
          col,
        },
      ],
    };
    let tri2 = Triangle {
      vertices: [
        Vertex {
          pos: bot_right,
          col,
        },
        Vertex {
          pos: top_right,
          col,
        },
        Vertex { pos: top_left, col },
      ],
    };
    Self { tris: [tri1, tri2] }
  }

  pub fn update_color(&mut self, col: Color) {
    for tri in &mut self.tris {
      for vertex in &mut tri.vertices {
        (*vertex).col = col;
      }
    }
  }
}

pub struct Screen {
  pixels: Vec<Pixel>,
}

impl Screen {
  pub fn new(window_resolution: Resolution) -> Self {
    // let pixel_size = Pos {
    //   x: window_resolution.width / GB_RESOLUTION.width,
    //   y: window_resolution.height / GB_RESOLUTION.height,
    // };
    let sx = window_resolution.width as f32 / GB_RESOLUTION.width as f32;
    let sy = window_resolution.height as f32 / GB_RESOLUTION.height as f32;
    let mut pixels = Vec::new();
    for y in 0..GB_RESOLUTION.height {
      for x in 0..GB_RESOLUTION.width {
        pixels.push(Pixel::new(Pos { x, y }, PIXEL_CLEAR, sx, sy));
      }
    }
    Self { pixels }
  }

  /// Grab a reference to the raw vertices in the screen. This can be passed to
  /// a vertex buffer.
  pub fn vertices(&self) -> &[Vertex] {
    unsafe {
      std::slice::from_raw_parts(
        self.pixels.as_ptr() as *const Vertex,
        self.pixels.len() * NUM_VERTICES_IN_PIXEL,
      )
    }
  }

  pub fn set_pixel(&mut self, pos: Pos, col: Color) {
    assert!(pos.x < GB_RESOLUTION.width);
    assert!(pos.y < GB_RESOLUTION.height);
    self.pixels[(pos.y * GB_RESOLUTION.width + pos.x) as usize].update_color(col);
  }
}
