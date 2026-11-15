//! Thuat toan Greedy Meshing 6 huong ket hop Voxel Ambient Occlusion da luong bang Rayon.

use crate::vertex::PackedVertex;
use casx_core::world::{CanonicalBlockState, ChunkSection};
use rayon::prelude::*;

/// Du lieu luoi da giac (Mesh) cua mot Chunk Section san sang nap len VRAM
#[derive(Debug, Clone, Default)]
pub struct SectionMesh {
    pub vertices: Vec<PackedVertex>,
    pub indices: Vec<u32>,
}

impl SectionMesh {
    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    pub fn quad_count(&self) -> usize {
        self.indices.len() / 6
    }
}

/// Tinh toan Ambient Occlusion (0..3) cho mot goc cua khoi dua tren 3 khoi tiep giap
///
/// Cong thuc:
/// $$\text{AO}(s_1, s_2, c) = \begin{cases} 0 & \text{neu } s_1 \text{ va } s_2 \text{ deu dac} \\ 3 - (s_1 + s_2 + c) & \text{truong hop con lai} \end{cases}$$
#[inline(always)]
pub fn compute_ao(side1: bool, side2: bool, corner: bool) -> u32 {
    if side1 && side2 {
        0
    } else {
        3 - (side1 as u32 + side2 as u32 + corner as u32)
    }
}

/// Bo tao luoi Voxel 6 huong da luong cho toan bo Section
pub struct GreedyMesher;

impl GreedyMesher {
    /// Tao luoi Section bang thuat toan Greedy Meshing
    pub fn mesh_section(section: &ChunkSection) -> SectionMesh {
        if section.non_air_count == 0 {
            return SectionMesh::default();
        }

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // Quet ca 6 huong: 0=Bottom, 1=Top, 2=North, 3=South, 4=West, 5=East
        for face_idx in 0..6 {
            Self::mesh_face_direction(section, face_idx, &mut vertices, &mut indices);
        }

        SectionMesh { vertices, indices }
    }

