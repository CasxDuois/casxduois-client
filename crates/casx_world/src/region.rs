// crates/casx_world/src/region.rs
// Quan ly luu tru dia hinh Voxel Chunk va cap nhat khoi Minecraft 1.12.2

use casx_core::palette::SectionPaletteV340;
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

/// Mot Section 16x16x16 trong Minecraft 1.12.2
#[derive(Debug, Clone)]
pub struct ChunkSectionV340 {
    pub y_index: u8, // 0..15
    pub palette: SectionPaletteV340,
    pub block_light: Vec<u8>,
    pub sky_light: Option<Vec<u8>>,
}

/// Mot cot Chunk day du (16 sections theo truc Y tu 0 den 255)
#[derive(Debug, Clone)]
pub struct ChunkColumnV340 {
    pub x: i32,
    pub z: i32,
    pub sections: HashMap<u8, ChunkSectionV340>,
    pub biomes: Option<Vec<u8>>,
}

impl ChunkColumnV340 {
    pub fn new(x: i32, z: i32) -> Self {
        Self {
            x,
            z,
            sections: HashMap::new(),
            biomes: None,
        }
    }

    pub fn get_block_state(&self, rel_x: usize, y: usize, rel_z: usize) -> u16 {
        if rel_x >= 16 || rel_z >= 16 || y >= 256 {
            return 0;
        }
        let sec_y = (y / 16) as u8;
        let local_y = y % 16;
        if let Some(sec) = self.sections.get(&sec_y) {
            sec.palette.get_block_state(rel_x, local_y, rel_z)
        } else {
            0
        }
    }
}

/// Bo luu tru toan bo the gioi Voxel tren Client
#[derive(Debug, Default)]
pub struct WorldStorage {
    pub chunks: HashMap<(i32, i32), ChunkColumnV340>,
}

impl WorldStorage {
    pub fn new() -> Self {
        Self {
            chunks: HashMap::new(),
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
            .or_insert_with(|| ChunkColumnV340::new(chunk_x, chunk_z));

        for sec_y in 0..16u8 {
            if (primary_bit_mask & (1 << sec_y)) != 0 {
                // Doc bits per block
                let mut bpe_buf = [0u8; 1];
                std::io::Read::read_exact(&mut cursor, &mut bpe_buf)
                    .map_err(|_| WorldError::CorruptedChunkData)?;
                let bits_per_entry = bpe_buf[0];

                // Doc palette neu bits_per_entry <= 8
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

                // Doc do dai data array (so luong u64)
                let data_len = read_varint(&mut cursor)
                    .map_err(|e| WorldError::ProtocolError(e.to_string()))? as usize;
                let mut u64_data = Vec::with_capacity(data_len);
                for _ in 0..data_len {
                    let mut u64_bytes = [0u8; 8];
                    std::io::Read::read_exact(&mut cursor, &mut u64_bytes)
                        .map_err(|_| WorldError::CorruptedChunkData)?;
                    u64_data.push(u64::from_be_bytes(u64_bytes));
                }

                // Doc Block Light (2048 bytes)
                let mut block_light = vec![0u8; 2048];
                let _ = std::io::Read::read_exact(&mut cursor, &mut block_light);

                // Doc Sky Light (2048 bytes neu con du lieu)
                let mut sky_light = vec![0u8; 2048];
                let sky_light = if std::io::Read::read_exact(&mut cursor, &mut sky_light).is_ok() {
                    Some(sky_light)
                } else {
                    None
                };

                if let Ok(section_palette) = SectionPaletteV340::from_packet_data(bits_per_entry, palette_list, u64_data) {
                    column.sections.insert(sec_y, ChunkSectionV340 {
                        y_index: sec_y,
                        palette: section_palette,
                        block_light,
                        sky_light,
                    });
                }
            } else if ground_up_continuous {
                column.sections.remove(&sec_y);
            }
        }

        // Biomes neu ground_up_continuous
        if ground_up_continuous {
            let mut biomes = vec![0u8; 256];
            if std::io::Read::read_exact(&mut cursor, &mut biomes).is_ok() {
                column.biomes = Some(biomes);
            }
        }

        Ok(())
    }

    /// Lay trang thai khoi tai toa do the gioi tuyet doi (x, y, z)
    pub fn get_block_state(&self, x: i32, y: i32, z: i32) -> u16 {
        if y < 0 || y >= 256 {
            return 0; // Air ngoai the gioi
        }
        let chunk_x = x >> 4;
        let chunk_z = z >> 4;
        let rel_x = (x & 15) as usize;
        let rel_z = (z & 15) as usize;

        if let Some(col) = self.chunks.get(&(chunk_x, chunk_z)) {
            col.get_block_state(rel_x, y as usize, rel_z)
        } else {
            0
        }
    }

    /// Kiem tra khoi co phai la vat the ran co the va cham khong (Air id = 0 khong ran)
    pub fn is_solid_block(&self, x: i32, y: i32, z: i32) -> bool {
        let state = self.get_block_state(x, y, z);
        let (id, _meta) = SectionPaletteV340::unpack_block_id_meta(state);
        // id != 0 (Air) va id != 8/9 (Water) va id != 10/11 (Lava)
        id != 0 && id != 8 && id != 9 && id != 10 && id != 11
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_world_storage_apply_chunk_and_query() {
        let mut world = WorldStorage::new();
        // Gia lap section 4 (Y = 64..79) voi 1 khoi Stone (id 1) tai (0, 0, 0) cuc bo (the gioi: 0, 64, 0)
        let mut data = vec![0u64; 256];
        data[0] |= 1u64; // index 0 co palette index 1
        let palette = vec![0, 16]; // id 0 (air), id 1 (stone = 16)

        let section_palette = SectionPaletteV340::from_packet_data(4, Some(palette), data).unwrap();
        let mut col = ChunkColumnV340::new(0, 0);
        col.sections.insert(4, ChunkSectionV340 {
            y_index: 4,
            palette: section_palette,
            block_light: vec![0; 2048],
            sky_light: None,
        });
        world.chunks.insert((0, 0), col);

        assert_eq!(world.get_block_state(0, 64, 0), 16);
        assert!(world.is_solid_block(0, 64, 0));
        assert!(!world.is_solid_block(0, 65, 0)); // Air
    }
}
