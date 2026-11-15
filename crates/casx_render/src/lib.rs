//! Crate `casx_render` (Tang 3C trong do thi phu thuoc DAG).
//!
//! Trach nhiem:
//! - Dinh nen 64-bit (`PackedVertex`) tiet kiem 83.3% bang thong VRAM.
//! - Thuat toan 6-Face Greedy Meshing da luong (`rayon`) kem Voxel Ambient Occlusion.
//! - Cat tam nhin 6 mat phang (`Frustum`).
//! - Shader WGSL to bong huong va hau ky Dual-Filter Kawase Blur.

pub mod vertex;
pub mod frustum;
pub mod mesher;
pub mod shader;
pub mod kawase;
pub mod indirect;
pub mod pipeline;

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
