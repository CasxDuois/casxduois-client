// crates/casx_assets/src/atlas.rs
// Texture atlas packer, UV coordinate mapping, and procedural fallback generator.

use std::collections::HashMap;

/// Internal texture IDs in Atlas
pub const TEX_MISSING: u16 = 0;
pub const TEX_STONE: u16 = 1;
pub const TEX_GRASS_TOP: u16 = 2;
pub const TEX_GRASS_SIDE: u16 = 3;
pub const TEX_DIRT: u16 = 4;
pub const TEX_COBBLESTONE: u16 = 5;
pub const TEX_PLANKS: u16 = 6;
pub const TEX_BEDROCK: u16 = 7;
pub const TEX_SAND: u16 = 8;
pub const TEX_GRAVEL: u16 = 9;
pub const TEX_LOG_SIDE: u16 = 10;
pub const TEX_LOG_TOP: u16 = 11;
pub const TEX_LEAVES: u16 = 12;
pub const TEX_GLASS: u16 = 13;
pub const TEX_WATER: u16 = 14;

/// TextureAtlas buffer manager.
#[derive(Debug, Clone)]
pub struct TextureAtlas {
    pub width: u32,
    pub height: u32,
    pub tile_size: u32,
    pub tiles_per_row: u32,
    pub texture_map: HashMap<String, u16>,
}

impl TextureAtlas {
    pub fn new(width: u32, height: u32, tile_size: u32) -> Self {
        let tiles_per_row = width / tile_size;
        let mut texture_map = HashMap::new();
        texture_map.insert("stone".to_string(), TEX_STONE);
        texture_map.insert("grass_top".to_string(), TEX_GRASS_TOP);
        texture_map.insert("grass_side".to_string(), TEX_GRASS_SIDE);
        texture_map.insert("dirt".to_string(), TEX_DIRT);
        texture_map.insert("cobblestone".to_string(), TEX_COBBLESTONE);
        texture_map.insert("planks".to_string(), TEX_PLANKS);
        texture_map.insert("bedrock".to_string(), TEX_BEDROCK);
        texture_map.insert("sand".to_string(), TEX_SAND);
        texture_map.insert("gravel".to_string(), TEX_GRAVEL);
        texture_map.insert("log_side".to_string(), TEX_LOG_SIDE);
        texture_map.insert("log_top".to_string(), TEX_LOG_TOP);
        texture_map.insert("leaves".to_string(), TEX_LEAVES);
        texture_map.insert("glass".to_string(), TEX_GLASS);
        texture_map.insert("water".to_string(), TEX_WATER);

        Self {
            width,
            height,
            tile_size,
            tiles_per_row,
            texture_map,
        }
    }

    /// Computes UV coordinates [u, v] by texture_id and corner index (0..3).
    /// - 0: Top-Left (U0, V0)
    /// - 1: Top-Right (U1, V0)
    /// - 2: Bottom-Right (U1, V1)
    /// - 3: Bottom-Left (U0, V1)
    pub fn get_uv(&self, texture_id: u16, corner: u8) -> [f32; 2] {
        let col = (texture_id as u32) % self.tiles_per_row;
        let row = (texture_id as u32) / self.tiles_per_row;

        let u0 = (col * self.tile_size) as f32 / self.width as f32;
        let v0 = (row * self.tile_size) as f32 / self.height as f32;
        let u1 = ((col + 1) * self.tile_size) as f32 / self.width as f32;
        let v1 = ((row + 1) * self.tile_size) as f32 / self.height as f32;

        match corner {
            0 => [u0, v0],
            1 => [u1, v0],
            2 => [u1, v1],
            3 => [u0, v1],
            _ => [u0, v0],
        }
    }

    /// Returns the texture ID corresponding to the Minecraft 1.12.2 block ID and face (0..5: Down, Up, North, South, West, East).
    pub fn get_block_texture_id(&self, block_id: u16, _meta: u8, face: u8) -> u16 {
        match block_id {
            1 => TEX_STONE,
            2 => {
                // Grass block
                if face == 1 {
                    TEX_GRASS_TOP // Up
                } else if face == 0 {
                    TEX_DIRT // Down
                } else {
                    TEX_GRASS_SIDE // North, South, West, East
                }
            }
            3 => TEX_DIRT,
            4 => TEX_COBBLESTONE,
            5 => TEX_PLANKS,
            7 => TEX_BEDROCK,
            8 | 9 => TEX_WATER,
            12 => TEX_SAND,
            13 => TEX_GRAVEL,
            17 => {
                if face == 0 || face == 1 {
                    TEX_LOG_TOP
                } else {
                    TEX_LOG_SIDE
                }
            }
            18 => TEX_LEAVES,
            20 => TEX_GLASS,
            _ => TEX_DIRT,
        }
    }

