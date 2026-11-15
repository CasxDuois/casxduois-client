//! Cau truc dinh nen 64-bit (8 bytes) toi uu bang thong bo nho VRAM cho Voxel Rendering.

use bytemuck::{Pod, Zeroable};

/// Dinh nén 64-bit cho hien thi khoi Voxel ($8\text{ bytes}$ moi dinh)
///
/// data0 (32 bits):
/// - X (5 bits): $0..31$
/// - Y (5 bits): $0..31$
/// - Z (5 bits): $0..31$
/// - FaceIndex (3 bits): $0..5$ (Bottom, Top, North, South, West, East)
/// - Ambient Occlusion (2 bits): $0..3$
/// - U Size (4 bits): $1..16$
/// - V Size (4 bits): $1..16$
/// - BlockLight (4 bits): $0..15$
///
/// data1 (32 bits):
/// - TextureLayer (16 bits): $0..65535$
/// - SkyLight (4 bits): $0..15$
/// - Reserved (12 bits): $0$
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Pod, Zeroable, Default)]
pub struct PackedVertex {
    pub data0: u32,
    pub data1: u32,
}

impl PackedVertex {
    pub fn new(
        x: u32,
        y: u32,
        z: u32,
        face_index: u32,
        ao: u32,
        u_size: u32,
        v_size: u32,
        block_light: u32,
        texture_layer: u32,
        sky_light: u32,
    ) -> Self {
        let data0 = ((x & 0x1F) << 27)
            | ((y & 0x1F) << 22)
            | ((z & 0x1F) << 17)
            | ((face_index & 0x07) << 14)
            | ((ao & 0x03) << 12)
            | ((u_size & 0x0F) << 8)
            | ((v_size & 0x0F) << 4)
            | (block_light & 0x0F);

        let data1 = ((texture_layer & 0xFFFF) << 16) | ((sky_light & 0x0F) << 12);

        Self { data0, data1 }
    }

    pub fn unpack_position(&self) -> (u32, u32, u32) {
        let x = (self.data0 >> 27) & 0x1F;
        let y = (self.data0 >> 22) & 0x1F;
        let z = (self.data0 >> 17) & 0x1F;
        (x, y, z)
    }

    pub fn unpack_face_index(&self) -> u32 {
        (self.data0 >> 14) & 0x07
    }

    pub fn unpack_ao(&self) -> u32 {
        (self.data0 >> 12) & 0x03
    }

    pub fn unpack_texture_layer(&self) -> u32 {
        (self.data1 >> 16) & 0xFFFF
    }

    /// Mo ta bo dem dinh cho wgpu
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Uint32,
                },
                wgpu::VertexAttribute {
                    offset: 4,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_packed_vertex_size_is_exactly_8_bytes() {
        assert_eq!(std::mem::size_of::<PackedVertex>(), 8);
    }

    #[test]
    fn test_packed_vertex_roundtrip_attributes() {
        let v = PackedVertex::new(15, 8, 3, 2, 3, 4, 12, 10, 1024, 15);
        assert_eq!(v.unpack_position(), (15, 8, 3));
        assert_eq!(v.unpack_face_index(), 2);
        assert_eq!(v.unpack_ao(), 3);
        assert_eq!(v.unpack_texture_layer(), 1024);
    }
}
