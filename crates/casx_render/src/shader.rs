//! WGSL shader source for unpacking 64-bit vertices, directional shading, and Ambient Occlusion.

pub const VOXEL_SHADER_WGSL: &str = r#"
struct CameraUniform {
    view_proj: mat4x4<f32>,
    camera_pos: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

@group(1) @binding(0)
var texture_array: texture_2d_array<f32>;
@group(1) @binding(1)
var texture_sampler: sampler;

struct VertexInput {
    @location(0) data0: u32,
    @location(1) data1: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) texture_layer: u32,
    @location(2) lighting: vec3<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    // Decode XYZ position from data0 (5 bits per axis)
    let x = f32((in.data0 >> 27u) & 0x1Fu);
    let y = f32((in.data0 >> 22u) & 0x1Fu);
    let z = f32((in.data0 >> 17u) & 0x1Fu);

    let face_idx = (in.data0 >> 14u) & 0x07u;
    let ao = f32((in.data0 >> 12u) & 0x03u);
    let u_coord = f32((in.data0 >> 8u) & 0x0Fu);
    let v_coord = f32((in.data0 >> 4u) & 0x0Fu);
    let block_light = f32(in.data0 & 0x0Fu);

    let tex_layer = (in.data1 >> 16u) & 0xFFFFu;
    let sky_light = f32((in.data1 >> 12u) & 0x0Fu);

    let world_pos = vec4<f32>(x, y, z, 1.0);
    out.clip_position = camera.view_proj * world_pos;
    out.uv = vec2<f32>(u_coord, v_coord);
    out.texture_layer = tex_layer;

    // Compute face directional shading factor
    var face_shade = 0.8;
    if (face_idx == 1u) {
        face_shade = 1.0; // Top
    } else if (face_idx == 0u) {
        face_shade = 0.5; // Bottom
    } else if (face_idx == 4u || face_idx == 5u) {
        face_shade = 0.6; // East / West
    }

    // Ambient Occlusion factor: ao in [0..3] -> multiplier in [0.45..1.0]
    let ao_factor = 0.45 + (ao * 0.183);

    // Combine block and sky light
    let light_level = max(block_light / 15.0, (sky_light / 15.0) * 0.95);
    let final_intensity = face_shade * ao_factor * max(0.08, light_level);

    out.lighting = vec3<f32>(final_intensity, final_intensity, final_intensity);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(
        texture_array,
        texture_sampler,
        in.uv,
        i32(in.texture_layer)
    );

    if (tex_color.a < 0.1) {
        discard;
    }

    return vec4<f32>(tex_color.rgb * in.lighting, tex_color.a);
}
"#;
