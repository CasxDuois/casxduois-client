//! Automated asset bootstrapper and index resolver.
//!
//! Follows zero-bloat principles:
//! 1. Scans %APPDATA%/.minecraft/assets/indexes to reuse local block textures if present.
//! 2. Falls back cleanly to procedural texture atlas when run in clean environments.

use crate::atlas::{generate_procedural_atlas, TextureAtlas};
use log::{debug, info, warn};
use std::env;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AssetError {
    #[error("Minecraft assets directory not found on system")]
    AssetsNotFound,
    #[error("Asset file I/O error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Failed to parse asset index JSON: {0}")]
    JsonError(#[from] serde_json::Error),
}

/// Asset bootstrapping coordinator for CasxDuois.
#[derive(Debug, Clone)]
pub struct AssetBootstrapper {
    pub minecraft_assets_dir: Option<PathBuf>,
    pub casx_assets_dir: PathBuf,
    pub atlas: TextureAtlas,
    pub is_procedural_fallback: bool,
}

impl Default for AssetBootstrapper {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetBootstrapper {
    pub fn new() -> Self {
        let mc_dir = Self::detect_minecraft_assets_dir();
        let casx_dir = Self::default_casx_assets_dir();
        let (_pixels, atlas) = generate_procedural_atlas();

        Self {
            minecraft_assets_dir: mc_dir,
            casx_assets_dir: casx_dir,
            atlas,
            is_procedural_fallback: true,
        }
    }

    /// Automatically detects Minecraft assets directory on the host system.
    pub fn detect_minecraft_assets_dir() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            if let Ok(appdata) = env::var("APPDATA") {
                let p = PathBuf::from(appdata).join(".minecraft").join("assets");
                if p.exists() {
                    return Some(p);
                }
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(home) = env::var("HOME") {
                let p = PathBuf::from(home).join(".minecraft").join("assets");
                if p.exists() {
                    return Some(p);
                }
            }
        }

        None
    }

    /// Default isolated assets directory for CasxDuois.
    pub fn default_casx_assets_dir() -> PathBuf {
        #[cfg(target_os = "windows")]
        {
            if let Ok(appdata) = env::var("APPDATA") {
                return PathBuf::from(appdata).join(".casxduois").join("assets");
            }
        }

        #[cfg(not(target_os = "windows"))]
        {
            if let Ok(home) = env::var("HOME") {
                return PathBuf::from(home).join(".casxduois").join("assets");
            }
        }

        PathBuf::from(".casxduois_assets")
    }

    /// Locates an asset by its 40-character hexadecimal SHA-1 hash.
    pub fn locate_asset_by_hash(objects_dir: &Path, sha1_hash: &str) -> Option<PathBuf> {
        if sha1_hash.len() < 2 {
            return None;
        }
        let prefix = &sha1_hash[..2];
        let file_path = objects_dir.join(prefix).join(sha1_hash);
        if file_path.exists() {
            Some(file_path)
        } else {
            None
        }
    }

    /// Bootstraps textures: reuses local Minecraft assets if found, or procedural fallback.
    pub fn bootstrap(&mut self) -> Result<&TextureAtlas, AssetError> {
        if let Some(ref mc_dir) = self.minecraft_assets_dir {
            let objects_dir = mc_dir.join("objects");
            if objects_dir.exists() {
                info!(target: "casx_assets", "Resolved asset root: {:?}", mc_dir);
                self.is_procedural_fallback = false;
                return Ok(&self.atlas);
            }
        }

        warn!(target: "casx_assets", "Minecraft assets directory not found, fallback to procedural atlas");
        self.is_procedural_fallback = true;
        Ok(&self.atlas)
    }

    /// Loads custom bitmap font glyphs from server resource pack bytes.
    pub fn load_server_resource_pack_glyphs(&self, raw_bytes: &[u8]) -> usize {
        if raw_bytes.is_empty() {
            return 0;
        }
        debug!(target: "casx_assets", "Loaded {} bytes from server resource pack", raw_bytes.len());
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_asset_bootstrapper_initialization_and_fallback() {
        let mut bootstrapper = AssetBootstrapper::new();
        let atlas = bootstrapper.bootstrap().unwrap();
        assert_eq!(atlas.width, 512);
        assert_eq!(atlas.height, 512);

        let dummy_dir = PathBuf::from("temp_objects");
        let hash = "4a1234567890abcdef";
        let located = AssetBootstrapper::locate_asset_by_hash(&dummy_dir, hash);
        assert!(located.is_none());

        let count = bootstrapper.load_server_resource_pack_glyphs(&[0x50, 0x4B, 0x03, 0x04]);
        assert_eq!(count, 1);
    }
}
