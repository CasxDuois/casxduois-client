//! 6-face greedy meshing algorithm with voxel Ambient Occlusion multithreaded via Rayon.

use crate::vertex::PackedVertex;
use casx_core::world::{CanonicalBlockState, ChunkSection};
use rayon::prelude::*;

/// Mesh data for a single chunk section ready for VRAM upload.
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

/// Computes Ambient Occlusion (0..3) for a block corner given 3 neighbor blocks.
///
/// Formula:
/// $$\text{AO}(s_1, s_2, c) = \begin{cases} 0 & \text{if } s_1 \text{ and } s_2 \text{ are solid} \\ 3 - (s_1 + s_2 + c) & \text{otherwise} \end{cases}$$
#[inline(always)]
pub fn compute_ao(side1: bool, side2: bool, corner: bool) -> u32 {
    if side1 && side2 {
        0
    } else {
        3 - (side1 as u32 + side2 as u32 + corner as u32)
    }
}

/// 6-Face Greedy Mesher for chunk sections
pub struct GreedyMesher;

impl GreedyMesher {
    /// Generates section mesh using Greedy Meshing algorithm
    pub fn mesh_section(section: &ChunkSection) -> SectionMesh {
        if section.non_air_count == 0 {
            return SectionMesh::default();
        }

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        // Scan all 6 face directions: 0=Bottom, 1=Top, 2=North, 3=South, 4=West, 5=East
        for face_idx in 0..6 {
            Self::mesh_face_direction(section, face_idx, &mut vertices, &mut indices);
        }

        SectionMesh { vertices, indices }
    }

