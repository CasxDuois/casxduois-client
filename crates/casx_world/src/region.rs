// crates/casx_world/src/region.rs
// Quan ly luu tru dia hinh Voxel Chunk ho tro da phien ban (1.12.2 Y=0..255 va 26.2 Y=-64..319)

use casx_core::palette::{PalettedContainerV776, SectionPaletteV340};
use casx_core::codec::read_varint;
use std::collections::HashMap;
use std::io::Cursor;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WorldError {
    #[error("Loi doc packet giao thuc: {0}")]
    ProtocolError(String),
    #[error("Du lieu chunk bi cat ngang hoac khong hop le")]
    CorruptedChunkData,
}

/// Du lieu Palette cua mot Section (ho tro ca 1.12.2 va 26.2)
#[derive(Debug, Clone)]
pub enum SectionData {
    V340(SectionPaletteV340),
    V776(PalettedContainerV776),
}

impl SectionData {
    pub fn get_block_state(&self, x: usize, y: usize, z: usize) -> u32 {
        match self {
            Self::V340(p) => p.get_block_state(x, y, z) as u32,
            Self::V776(p) => p.get_block_state(x, y, z),
        }
    }
}

/// Mot Section 16x16x16 trong khong gian 3D
#[derive(Debug, Clone)]
pub struct ChunkSection {
    pub y_index: i8, // Chi so section (co the am tu -4 den 19 cho 26.2 hoac 0..15 cho 1.12.2)
    pub data: SectionData,
    pub block_light: Vec<u8>,
    pub sky_light: Option<Vec<u8>>,
}

// Giu nguyen tuong thich ten goi cu cho cac crate khac
pub type ChunkSectionV340 = ChunkSection;

impl ChunkSection {
    pub fn new_v340(y_index: i8, palette: SectionPaletteV340, block_light: Vec<u8>, sky_light: Option<Vec<u8>>) -> Self {
        Self {
            y_index,
            data: SectionData::V340(palette),
            block_light,
            sky_light,
        }
    }

    pub fn new_v776(y_index: i8, container: PalettedContainerV776, block_light: Vec<u8>, sky_light: Option<Vec<u8>>) -> Self {
        Self {
            y_index,
            data: SectionData::V776(container),
            block_light,
            sky_light,
        }
    }

    pub fn palette_v340(&self) -> Option<&SectionPaletteV340> {
        match &self.data {
            SectionData::V340(p) => Some(p),
            _ => None,
        }
    }
}

/// Mot cot Chunk day du (chieu cao dong ho tro ca Y < 0)
#[derive(Debug, Clone)]
pub struct ChunkColumn {
    pub x: i32,
    pub z: i32,
    pub min_y: i32,
    pub height: u32,
    pub sections: HashMap<i8, ChunkSection>,
    pub biomes: Option<Vec<u8>>,
}

pub type ChunkColumnV340 = ChunkColumn;

impl ChunkColumn {
    pub fn new(x: i32, z: i32) -> Self {
        Self {
            x,
            z,
            min_y: 0,
            height: 256,
            sections: HashMap::new(),
            biomes: None,
        }
    }

    pub fn new_v776(x: i32, z: i32, min_y: i32, height: u32) -> Self {
        Self {
            x,
            z,
            min_y,
            height,
            sections: HashMap::new(),
            biomes: None,
        }
    }

    pub fn get_block_state(&self, rel_x: usize, y: i32, rel_z: usize) -> u32 {
        if rel_x >= 16 || rel_z >= 16 || y < self.min_y || y >= self.min_y + self.height as i32 {
            return 0;
        }

        let sec_y = (y >> 4) as i8;
        let local_y = (y & 15) as usize;

        if let Some(sec) = self.sections.get(&sec_y) {
            sec.data.get_block_state(rel_x, local_y, rel_z)
        } else {
            0
        }
    }
}

/// Bo luu tru toan bo the gioi Voxel tren Client
#[derive(Debug, Default)]
pub struct WorldStorage {
    pub chunks: HashMap<(i32, i32), ChunkColumn>,
    pub default_min_y: i32,
    pub default_height: u32,
}

