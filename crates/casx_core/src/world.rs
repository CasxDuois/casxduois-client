//! Bo nho dem the gioi (World Cache), cau truc Chunk Column va Section cho CasxDuois Client.

use glam::IVec3;
use std::collections::HashMap;

/// Toa do cot Chunk 2D trong the gioi (Chunk X, Chunk Z)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct ChunkPos {
    pub x: i32,
    pub z: i32,
}

impl ChunkPos {
    pub const fn new(x: i32, z: i32) -> Self {
        Self { x, z }
    }
}

/// Dinh danh khoi tieu chuan hoa (Canonical Block State Id)
///
/// Ho tro tra cuu truc tiep cac dac tinh vat ly:
/// - 1.12.2: (block_id << 4) | meta
/// - 26.2: Global Registry State ID tu PalettedContainer
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
pub struct CanonicalBlockStateId(pub u32);

impl CanonicalBlockStateId {
    pub const AIR: Self = Self(0);

    pub fn from_v340(block_id: u16, meta: u8) -> Self {
        Self(((block_id as u32) << 4) | ((meta as u32) & 0x0F))
    }

    pub fn to_v340(&self) -> (u16, u8) {
        let block_id = (self.0 >> 4) as u16;
        let meta = (self.0 & 0x0F) as u8;
        (block_id, meta)
    }

    pub fn from_v776_global(global_id: u32) -> Self {
        Self(global_id)
    }

    pub fn is_air(&self) -> bool {
        self.0 == 0
    }

    pub fn is_solid(&self) -> bool {
        let block_id = (self.0 >> 4) as u16;
        !self.is_air() && block_id != 8 && block_id != 9 && block_id != 10 && block_id != 11
    }

    pub fn is_opaque(&self) -> bool {
        let block_id = (self.0 >> 4) as u16;
        self.is_solid() && block_id != 20 && block_id != 95 // Glass / stained glass
    }

    pub fn light_emission(&self) -> u8 {
        let block_id = (self.0 >> 4) as u16;
        match block_id {
            10 | 11 => 15, // Lava
            50 => 14,      // Torch
            89 => 15,      // Glowstone
            124 => 15,     // Redstone Lamp on
            _ => 0,
        }
    }

    pub fn aabb(&self, x: i32, y: i32, z: i32) -> crate::jmath::AABB {
        crate::jmath::AABB::from_block(crate::jmath::IVec3::new(x, y, z))
    }

    pub fn face_uvs(&self) -> [u16; 6] {
        let block_id = (self.0 >> 4) as u16;
        let tex = match block_id {
            1 => 1,  // Stone
            2 => 2,  // Grass
            3 => 4,  // Dirt
            4 => 5,  // Cobblestone
            5 => 6,  // Planks
            7 => 7,  // Bedrock
            12 => 8, // Sand
            13 => 9, // Gravel
            17 => 10,// Log
            18 => 12,// Leaves
            20 => 13,// Glass
            8 | 9 => 14, // Water
            _ => 1,
        };
        [tex; 6]
    }
}

/// Trang thai khoi tieu chuan hoa (Canonical Block State)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CanonicalBlockState {
    pub id: u16,
    pub meta_or_palette: u16,
}

impl CanonicalBlockState {
    pub const AIR: Self = Self { id: 0, meta_or_palette: 0 };

    pub fn new(id: u16, meta_or_palette: u16) -> Self {
        Self { id, meta_or_palette }
    }

    pub fn is_air(&self) -> bool {
        self.id == 0
    }

    /// Kiem tra khoi co phai khoi dac (solid collision) hay khong
    pub fn is_solid(&self) -> bool {
        // Id 0 = air, 8/9 = water, 10/11 = lava, v.v.
        !self.is_air() && self.id != 8 && self.id != 9 && self.id != 10 && self.id != 11
    }
}

/// Mot phan doan Chunk ($16 \times 16 \times 16 = 4096$ khoi)
#[derive(Debug, Clone)]
pub struct ChunkSection {
    pub blocks: [CanonicalBlockState; 4096],
    pub block_light: [u8; 2048],
    pub sky_light: [u8; 2048],
    pub non_air_count: u16,
}

impl Default for ChunkSection {
    fn default() -> Self {
        Self {
            blocks: [CanonicalBlockState::AIR; 4096],
            block_light: [0; 2048],
            sky_light: [255; 2048], // Mac dinh anh sang troi toi da
            non_air_count: 0,
        }
    }
}

impl ChunkSection {
    #[inline(always)]
    fn index(x: usize, y: usize, z: usize) -> usize {
        (y << 8) | (z << 4) | x
    }

    pub fn get_block(&self, x: usize, y: usize, z: usize) -> CanonicalBlockState {
        if x < 16 && y < 16 && z < 16 {
            self.blocks[Self::index(x, y, z)]
        } else {
            CanonicalBlockState::AIR
        }
    }

    pub fn set_block(&mut self, x: usize, y: usize, z: usize, state: CanonicalBlockState) {
        if x < 16 && y < 16 && z < 16 {
            let idx = Self::index(x, y, z);
            let prev = self.blocks[idx];
            if prev.is_air() && !state.is_air() {
                self.non_air_count += 1;
            } else if !prev.is_air() && state.is_air() {
                self.non_air_count = self.non_air_count.saturating_sub(1);
            }
            self.blocks[idx] = state;
        }
    }
}

