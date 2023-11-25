// Vertex shader

struct ResolutionUniform {
  val: vec2<u32>
};
@group(0) @binding(0)
var<uniform> resolution: ResolutionUniform;

struct VertexInput {
  @location(0) position: vec2<u32>,
  @location(1) color: vec3<f32>,
};

struct VertexOutput {
  @builtin(position) clip_position: vec4<f32>,
  @location(0) color: vec3<f32>
};

@vertex
fn vs_main(model: VertexInput) -> VertexOutput {
  var out: VertexOutput;
  out.color = model.color;
  // vertices were given in screen space, convert to clip space
  let clip_x = ((f32(model.position.x) / f32(resolution.val.x)) * 2.0) - 1.0;
  let clip_y = ((f32(model.position.y) / f32(resolution.val.y)) * 2.0) - 1.0;
  out.clip_position = vec4<f32>(clip_x, clip_y, 0.0, 1.0);
  return out;
}

// Fragment Shader

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
  // output color
  return vec4<f32>(in.color, 1.0);
}