impl WorldStorage {
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
            default_min_y: 0,
            default_height: 256,
        }
    }

    pub fn new_v776(min_y: i32, height: u32) -> Self {
        Self {
            chunks: HashMap::new(),
            default_min_y: min_y,
            default_height: height,
        }
    }

    /// Giai ma va nap goi tin Chunk Data (0x20) cua Minecraft 1.12.2
    pub fn apply_chunk_packet_v340(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
        ground_up_continuous: bool,
        primary_bit_mask: u16,
        data: &[u8],
    ) -> Result<(), WorldError> {
        let mut cursor = Cursor::new(data);
        let column = self.chunks.entry((chunk_x, chunk_z))
            .or_insert_with(|| ChunkColumn::new(chunk_x, chunk_z));

        for sec_y in 0..16u8 {
            let sec_idx = sec_y as i8;
            if (primary_bit_mask & (1 << sec_y)) != 0 {
                let mut bpe_buf = [0u8; 1];
                std::io::Read::read_exact(&mut cursor, &mut bpe_buf)
                    .map_err(|_| WorldError::CorruptedChunkData)?;
                let bits_per_entry = bpe_buf[0];

                let palette_list = if bits_per_entry <= 8 {
                    let palette_len = read_varint(&mut cursor)
                        .map_err(|e| WorldError::ProtocolError(e.to_string()))? as usize;
                    let mut palette = Vec::with_capacity(palette_len);
                    for _ in 0..palette_len {
                        let id = read_varint(&mut cursor)
                            .map_err(|e| WorldError::ProtocolError(e.to_string()))? as u16;
                        palette.push(id);
                    }
                    Some(palette)
                } else {
                    None
                };

                let data_len = read_varint(&mut cursor)
                    .map_err(|e| WorldError::ProtocolError(e.to_string()))? as usize;
                let mut u64_data = Vec::with_capacity(data_len);
                for _ in 0..data_len {
                    let mut u64_bytes = [0u8; 8];
                    std::io::Read::read_exact(&mut cursor, &mut u64_bytes)
                        .map_err(|_| WorldError::CorruptedChunkData)?;
                    u64_data.push(u64::from_be_bytes(u64_bytes));
                }

                let mut block_light = vec![0u8; 2048];
                let _ = std::io::Read::read_exact(&mut cursor, &mut block_light);

                let mut sky_light = vec![0u8; 2048];
                let sky_light = if std::io::Read::read_exact(&mut cursor, &mut sky_light).is_ok() {
                    Some(sky_light)
                } else {
                    None
                };

                if let Ok(section_palette) = SectionPaletteV340::from_packet_data(bits_per_entry, palette_list, u64_data) {
                    column.sections.insert(sec_idx, ChunkSection::new_v340(sec_idx, section_palette, block_light, sky_light));
                }
            } else if ground_up_continuous {
                column.sections.remove(&sec_idx);
            }
        }

        if ground_up_continuous {
            let mut biomes = vec![0u8; 256];
            if std::io::Read::read_exact(&mut cursor, &mut biomes).is_ok() {
                column.biomes = Some(biomes);
            }
        }

        Ok(())
    }

    /// Nap truc tiep mot Section 26.2 (Protocol 776) voi chieu cao tuy y
    pub fn insert_section_v776(&mut self, chunk_x: i32, chunk_z: i32, sec_y: i8, container: PalettedContainerV776) {
        let min_y = self.default_min_y;
        let height = self.default_height;
        let column = self.chunks.entry((chunk_x, chunk_z))
            .or_insert_with(|| ChunkColumn::new_v776(chunk_x, chunk_z, min_y, height));

        let sec = ChunkSection::new_v776(sec_y, container, vec![15; 2048], Some(vec![15; 2048]));
        column.sections.insert(sec_y, sec);
    }

    /// Lay trang thai khoi tai toa do the gioi tuyet doi (x, y, z)
    pub fn get_block_state(&self, x: i32, y: i32, z: i32) -> u16 {
        let chunk_x = x >> 4;
        let chunk_z = z >> 4;
        let rel_x = (x & 15) as usize;
        let rel_z = (z & 15) as usize;

        if let Some(col) = self.chunks.get(&(chunk_x, chunk_z)) {
            col.get_block_state(rel_x, y, rel_z) as u16
        } else {
            0
        }
    }

    /// Kiem tra khoi co phai la vat the ran co the va cham khong
    pub fn is_solid_block(&self, x: i32, y: i32, z: i32) -> bool {
        let state = self.get_block_state(x, y, z);
        let (id, _meta) = SectionPaletteV340::unpack_block_id_meta(state);
        id != 0 && id != 8 && id != 9 && id != 10 && id != 11
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_storage_apply_chunk_and_query() {
        let mut world = WorldStorage::new();
        let mut data = vec![0u64; 256];
        data[0] |= 1u64;
        let palette = vec![0, 16];

        let section_palette = SectionPaletteV340::from_packet_data(4, Some(palette), data).unwrap();
        let mut col = ChunkColumn::new(0, 0);
        col.sections.insert(4, ChunkSection::new_v340(4, section_palette, vec![0; 2048], None));
        world.chunks.insert((0, 0), col);

        assert_eq!(world.get_block_state(0, 64, 0), 16);
        assert!(world.is_solid_block(0, 64, 0));
        assert!(!world.is_solid_block(0, 65, 0));
    }

    #[test]
    fn test_world_storage_negative_y_v776() {
        // Thu nghiem Section tai Y am: Section -4 (Y = -64..-49)
        let mut world = WorldStorage::new_v776(-64, 384);
        let container = PalettedContainerV776::single(16); // stone toan bo
        world.insert_section_v776(0, 0, -4, container);

        // Kiem tra khoi tai toa do the gioi Y = -64 va Y = -50
        assert_eq!(world.get_block_state(0, -64, 0), 16);
        assert_eq!(world.get_block_state(5, -50, 5), 16);
        assert!(world.is_solid_block(0, -64, 0));

        // Kiem tra ngoai pham vi Y (vi du Y = -65 hoac Y = 320)
        assert_eq!(world.get_block_state(0, -65, 0), 0);
        assert_eq!(world.get_block_state(0, 320, 0), 0);
    }
}