/// Cot Chunk day du toa do $(X, Z)$ chua mang cac Section theo chieu doc
#[derive(Debug, Clone)]
pub struct ChunkColumn {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub min_y: i32,
    pub sections: Vec<Option<ChunkSection>>,
    pub dirty_mesh: bool,
}

impl ChunkColumn {
    pub fn new(chunk_x: i32, chunk_z: i32, min_y: i32, section_count: usize) -> Self {
        let mut sections = Vec::with_capacity(section_count);
        for _ in 0..section_count {
            sections.push(None);
        }
        Self {
            chunk_x,
            chunk_z,
            min_y,
            sections,
            dirty_mesh: true,
        }
    }

    pub fn get_block(&self, rel_x: i32, y: i32, rel_z: i32) -> CanonicalBlockState {
        if !(0..16).contains(&rel_x) || !(0..16).contains(&rel_z) {
            return CanonicalBlockState::AIR;
        }
        let section_idx = ((y - self.min_y) >> 4) as usize;
        if section_idx >= self.sections.len() {
            return CanonicalBlockState::AIR;
        }
        if let Some(ref section) = self.sections[section_idx] {
            let sec_y = ((y - self.min_y) & 0xF) as usize;
            section.get_block(rel_x as usize, sec_y, rel_z as usize)
        } else {
            CanonicalBlockState::AIR
        }
    }

    pub fn set_block(&mut self, rel_x: i32, y: i32, rel_z: i32, state: CanonicalBlockState) {
        if !(0..16).contains(&rel_x) || !(0..16).contains(&rel_z) {
            return;
        }
        let section_idx = ((y - self.min_y) >> 4) as usize;
        if section_idx >= self.sections.len() {
            return;
        }
        if self.sections[section_idx].is_none() {
            if state.is_air() {
                return;
            }
            self.sections[section_idx] = Some(ChunkSection::default());
        }

        if let Some(ref mut section) = self.sections[section_idx] {
            let sec_y = ((y - self.min_y) & 0xF) as usize;
            section.set_block(rel_x as usize, sec_y, rel_z as usize, state);
            self.dirty_mesh = true;
        }
    }
}

/// Bo nho dem toan bo the gioi tai Client luu tru cac Chunk xung quanh nguoi choi
#[derive(Debug, Default, Clone)]
pub struct ClientWorldCache {
    chunks: HashMap<(i32, i32), ChunkColumn>,
}

impl ClientWorldCache {
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
        }
    }

    pub fn insert_chunk(&mut self, chunk: ChunkColumn) {
        self.chunks.insert((chunk.chunk_x, chunk.chunk_z), chunk);
    }

    pub fn unload_chunk(&mut self, chunk_x: i32, chunk_z: i32) -> Option<ChunkColumn> {
        self.chunks.remove(&(chunk_x, chunk_z))
    }

    pub fn loaded_chunk_count(&self) -> usize {
        self.chunks.len()
    }

    pub fn get_block(&self, x: i32, y: i32, z: i32) -> CanonicalBlockState {
        let chunk_x = x >> 4;
        let chunk_z = z >> 4;
        if let Some(chunk) = self.chunks.get(&(chunk_x, chunk_z)) {
            let rel_x = x & 0xF;
            let rel_z = z & 0xF;
            chunk.get_block(rel_x, y, rel_z)
        } else {
            CanonicalBlockState::AIR
        }
    }

    pub fn set_block(&mut self, x: i32, y: i32, z: i32, state: CanonicalBlockState) {
        let chunk_x = x >> 4;
        let chunk_z = z >> 4;
        if let Some(chunk) = self.chunks.get_mut(&(chunk_x, chunk_z)) {
            let rel_x = x & 0xF;
            let rel_z = z & 0xF;
            chunk.set_block(rel_x, y, rel_z, state);
        }
    }

    pub fn is_block_solid(&self, pos: IVec3) -> bool {
        self.get_block(pos.x, pos.y, pos.z).is_solid()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_section_block_access() {
        let mut section = ChunkSection::default();
        assert_eq!(section.non_air_count, 0);

        let stone = CanonicalBlockState::new(1, 0);
        section.set_block(0, 0, 0, stone);
        section.set_block(15, 15, 15, stone);

        assert_eq!(section.get_block(0, 0, 0), stone);
        assert_eq!(section.get_block(15, 15, 15), stone);
        assert_eq!(section.get_block(1, 1, 1), CanonicalBlockState::AIR);
        assert_eq!(section.non_air_count, 2);
    }

    #[test]
    fn test_world_cache_cross_chunk_boundary() {
        let mut world = ClientWorldCache::new();
        let chunk1 = ChunkColumn::new(0, 0, 0, 16);
        let chunk2 = ChunkColumn::new(1, 0, 0, 16);
        world.insert_chunk(chunk1);
        world.insert_chunk(chunk2);

        let dirt = CanonicalBlockState::new(3, 0);
        world.set_block(15, 64, 5, dirt);
        world.set_block(16, 64, 5, dirt);

        assert_eq!(world.get_block(15, 64, 5), dirt);
        assert_eq!(world.get_block(16, 64, 5), dirt);
        assert!(world.is_block_solid(IVec3::new(15, 64, 5)));
        assert!(!world.is_block_solid(IVec3::new(17, 64, 5)));
    }
}