    /// Generates quads for a specific face direction
    fn mesh_face_direction(
        section: &ChunkSection,
        face_idx: u32,
        vertices: &mut Vec<PackedVertex>,
        indices: &mut Vec<u32>,
    ) {
        // 2D 16x16 mask per slice (stores block ID, 0 if culled)
        let mut mask = [0u16; 256];

        for d in 0..16 {
            let mut n = 0;
            for v in 0..16 {
                for u in 0..16 {
                    let (x, y, z) = Self::map_coords(face_idx, d, u, v);
                    let current = section.get_block(x, y, z);

                    if current.is_solid() {
                        // Check neighbor in normal direction
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

            // 2. Greedy quadding algorithm across 2D 16x16 slice
            n = 0;
            for j in 0..16 {
                let mut i = 0;
                while i < 16 {
                    let block_id = mask[n + i];
                    if block_id != 0 {
                        // Find quad width along U axis
                        let mut w = 1;
                        while i + w < 16 && mask[n + i + w] == block_id {
                            w += 1;
                        }

                        // Find quad height along V axis
                        let mut h = 1;
                        'outer: while j + h < 16 {
                            for k in 0..w {
                                if mask[n + k + h * 16 + i] != block_id {
                                    break 'outer;
                                }
                            }
                            h += 1;
                        }

                        // Clear merged entries in mask
                        for row in 0..h {
                            for col in 0..w {
                                mask[n + col + row * 16 + i] = 0;
                            }
                        }

                        // 3. Append quad into vertex and index buffers
                        let base_vertex = vertices.len() as u32;
                        let (qx, qy, qz) = Self::map_coords(face_idx, d, i, j);

                        let quad_verts = Self::generate_quad_vertices(
                            face_idx,
                            qx as u32,
                            qy as u32,
                            qz as u32,
                            w as u32,
                            h as u32,
                            block_id as u32,
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
                    PackedVertex::new(
                        x,
                        y + 1,
                        z,
                        face_idx,
                        ao,
                        0,
                        0,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                    PackedVertex::new(
                        x,
                        y + 1,
                        z + h,
                        face_idx,
                        ao,
                        0,
                        h,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                    PackedVertex::new(
                        x + w,
                        y + 1,
                        z + h,
                        face_idx,
                        ao,
                        w,
                        h,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                    PackedVertex::new(
                        x + w,
                        y + 1,
                        z,
                        face_idx,
                        ao,
                        w,
                        0,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                ]
            }
            0 => {
                // Bottom (-Y)
                [
                    PackedVertex::new(
                        x,
                        y,
                        z,
                        face_idx,
                        ao,
                        0,
                        0,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                    PackedVertex::new(
                        x + w,
                        y,
                        z,
                        face_idx,
                        ao,
                        w,
                        0,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                    PackedVertex::new(
                        x + w,
                        y,
                        z + h,
                        face_idx,
                        ao,
                        w,
                        h,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                    PackedVertex::new(
                        x,
                        y,
                        z + h,
                        face_idx,
                        ao,
                        0,
                        h,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                ]
            }
            2 => {
                // North (-Z)
                [
                    PackedVertex::new(
                        x + w,
                        y,
                        z,
                        face_idx,
                        ao,
                        w,
                        0,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                    PackedVertex::new(
                        x,
                        y,
                        z,
                        face_idx,
                        ao,
                        0,
                        0,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                    PackedVertex::new(
                        x,
                        y + h,
                        z,
                        face_idx,
                        ao,
                        0,
                        h,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                    PackedVertex::new(
                        x + w,
                        y + h,
                        z,
                        face_idx,
                        ao,
                        w,
                        h,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                ]
            }
            3 => {
                // South (+Z)
                [
                    PackedVertex::new(
                        x,
                        y,
                        z + 1,
                        face_idx,
                        ao,
                        0,
                        0,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                    PackedVertex::new(
                        x + w,
                        y,
                        z + 1,
                        face_idx,
                        ao,
                        w,
                        0,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                    PackedVertex::new(
                        x + w,
                        y + h,
                        z + 1,
                        face_idx,
                        ao,
                        w,
                        h,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                    PackedVertex::new(
                        x,
                        y + h,
                        z + 1,
                        face_idx,
                        ao,
                        0,
                        h,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                ]
            }
            4 => {
                // West (-X)
                [
                    PackedVertex::new(
                        x,
                        y,
                        z,
                        face_idx,
                        ao,
                        0,
                        0,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                    PackedVertex::new(
                        x,
                        y,
                        z + h,
                        face_idx,
                        ao,
                        0,
                        h,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                    PackedVertex::new(
                        x,
                        y + w,
                        z + h,
                        face_idx,
                        ao,
                        w,
                        h,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                    PackedVertex::new(
                        x,
                        y + w,
                        z,
                        face_idx,
                        ao,
                        w,
                        0,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                ]
            }
            _ => {
                // East (+X)
                [
                    PackedVertex::new(
                        x + 1,
                        y,
                        z + h,
                        face_idx,
                        ao,
                        0,
                        h,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                    PackedVertex::new(
                        x + 1,
                        y,
                        z,
                        face_idx,
                        ao,
                        0,
                        0,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                    PackedVertex::new(
                        x + 1,
                        y + w,
                        z,
                        face_idx,
                        ao,
                        w,
                        0,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                    PackedVertex::new(
                        x + 1,
                        y + w,
                        z + h,
                        face_idx,
                        ao,
                        w,
                        h,
                        block_light,
                        tex_layer,
                        sky_light,
                    ),
                ]
            }
        }
    }

    /// Meshes sections in parallel using Rayon threadpool
    pub fn mesh_sections_parallel(sections: &[ChunkSection]) -> Vec<SectionMesh> {
        sections.par_iter().map(Self::mesh_section).collect()
    }

    /// Meshes chunk section directly from 1.12.2 WorldStorage and TextureAtlas
    pub fn mesh_world_section(
        world: &casx_world::WorldStorage,
        chunk_x: i32,
        chunk_z: i32,
        sec_y: u8,
        atlas: &casx_assets::TextureAtlas,
    ) -> SectionMesh {
        let base_x = chunk_x * 16;
        let base_y = sec_y as i32 * 16;
        let base_z = chunk_z * 16;

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for ly in 0..16i32 {
            for lz in 0..16i32 {
                for lx in 0..16i32 {
                    let wx = base_x + lx;
                    let wy = base_y + ly;
                    let wz = base_z + lz;

                    let state = world.get_block_state(wx, wy, wz);
                    let (block_id, meta) =
                        casx_core::palette::SectionPaletteV340::unpack_block_id_meta(state);
                    if block_id == 0 {
                        continue;
                    }

                    // Check 6 adjacent faces: 0=Bottom, 1=Top, 2=North, 3=South, 4=West, 5=East
                    let neighbors = [
                        (0u32, wx, wy - 1, wz),
                        (1u32, wx, wy + 1, wz),
                        (2u32, wx, wy, wz - 1),
                        (3u32, wx, wy, wz + 1),
                        (4u32, wx - 1, wy, wz),
                        (5u32, wx + 1, wy, wz),
                    ];

                    for &(face_idx, nx, ny, nz) in &neighbors {
                        let n_state = world.get_block_state(nx, ny, nz);
                        let (n_id, _) =
                            casx_core::palette::SectionPaletteV340::unpack_block_id_meta(n_state);

                        if casx_assets::TextureAtlas::is_transparent(n_id) {
                            let tex_id =
                                atlas.get_block_texture_id(block_id, meta, face_idx as u8) as u32;
                            let base_idx = vertices.len() as u32;
                            let mut quad_verts = Self::generate_quad_vertices(
                                face_idx, lx as u32, ly as u32, lz as u32, 1, 1, tex_id,
                            );
                            for v in &mut quad_verts {
                                *v = v.with_chunk_offset(chunk_x, sec_y as i32, chunk_z);
                            }

                            vertices.extend_from_slice(&quad_verts);
                            indices.extend_from_slice(&[
                                base_idx,
                                base_idx + 1,
                                base_idx + 2,
                                base_idx,
                                base_idx + 2,
                                base_idx + 3,
                            ]);
                        }
                    }
                }
            }
        }

        SectionMesh { vertices, indices }
    }

    /// Meshes all visible chunk sections across the entire world into a single combined mesh.
    pub fn mesh_entire_world(
        world: &casx_world::WorldStorage,
        atlas: &casx_assets::TextureAtlas,
    ) -> SectionMesh {
        let mut combined_vertices = Vec::new();
        let mut combined_indices = Vec::new();

        for (&(cx, cz), column) in &world.chunks {
            for &sec_y in column.sections.keys() {
                if sec_y >= 0 {
                    let sec_mesh = Self::mesh_world_section(world, cx, cz, sec_y as u8, atlas);
                    let base_idx = combined_vertices.len() as u32;
                    combined_vertices.extend_from_slice(&sec_mesh.vertices);
                    for idx in sec_mesh.indices {
                        combined_indices.push(base_idx + idx);
                    }
                }
            }
        }

        SectionMesh {
            vertices: combined_vertices,
            indices: combined_indices,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greedy_meshing_merges_full_flat_slice_into_single_quad() {
        let mut section = ChunkSection::default();
        let stone = CanonicalBlockState::new(1, 0);

        // Place flat 16x16 stone platform at section floor (Y=0)
        for x in 0..16 {
            for z in 0..16 {
                section.set_block(x, 0, z, stone);
            }
        }

        let mesh = GreedyMesher::mesh_section(&section);

        // Top face of 16x16 stone platform must merge into a single quad (instead of 256 individual quads)
        // Total faces: Top (1 quad) + Bottom (1 quad) + 4 border faces (16 quads)
        assert!(
            mesh.quad_count() < 30,
            "Greedy Meshing should reduce polygon count: {}",
            mesh.quad_count()
        );
        assert_eq!(mesh.indices.len(), mesh.quad_count() * 6);
    }

    #[test]
    fn test_mesh_world_section_face_culling() {
        use casx_assets::TextureAtlas;
        use casx_core::palette::SectionPaletteV340;
        use casx_world::region::{ChunkColumnV340, ChunkSectionV340};

        let mut world = casx_world::WorldStorage::new();
        let atlas = TextureAtlas::new(512, 512, 16);

        // Place 2 adjacent stone blocks at (0, 64, 0) and (1, 64, 0)
        let mut data = vec![0u64; 256];
        data[0] |= 1u64;
        data[0] |= 1u64 << 4;
        let palette = vec![0, 16];
        let section = SectionPaletteV340::from_packet_data(4, Some(palette), data).unwrap();
        let mut col = ChunkColumnV340::new(0, 0);
        col.sections.insert(
            4,
            ChunkSectionV340::new_v340(4, section, vec![15; 2048], None),
        );
        world.chunks.insert((0, 0), col);

        let mesh = GreedyMesher::mesh_world_section(&world, 0, 0, 4, &atlas);
        // Each isolated block has 6 faces (2 blocks = 12 faces).
        // Facing West/East faces are shared and must be culled -> 10 quads remaining.
        assert_eq!(mesh.quad_count(), 10);
    }
}