    /// Checks if a block state is transparent for culling and alpha blending.
    pub fn is_transparent(block_id: u16) -> bool {
        block_id == 0 || block_id == 8 || block_id == 9 || block_id == 18 || block_id == 20
    }
}

/// Generates RGBA8 pixel buffer for procedural fallback TextureAtlas (512x512, 16x16 tiles).
pub fn generate_procedural_atlas() -> (Vec<u8>, TextureAtlas) {
    let width = 512u32;
    let height = 512u32;
    let tile_size = 16u32;
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let atlas = TextureAtlas::new(width, height, tile_size);

    // Color definitions for each procedural tile
    let color_defs: [(u16, [u8; 4], [u8; 4]); 14] = [
        (TEX_MISSING, [255, 0, 255, 255], [0, 0, 0, 255]), // Magenta/Black checker
        (TEX_STONE, [125, 125, 125, 255], [105, 105, 105, 255]), // Stone gray noise
        (TEX_GRASS_TOP, [92, 160, 46, 255], [76, 140, 36, 255]), // Grass green
        (TEX_GRASS_SIDE, [134, 96, 67, 255], [92, 160, 46, 255]), // Grass side
        (TEX_DIRT, [134, 96, 67, 255], [115, 80, 52, 255]), // Dirt brown
        (TEX_COBBLESTONE, [100, 100, 100, 255], [70, 70, 70, 255]), // Cobble dark gray
        (TEX_PLANKS, [162, 130, 78, 255], [140, 110, 60, 255]), // Oak planks
        (TEX_BEDROCK, [50, 50, 50, 255], [20, 20, 20, 255]), // Bedrock dark
        (TEX_SAND, [219, 207, 153, 255], [200, 185, 130, 255]), // Sand yellow
        (TEX_GRAVEL, [136, 126, 126, 255], [110, 100, 100, 255]), // Gravel gray-brown
        (TEX_LOG_SIDE, [103, 82, 49, 255], [60, 45, 25, 255]), // Log bark
        (TEX_LOG_TOP, [162, 130, 78, 255], [103, 82, 49, 255]), // Log rings
        (TEX_LEAVES, [50, 120, 20, 220], [30, 90, 15, 220]), // Leaves cutout green
        (TEX_WATER, [47, 67, 244, 180], [35, 55, 200, 180]), // Water translucent blue
    ];

    for &(id, c1, c2) in &color_defs {
        let col = (id as u32) % atlas.tiles_per_row;
        let row = (id as u32) / atlas.tiles_per_row;
        let start_x = col * tile_size;
        let start_y = row * tile_size;

        for ty in 0..tile_size {
            for tx in 0..tile_size {
                let px = start_x + tx;
                let py = start_y + ty;
                let idx = ((py * width + px) * 4) as usize;

                let use_alt = ((tx ^ ty) & 1) == 0;
                let color = if use_alt { c1 } else { c2 };

                pixels[idx] = color[0];
                pixels[idx + 1] = color[1];
                pixels[idx + 2] = color[2];
                pixels[idx + 3] = color[3];
            }
        }
    }

    (pixels, atlas)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_texture_atlas_uv_coordinates() {
        let atlas = TextureAtlas::new(512, 512, 16);
        // Stone o texture_id = 1
        // tiles_per_row = 32
        // col = 1, row = 0
        // u0 = 16 / 512 = 0.03125, v0 = 0.0
        // u1 = 32 / 512 = 0.0625, v1 = 16 / 512 = 0.03125
        let uv0 = atlas.get_uv(TEX_STONE, 0);
        let uv2 = atlas.get_uv(TEX_STONE, 2);
        assert!((uv0[0] - 0.03125).abs() < 1e-5);
        assert!((uv0[1] - 0.0).abs() < 1e-5);
        assert!((uv2[0] - 0.0625).abs() < 1e-5);
        assert!((uv2[1] - 0.03125).abs() < 1e-5);
    }

    #[test]
    fn test_procedural_atlas_generation() {
        let (pixels, atlas) = generate_procedural_atlas();
        assert_eq!(pixels.len(), (512 * 512 * 4) as usize);
        assert_eq!(atlas.width, 512);
        assert_eq!(atlas.height, 512);

        // Verify first pixel of stone tile (px = 16, py = 0)
        let stone_idx = (16 * 4) as usize;
        let r = pixels[stone_idx];
        assert!(r == 125 || r == 105);
    }
}
