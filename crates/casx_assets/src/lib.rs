//! Crate `casx-assets`: Trinh quan ly tai nguyen TextureAtlas, mo hinh khoi va font chu Unicode.

pub mod atlas;
pub mod font;
pub mod custom_model;
pub mod bootstrapper;

pub use atlas::{
    generate_procedural_atlas, TextureAtlas, TEX_BEDROCK, TEX_COBBLESTONE, TEX_DIRT, TEX_GLASS,
    TEX_GRASS_SIDE, TEX_GRASS_TOP, TEX_GRAVEL, TEX_LEAVES, TEX_LOG_SIDE, TEX_LOG_TOP,
    TEX_MISSING, TEX_PLANKS, TEX_SAND, TEX_STONE, TEX_WATER,
};
pub use bootstrapper::{AssetBootstrapper, AssetError};
pub use font::{FontProvider, GlyphMetrics};
pub use custom_model::{CustomModelRegistry, ItemModelDefinition, ModelOverride};
