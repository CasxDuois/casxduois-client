// crates/casx_core/src/palette.rs
// Bo giai nen mang bit u64 va Palette chuan Minecraft 1.12.2 (Protocol 340)

use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum PaletteError {
    #[error("Bits per entry khong hop le: {0}")]
    InvalidBitsPerEntry(u8),
    #[error("Kich thuoc du lieu mang bit khong hop le: can {expected} u64, nhan duoc {actual}")]
    InvalidDataLength { expected: usize, actual: usize },
    #[error("Chi so palette vuot nguong: index {index} >= palette len {len}")]
    IndexOutOfBounds { index: usize, len: usize },
}

/// Bo luu tru mang bit kieu 1.12.2 voi co che spanning (cac phan tu co the vat ngang 2 u64 lien tiep)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitStorageV340 {
    bits_per_entry: u8,
    size: usize,
    mask: u64,
    data: Vec<u64>,
}

impl BitStorageV340 {
    pub const SECTION_BLOCK_COUNT: usize = 4096; // 16 x 16 x 16

    pub fn new(bits_per_entry: u8, size: usize, data: Vec<u64>) -> Result<Self, PaletteError> {
        if bits_per_entry == 0 || bits_per_entry > 16 {
            return Err(PaletteError::InvalidBitsPerEntry(bits_per_entry));
        }

        let expected_u64 = ((size * bits_per_entry as usize) + 63) / 64;
        if data.len() < expected_u64 {
            return Err(PaletteError::InvalidDataLength {
                expected: expected_u64,
                actual: data.len(),
            });
        }

        let mask = (1u64 << bits_per_entry) - 1;
        Ok(Self {
            bits_per_entry,
            size,
            mask,
            data,
        })
    }

    /// Lay gia tri raw tai chi so index (0 <= index < size)
    pub fn get(&self, index: usize) -> u16 {
        if index >= self.size {
            return 0;
        }

        let b = self.bits_per_entry as usize;
        let start_bit = index * b;
        let start_index = start_bit / 64;
        let bit_offset = start_bit % 64;

        if bit_offset + b <= 64 {
            // Nam tron ven trong 1 u64
            let val = (self.data[start_index] >> bit_offset) & self.mask;
            val as u16
        } else {
            // Vat ngang qua ranh gioi cua 2 u64 lien tiep (1.12.2 Spanning)
            let end_index = start_index + 1;
            let bits_in_first = 64 - bit_offset;
            let part1 = self.data[start_index] >> bit_offset;
            let part2 = if end_index < self.data.len() {
                self.data[end_index] << bits_in_first
            } else {
                0
            };
            let val = (part1 | part2) & self.mask;
            val as u16
        }
    }

    pub fn bits_per_entry(&self) -> u8 {
        self.bits_per_entry
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

/// Dinh nghia Section Palette cua 1.12.2
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionPaletteV340 {
    /// Che do Indirect Palette (bits_per_entry <= 8)
    Indirect {
        bits_per_entry: u8,
        palette: Vec<u16>,
        storage: BitStorageV340,
    },
    /// Che do Direct Global ID (bits_per_entry > 8, thong thuong la 13 bits)
    Direct {
        bits_per_entry: u8,
        storage: BitStorageV340,
    },
}

impl SectionPaletteV340 {
    /// Khoi tao Section tu du lieu mang bit va palette truyen tu packet Chunk Data (0x20)
    pub fn from_packet_data(
        bits_per_entry: u8,
        palette_list: Option<Vec<u16>>,
        data: Vec<u64>,
    ) -> Result<Self, PaletteError> {
        let storage = BitStorageV340::new(bits_per_entry, BitStorageV340::SECTION_BLOCK_COUNT, data)?;

        if bits_per_entry <= 8 {
            let palette = palette_list.unwrap_or_default();
            Ok(Self::Indirect {
                bits_per_entry,
                palette,
                storage,
            })
        } else {
            Ok(Self::Direct {
                bits_per_entry,
                storage,
            })
        }
    }

    /// Lay Block State toan cuc (13-bit: (block_id << 4) | meta) tai toa do cuc bo (x, y, z)
    pub fn get_block_state(&self, x: usize, y: usize, z: usize) -> u16 {
        if x >= 16 || y >= 16 || z >= 16 {
            return 0;
        }
        // Minecraft 1.12.2 section block index order: (y << 8) | (z << 4) | x
        let index = (y << 8) | (z << 4) | x;

        match self {
            Self::Indirect { palette, storage, .. } => {
                let palette_idx = storage.get(index) as usize;
                if palette_idx < palette.len() {
                    palette[palette_idx]
                } else {
                    0
                }
            }
            Self::Direct { storage, .. } => storage.get(index),
        }
    }

    /// Phan tach 13-bit global state thanh block_id va 4-bit metadata
    pub fn unpack_block_id_meta(state: u16) -> (u16, u8) {
        let block_id = state >> 4;
        let meta = (state & 0x0F) as u8;
        (block_id, meta)
    }

    /// Dong goi block_id va metadata thanh 13-bit global state
    pub fn pack_block_id_meta(block_id: u16, meta: u8) -> u16 {
        (block_id << 4) | ((meta & 0x0F) as u16)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bit_storage_v340_spanning_across_u64() {
        // Thu nghiem 5 bits moi entry, 4096 entries
        // entry 12: start_bit = 12 * 5 = 60, offset = 60.
        // entries 12 vat ngang tu u64[0] sang u64[1]: 4 bits o u64[0], 1 bit o u64[1]
        let mut data = vec![0u64; 320]; // 4096 * 5 / 64 = 320 u64
        
        // Ghi gia tri 0x1B (27 = 0b11011) vao entry 12:
        // bit 0..3 (0b1011) vao u64[0] tai bit 60..63
        // bit 4 (0b1) vao u64[1] tai bit 0
        data[0] |= 0x0B_u64 << 60;
        data[1] |= 0x01_u64;

        let storage = BitStorageV340::new(5, 4096, data).unwrap();
        assert_eq!(storage.get(12), 27);
    }

    #[test]
    fn test_section_palette_indirect_and_direct() {
        // Test Indirect: bits = 4
        let palette = vec![0, 16, 32, 48]; // air, stone, grass, dirt
        let mut data = vec![0u64; 256]; // 4096 * 4 / 64 = 256 u64
        // Dat block tai (0, 0, 0) chi so 0 gia tri palette 2 (grass = 32)
        // index 0 -> bit 0..3: val = 2
        data[0] |= 2u64;

        let section = SectionPaletteV340::from_packet_data(4, Some(palette), data).unwrap();
        assert_eq!(section.get_block_state(0, 0, 0), 32);
        let (id, meta) = SectionPaletteV340::unpack_block_id_meta(32);
        assert_eq!(id, 2); // 32 >> 4 = 2
        assert_eq!(meta, 0);

        // Test pack/unpack parity
        let packed = SectionPaletteV340::pack_block_id_meta(35, 14); // red wool: id 35, meta 14
        assert_eq!(packed, (35 << 4) | 14);
        assert_eq!(SectionPaletteV340::unpack_block_id_meta(packed), (35, 14));
    }
}
