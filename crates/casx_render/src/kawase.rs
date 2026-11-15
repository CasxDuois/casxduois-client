//! Shader va bo loc hau ky Dual-Filter Kawase Blur cho Lunar Overlay va HoYoPlay Hub.

pub const KAWASE_BLUR_WGSL: &str = r#"
@group(0) @binding(0)
var input_texture: texture_2d<f32>;
@group(0) @binding(1)
var texture_sampler: sampler;

struct KawaseUniform {
    offset: vec2<f32>,
    resolution: vec2<f32>,
};

@group(1) @binding(0)
var<uniform> params: KawaseUniform;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) in_vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32(i32(in_vertex_index) / 2) * 4.0 - 1.0;
    let y = f32(i32(in_vertex_index) % 2) * 4.0 - 1.0;
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

@fragment
fn fs_kawase_downsample(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = 1.0 / params.resolution;
    let half_pixel = texel * 0.5;

    // Lay mau 5 diem: tam va 4 goc chech nua pixel
    var col = textureSample(input_texture, texture_sampler, in.uv) * 4.0;
    col += textureSample(input_texture, texture_sampler, in.uv - half_pixel * params.offset);
    col += textureSample(input_texture, texture_sampler, in.uv + half_pixel * params.offset);
    col += textureSample(input_texture, texture_sampler, in.uv + vec2<f32>(half_pixel.x, -half_pixel.y) * params.offset);
    col += textureSample(input_texture, texture_sampler, in.uv + vec2<f32>(-half_pixel.x, half_pixel.y) * params.offset);

    return col * (1.0 / 8.0);
}

@fragment
fn fs_kawase_upsample(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = 1.0 / params.resolution;
    let offset = texel * params.offset;

    // Lay mau 8 diem hinh chu thap va goc
    var col = textureSample(input_texture, texture_sampler, in.uv + vec2<f32>(-offset.x * 2.0, 0.0));
    col += textureSample(input_texture, texture_sampler, in.uv + vec2<f32>(-offset.x, offset.y)) * 2.0;
    col += textureSample(input_texture, texture_sampler, in.uv + vec2<f32>(0.0, offset.y * 2.0));
    col += textureSample(input_texture, texture_sampler, in.uv + vec2<f32>(offset.x, offset.y)) * 2.0;
    col += textureSample(input_texture, texture_sampler, in.uv + vec2<f32>(offset.x * 2.0, 0.0));
    col += textureSample(input_texture, texture_sampler, in.uv + vec2<f32>(offset.x, -offset.y)) * 2.0;
    col += textureSample(input_texture, texture_sampler, in.uv + vec2<f32>(0.0, -offset.y * 2.0));
    col += textureSample(input_texture, texture_sampler, in.uv + vec2<f32>(-offset.x, -offset.y)) * 2.0;

    return col * (1.0 / 12.0);
}
"#;