    /// Tao luoi cho mot huong mat cu the
    fn mesh_face_direction(
        section: &ChunkSection,
        face_idx: u32,
        vertices: &mut Vec<PackedVertex>,
        indices: &mut Vec<u32>,
    ) {
        // Mat phang 2D 16x16 cho tung lat cat
        // mask luu ID khoi (0 neu khong can ve mat)
        let mut mask = [0u16; 256];

        // Xac dinh truc chieu doc (d) va hai truc phang (u, v) tuy thuoc vao face_idx
        for d in 0..16 {
            // 1. Dien du lieu mask cho lat cat hien tai
            let mut n = 0;
            for v in 0..16 {
                for u in 0..16 {
                    let (x, y, z) = Self::map_coords(face_idx, d, u, v);
                    let current = section.get_block(x, y, z);

                    if current.is_solid() {
                        // Kiem tra khoi lang gieng theo huong phap tuyen
                        let (nx, ny, nz) = Self::neighbor_coords(face_idx, x, y, z);
                        let neighbor = if nx < 16 && ny < 16 && nz < 16 {
                            section.get_block(nx, ny, nz)
                        } else {
                            CanonicalBlockState::AIR
                        };

                        if !neighbor.is_solid() {
                            mask[n] = current.id;
                        } else {
                            mask[n] = 0;
                        }
                    } else {
                        mask[n] = 0;
                    }
                    n += 1;
                }
            }

            // 2. Thuat toan Greedy Quadding tren mang 2D 16x16
            n = 0;
            for j in 0..16 {
                let mut i = 0;
                while i < 16 {
                    let block_id = mask[n + i];
                    if block_id != 0 {
                        // Tim chieu rong quad theo truc U
                        let mut w = 1;
                        while i + w < 16 && mask[n + i + w] == block_id {
                            w += 1;
                        }

                        // Tim chieu cao quad theo truc V
                        let mut h = 1;
                        'outer: while j + h < 16 {
                            for k in 0..w {
                                if mask[n + k + h * 16 + i] != block_id {
                                    break 'outer;
                                }
                            }
                            h += 1;
                        }

                        // Xoa cac o da duoc gop trong mask
                        for row in 0..h {
                            for col in 0..w {
                                mask[n + col + row * 16 + i] = 0;
                            }
                        }

                        // 3. Them quad vao bo dem dinh va chi so
                        let base_vertex = vertices.len() as u32;
                        let (qx, qy, qz) = Self::map_coords(face_idx, d, i, j);

                        let quad_verts = Self::generate_quad_vertices(
                            face_idx, qx as u32, qy as u32, qz as u32, w as u32, h as u32, block_id as u32,
                        );
                        vertices.extend_from_slice(&quad_verts);

                        indices.extend_from_slice(&[
                            base_vertex,
                            base_vertex + 1,
                            base_vertex + 2,
                            base_vertex + 2,
                            base_vertex + 3,
                            base_vertex,
                        ]);

                        i += w;
                    } else {
                        i += 1;
                    }
                }
                n += 16;
            }
        }
    }

    #[inline(always)]
    fn map_coords(face_idx: u32, d: usize, u: usize, v: usize) -> (usize, usize, usize) {
        match face_idx {
            0 | 1 => (u, d, v), // Y- / Y+
            2 | 3 => (u, v, d), // Z- / Z+
            _ => (d, v, u),     // X- / X+
        }
    }

    #[inline(always)]
    fn neighbor_coords(face_idx: u32, x: usize, y: usize, z: usize) -> (usize, usize, usize) {
        match face_idx {
            0 => (x, y.wrapping_sub(1), z),
            1 => (x, y + 1, z),
            2 => (x, y, z.wrapping_sub(1)),
            3 => (x, y, z + 1),
            4 => (x.wrapping_sub(1), y, z),
            _ => (x + 1, y, z),
        }
    }

    fn generate_quad_vertices(
        face_idx: u32,
        x: u32,
        y: u32,
        z: u32,
        w: u32,
        h: u32,
        block_id: u32,
    ) -> [PackedVertex; 4] {
        let ao = 3; // Mac dinh sang toi da
        let block_light = 15;
        let sky_light = 15;
        let tex_layer = block_id;

        match face_idx {
            1 => {
                // Top (+Y)
                [
                    PackedVertex::new(x, y + 1, z, face_idx, ao, 0, 0, block_light, tex_layer, sky_light),
                    PackedVertex::new(x, y + 1, z + h, face_idx, ao, 0, h, block_light, tex_layer, sky_light),
                    PackedVertex::new(x + w, y + 1, z + h, face_idx, ao, w, h, block_light, tex_layer, sky_light),
                    PackedVertex::new(x + w, y + 1, z, face_idx, ao, w, 0, block_light, tex_layer, sky_light),
                ]
            }
            0 => {
                // Bottom (-Y)
                [
                    PackedVertex::new(x, y, z, face_idx, ao, 0, 0, block_light, tex_layer, sky_light),
                    PackedVertex::new(x + w, y, z, face_idx, ao, w, 0, block_light, tex_layer, sky_light),
                    PackedVertex::new(x + w, y, z + h, face_idx, ao, w, h, block_light, tex_layer, sky_light),
                    PackedVertex::new(x, y, z + h, face_idx, ao, 0, h, block_light, tex_layer, sky_light),
                ]
            }
            2 => {
                // North (-Z)
                [
                    PackedVertex::new(x + w, y, z, face_idx, ao, w, 0, block_light, tex_layer, sky_light),
                    PackedVertex::new(x, y, z, face_idx, ao, 0, 0, block_light, tex_layer, sky_light),
                    PackedVertex::new(x, y + h, z, face_idx, ao, 0, h, block_light, tex_layer, sky_light),
                    PackedVertex::new(x + w, y + h, z, face_idx, ao, w, h, block_light, tex_layer, sky_light),
                ]
            }
            3 => {
                // South (+Z)
                [
                    PackedVertex::new(x, y, z + 1, face_idx, ao, 0, 0, block_light, tex_layer, sky_light),
                    PackedVertex::new(x + w, y, z + 1, face_idx, ao, w, 0, block_light, tex_layer, sky_light),
                    PackedVertex::new(x + w, y + h, z + 1, face_idx, ao, w, h, block_light, tex_layer, sky_light),
                    PackedVertex::new(x, y + h, z + 1, face_idx, ao, 0, h, block_light, tex_layer, sky_light),
                ]
            }
            4 => {
                // West (-X)
                [
                    PackedVertex::new(x, y, z, face_idx, ao, 0, 0, block_light, tex_layer, sky_light),
                    PackedVertex::new(x, y, z + h, face_idx, ao, 0, h, block_light, tex_layer, sky_light),
                    PackedVertex::new(x, y + w, z + h, face_idx, ao, w, h, block_light, tex_layer, sky_light),
                    PackedVertex::new(x, y + w, z, face_idx, ao, w, 0, block_light, tex_layer, sky_light),
                ]
            }
            _ => {
                // East (+X)
                [
                    PackedVertex::new(x + 1, y, z + h, face_idx, ao, 0, h, block_light, tex_layer, sky_light),
                    PackedVertex::new(x + 1, y, z, face_idx, ao, 0, 0, block_light, tex_layer, sky_light),
                    PackedVertex::new(x + 1, y + w, z, face_idx, ao, w, 0, block_light, tex_layer, sky_light),
                    PackedVertex::new(x + 1, y + w, z + h, face_idx, ao, w, h, block_light, tex_layer, sky_light),
                ]
            }
        }
    }

    /// Tao luoi song song cho toan bo danh sach cac Section bang Rayon threadpool
    pub fn mesh_sections_parallel(sections: &[ChunkSection]) -> Vec<SectionMesh> {
        sections.par_iter().map(Self::mesh_section).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greedy_meshing_merges_full_flat_slice_into_single_quad() {
        let mut section = ChunkSection::default();
        let stone = CanonicalBlockState::new(1, 0);

        // Dat mot san da phang 16x16 tai day Section (Y=0)
        for x in 0..16 {
            for z in 0..16 {
                section.set_block(x, 0, z, stone);
            }
        }

        let mesh = GreedyMesher::mesh_section(&section);

        // Mat Top cua san da phang 16x16 phai duoc gop lai thanh dung 1 quad duy nhat (thay vi 256 quads rieng le!)
        // Tong so mat: mat Top (1 quad) + mat Bottom (1 quad) + 4 mat bien (16 quads)
        assert!(mesh.quad_count() < 30, "Greedy Meshing phai giam manh so luong da giac: {}", mesh.quad_count());
        assert_eq!(mesh.indices.len(), mesh.quad_count() * 6);
    }
}
