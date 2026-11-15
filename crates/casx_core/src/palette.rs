// crates/casx_core/src/palette.rs
// Bit-packed u64 storage and PalettedContainer for Protocol 340 (1.12.2) and Protocol 776 (26.2).

use thiserror::Error;

#[derive(Error, Debug, PartialEq, Eq)]
pub enum PaletteError {
    #[error("Invalid bits per entry: {0}")]
    InvalidBitsPerEntry(u8),
    #[error("Invalid bit storage data length: expected {expected} u64s, got {actual}")]
    InvalidDataLength { expected: usize, actual: usize },
    #[error("Palette index out of bounds: index {index} >= palette len {len}")]
    IndexOutOfBounds { index: usize, len: usize },
}

/// Bit-storage array for 1.12.2 with bit-spanning across adjacent u64 boundaries
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

        let expected_u64 = (size * bits_per_entry as usize).div_ceil(64);
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

    pub fn new_empty(bits_per_entry: u8, size: usize) -> Result<Self, PaletteError> {
        if bits_per_entry == 0 || bits_per_entry > 16 {
            return Err(PaletteError::InvalidBitsPerEntry(bits_per_entry));
        }
        let expected_u64 = (size * bits_per_entry as usize).div_ceil(64);
        let mask = (1u64 << bits_per_entry) - 1;
        Ok(Self {
            bits_per_entry,
            size,
            mask,
            data: vec![0u64; expected_u64],
        })
    }

    /// Sets the raw value at the specified index (0 <= index < size).
    pub fn set(&mut self, index: usize, val: u16) {
        if index >= self.size {
            return;
        }

        let b = self.bits_per_entry as usize;
        let start_bit = index * b;
        let start_index = start_bit / 64;
        let bit_offset = start_bit % 64;
        let val_masked = (val as u64) & self.mask;

        if bit_offset + b <= 64 {
            self.data[start_index] &= !(self.mask << bit_offset);
            self.data[start_index] |= val_masked << bit_offset;
        } else {
            let end_index = start_index + 1;
            let bits_in_first = 64 - bit_offset;
            let mask1 = (1u64 << bits_in_first) - 1;
            self.data[start_index] &= !(mask1 << bit_offset);
            self.data[start_index] |= (val_masked & mask1) << bit_offset;

            if end_index < self.data.len() {
                let bits_in_second = b - bits_in_first;
                let mask2 = (1u64 << bits_in_second) - 1;
                self.data[end_index] &= !mask2;
                self.data[end_index] |= (val_masked >> bits_in_first) & mask2;
            }
        }
    }

    /// Retrieves the raw value at the specified index (0 <= index < size).
    pub fn get(&self, index: usize) -> u16 {
        if index >= self.size {
            return 0;
        }

        let b = self.bits_per_entry as usize;
        let start_bit = index * b;
        let start_index = start_bit / 64;
        let bit_offset = start_bit % 64;

        if bit_offset + b <= 64 {
            // Fully contained within single u64
            let val = (self.data[start_index] >> bit_offset) & self.mask;
            val as u16
        } else {
            // Spanning across adjacent u64 boundary (1.12.2 Spanning)
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

/// Section Palette definition for 1.12.2
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SectionPaletteV340 {
    /// Indirect Palette mode (bits_per_entry <= 8)
    Indirect {
        bits_per_entry: u8,
        palette: Vec<u16>,
        storage: BitStorageV340,
    },
    /// Direct Global ID mode (bits_per_entry > 8, typically 13 bits).
    Direct {
        bits_per_entry: u8,
        storage: BitStorageV340,
    },
}

impl SectionPaletteV340 {
    pub fn new_direct() -> Self {
        let storage = BitStorageV340::new_empty(13, BitStorageV340::SECTION_BLOCK_COUNT)
            .expect("Valid 13-bit storage");
        Self::Direct {
            bits_per_entry: 13,
            storage,
        }
    }

    /// Sets global block state at local coordinates (x, y, z).
    pub fn set_block_state(&mut self, x: usize, y: usize, z: usize, state: u16) {
        if x >= 16 || y >= 16 || z >= 16 {
            return;
        }
        let index = (y << 8) | (z << 4) | x;
        match self {
            Self::Direct { storage, .. } => storage.set(index, state),
            Self::Indirect {
                palette, storage, ..
            } => {
                if let Some(pos) = palette.iter().position(|&s| s == state) {
                    storage.set(index, pos as u16);
                }
            }
        }
    }

    /// Initializes Section from bit storage and palette passed in Chunk Data (0x20).
    pub fn from_packet_data(
        bits_per_entry: u8,
        palette_list: Option<Vec<u16>>,
        data: Vec<u64>,
    ) -> Result<Self, PaletteError> {
        let storage =
            BitStorageV340::new(bits_per_entry, BitStorageV340::SECTION_BLOCK_COUNT, data)?;

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

    /// Returns global block state (13-bit: (block_id << 4) | meta) at local coordinates (x, y, z).
    pub fn get_block_state(&self, x: usize, y: usize, z: usize) -> u16 {
        if x >= 16 || y >= 16 || z >= 16 {
            return 0;
        }
        // Minecraft 1.12.2 section block index order: (y << 8) | (z << 4) | x
        let index = (y << 8) | (z << 4) | x;

        match self {
            Self::Indirect {
                palette, storage, ..
            } => {
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

    /// Unpacks 13-bit global state into block_id and 4-bit metadata
    pub fn unpack_block_id_meta(state: u16) -> (u16, u8) {
        let block_id = state >> 4;
        let meta = (state & 0x0F) as u8;
        (block_id, meta)
    }

    /// Packs block_id and metadata into 13-bit global state
    pub fn pack_block_id_meta(block_id: u16, meta: u8) -> u16 {
        (block_id << 4) | ((meta & 0x0F) as u16)
    }
}

/// Bit-storage array for 1.16+ and 26.2 (Protocol 776) with padding
/// Elements never cross u64 boundaries, trailing bits in each u64 are zero
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BitStorageV776 {
    bits_per_entry: u8,
    values_per_u64: usize,
    size: usize,
    mask: u64,
    data: Vec<u64>,
}

impl BitStorageV776 {
    pub const SECTION_BLOCK_COUNT: usize = 4096;

    pub fn new(bits_per_entry: u8, size: usize, data: Vec<u64>) -> Result<Self, PaletteError> {
        if bits_per_entry == 0 || bits_per_entry > 32 {
            return Err(PaletteError::InvalidBitsPerEntry(bits_per_entry));
        }

        let values_per_u64 = 64 / bits_per_entry as usize;
        let expected_u64 = size.div_ceil(values_per_u64);
        if data.len() < expected_u64 {
            return Err(PaletteError::InvalidDataLength {
                expected: expected_u64,
                actual: data.len(),
            });
        }

        let mask = (1u64 << bits_per_entry) - 1;
        Ok(Self {
            bits_per_entry,
            values_per_u64,
            size,
            mask,
            data,
        })
    }

    /// Returns value at index (0 <= index < size)
    pub fn get(&self, index: usize) -> u32 {
        if index >= self.size {
            return 0;
        }

        let u64_idx = index / self.values_per_u64;
        let bit_offset = (index % self.values_per_u64) * self.bits_per_entry as usize;

        ((self.data[u64_idx] >> bit_offset) & self.mask) as u32
    }

    pub fn bits_per_entry(&self) -> u8 {
        self.bits_per_entry
    }

    pub fn size(&self) -> usize {
        self.size
    }
}

/// Paletted container according to 26.2 (Protocol 776) specification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PalettedContainerV776 {
    /// Single value for entire section (bits_per_entry == 0)
    SingleValued { value: u32 },
    /// Indirect Palette mode (4 <= bits_per_entry <= 8)
    Indirect {
        bits_per_entry: u8,
        palette: Vec<u32>,
        storage: BitStorageV776,
    },
    /// Direct Global Palette mode (bits_per_entry >= 15)
    Direct {
        bits_per_entry: u8,
        storage: BitStorageV776,
    },
}

impl PalettedContainerV776 {
    pub fn single(value: u32) -> Self {
        Self::SingleValued { value }
    }

    pub fn indirect(
        bits_per_entry: u8,
        palette: Vec<u32>,
        data: Vec<u64>,
    ) -> Result<Self, PaletteError> {
        let storage =
            BitStorageV776::new(bits_per_entry, BitStorageV776::SECTION_BLOCK_COUNT, data)?;
        Ok(Self::Indirect {
            bits_per_entry,
            palette,
            storage,
        })
    }

    pub fn direct(bits_per_entry: u8, data: Vec<u64>) -> Result<Self, PaletteError> {
        let storage =
            BitStorageV776::new(bits_per_entry, BitStorageV776::SECTION_BLOCK_COUNT, data)?;
        Ok(Self::Direct {
            bits_per_entry,
            storage,
        })
    }

    /// Returns block state ID at local section coordinates (x, y, z).
    pub fn get_block_state(&self, x: usize, y: usize, z: usize) -> u32 {
        if x >= 16 || y >= 16 || z >= 16 {
            return 0;
        }
        let index = (y << 8) | (z << 4) | x;

        match self {
            Self::SingleValued { value } => *value,
            Self::Indirect {
                palette, storage, ..
            } => {
                let idx = storage.get(index) as usize;
                if idx < palette.len() {
                    palette[idx]
                } else {
                    0
                }
            }
            Self::Direct { storage, .. } => storage.get(index),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bit_storage_v340_spanning_across_u64() {
        // Test 5 bits per entry, 4096 entries spanning across u64 boundaries
        let mut data = vec![0u64; 320]; // 4096 * 5 / 64 = 320 u64

        // Write value 0x1B (27 = 0b11011) into entry 12:
        // bit 0..3 (0b1011) into u64[0] at bits 60..63
        // bit 4 (0b1) into u64[1] at bit 0
        data[0] |= 0x0B_u64 << 60;
        data[1] |= 0x01_u64;

        let storage = BitStorageV340::new(5, 4096, data).unwrap();
        assert_eq!(storage.get(12), 27);

        // Test set and get roundtrip spanning u64 boundary
        let mut empty = BitStorageV340::new_empty(5, 4096).unwrap();
        empty.set(12, 27);
        assert_eq!(empty.get(12), 27);
        assert_eq!(empty.get(11), 0);
        assert_eq!(empty.get(13), 0);
    }

    #[test]
    fn test_section_palette_indirect_and_direct() {
        // Test Indirect: bits = 4
        let palette = vec![0, 16, 32, 48]; // air, stone, grass, dirt
        let mut data = vec![0u64; 256]; // 4096 * 4 / 64 = 256 u64
                                        // Place block at (0, 0, 0) index 0 with palette value 2 (grass = 32)
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

    #[test]
    fn test_bit_storage_v776_padded_elements() {
        // Test 5 bits per entry with padded non-spanning storage
        let mut data = vec![0u64; 342]; // (4096 + 11) / 12 = 342 u64

        // Write value 27 into entry 12: located at start of u64[1] at bits 0..4
        data[1] |= 27u64;

        let storage = BitStorageV776::new(5, 4096, data).unwrap();
        assert_eq!(storage.get(12), 27);
        assert_eq!(storage.get(0), 0);
    }

    #[test]
    fn test_paletted_container_v776_single_and_indirect() {
        // 1. Single-valued (entire section is block id 100)
        let single = PalettedContainerV776::single(100);
        assert_eq!(single.get_block_state(0, 0, 0), 100);
        assert_eq!(single.get_block_state(15, 15, 15), 100);

        // 2. Indirect (bits = 4, palette: [0, 42, 99])
        let palette = vec![0, 42, 99];
        let mut data = vec![0u64; 256];
        // Place block at (0, 0, 0) index 0 -> palette 1 (block id 42)
        // Place block at (1, 0, 0) index 1 -> palette 2 (block id 99)
        data[0] |= 1u64; // bit 0..3 = 1
        data[0] |= 2u64 << 4; // bit 4..7 = 2
        let container = PalettedContainerV776::indirect(4, palette, data).unwrap();
        assert_eq!(container.get_block_state(0, 0, 0), 42);
        assert_eq!(container.get_block_state(1, 0, 0), 99);
    }
}
