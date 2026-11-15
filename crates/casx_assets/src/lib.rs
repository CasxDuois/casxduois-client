//! Crate `casx-assets`: Trinh quan ly tai nguyen TextureAtlas, mo hinh khoi va font chu Unicode.

pub mod atlas;

pub use atlas::{generate_procedural_atlas, TextureAtlas, TEX_STONE, TEX_GRASS_TOP, TEX_GRASS_SIDE, TEX_DIRT, TEX_COBBLESTONE, TEX_PLANKS, TEX_BEDROCK};
