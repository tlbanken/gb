// Vertex shader

// create a quad to cover the entire screen
var<private> VERTICES: array<vec2<f32>, 6> = array<vec2<f32>, 6>(
  // Triangle 1
  vec2<f32>(-1.0, 1.0),  // top left
  vec2<f32>(-1.0, -1.0), // bot left
  vec2<f32>(1.0, -1.0),  // bot right
  // Triangle 2
  vec2<f32>(1.0, -1.0),  // bot right
  vec2<f32>(1.0, 1.0),   // top right
  vec2<f32>(-1.0, 1.0),  // top left
);

struct VertexOutput {
  @builtin(position) pos: vec4<f32>,
};

@vertex
fn vs_main(
    @builtin(vertex_index) in_vertex_index: u32,
) -> VertexOutput {
  // draw a quad covering the whole screen
  var out: VertexOutput;
  out.pos = vec4<f32>(VERTICES[in_vertex_index], 0.0, 1.0);
  return out;
}

// Fragment Shader

struct ResolutionUniform {
  val: vec2<u32>
};
@group(0) @binding(0)
var<uniform> resolution: ResolutionUniform;

struct Pixel {
  color: vec4<f32>
}
@group(1) @binding(0)
var<storage, read> pixels: array<Pixel>;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
  // The goal of the fragment shader is to map our screen pixel coordinate 
  // to our gameboy pixel coordinate and read that pixels color. The color 
  // is provided from a storage buffer sent from the cpu.

  // TODO: pass gameboy screen resolution in as uniform buffer
  // gameboy screen is 160 x 144
  let scale_x = in.pos.x / f32(resolution.val.x);
  let scale_y = in.pos.y / f32(resolution.val.y);
  let x = u32(scale_x * 160.0);
  let y = u32(scale_y * 144.0);
  let color = pixels[(y * u32(160)) + x].color;
  return color;
}