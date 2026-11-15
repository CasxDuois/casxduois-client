//! Chunk meshing, packed vertex layout, frustum culling, and wgpu rendering pipelines.
//!
//! Features:
//! - 64-bit packed vertex layout (`PackedVertex`) minimizing VRAM bandwidth.
//! - Multithreaded 6-Face Greedy Meshing (`rayon`) with Voxel Ambient Occlusion.
//! - 6-plane View-Frustum Culling (`Frustum`).
//! - WGSL shaders with directional shading and post-process Dual-Filter Kawase Blur.

pub mod frustum;
pub mod indirect;
pub mod kawase;
pub mod mesher;
pub mod pipeline;
pub mod shader;
pub mod vertex;

pub use frustum::{Frustum, Plane};
pub use indirect::{DrawIndexedIndirectArgs, IndirectBatchManager};
pub use mesher::{compute_ao, GreedyMesher, SectionMesh};
pub use pipeline::RenderPipeline5Pass;
pub use vertex::PackedVertex;

use bytemuck::{Pod, Zeroable};
use glam::Mat4;

/// Du lieu Uniform truyen vao Shader Camera
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 4],
}

impl Default for CameraUniform {
    fn default() -> Self {
        Self {
            view_proj: Mat4::IDENTITY.to_cols_array_2d(),
            camera_pos: [0.0, 0.0, 0.0, 1.0],
        }
    }
}
