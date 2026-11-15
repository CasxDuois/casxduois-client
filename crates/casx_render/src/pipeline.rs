//! 5-pass wgpu rendering pipeline (Sky, Solid Terrain, Entity Outline, Translucent, Native UI).
//!
//! Pass structure:
//! - Pass 1: Sky Pass (Sky color and fog based on world time).
//! - Pass 2: Solid & Cutout Terrain Pass (Depth32Float, Back-face Culling, Frustum culling, Indirect Batching).
//! - Pass 3: Entity & Block Outline Pass (Wireframe outline for targeted block via 4.5m DDA Raycast).
//! - Pass 4: Translucent Terrain Pass (Alpha Blending, Depth Write = false for water and stained glass).
//! - Pass 5: Native UI Pass (HUD, Hotbar, Chat, Scoreboard, Launcher overlay).

use crate::frustum::Frustum;
use crate::indirect::IndirectBatchManager;
use casx_core::physics::Aabb;

/// Pass 1 configuration: Sky and fog parameters.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkyPassConfig {
    pub sky_color: [f32; 4],
    pub fog_color: [f32; 4],
    pub fog_start: f32,
    pub fog_end: f32,
}

impl Default for SkyPassConfig {
    fn default() -> Self {
        Self {
            sky_color: [0.47, 0.65, 1.0, 1.0], // Daytime blue
            fog_color: [0.75, 0.85, 1.0, 1.0],
            fog_start: 48.0,
            fog_end: 128.0,
        }
    }
}

impl SkyPassConfig {
    /// Computes sky and fog color according to server world time (0..24000 ticks)
    pub fn update_from_world_time(&mut self, world_time: i64) {
        let day_time = (world_time % 24000) as f32;

        if (1000.0..11000.0).contains(&day_time) {
            // Daytime
            self.sky_color = [0.47, 0.65, 1.0, 1.0];
            self.fog_color = [0.75, 0.85, 1.0, 1.0];
        } else if (11000.0..13000.0).contains(&day_time) {
            // Sunset / Sunrise
            self.sky_color = [0.85, 0.45, 0.2, 1.0];
            self.fog_color = [0.65, 0.35, 0.25, 1.0];
        } else {
            // Nighttime
            self.sky_color = [0.03, 0.03, 0.08, 1.0];
            self.fog_color = [0.05, 0.05, 0.1, 1.0];
        }
    }
}

/// Pass 2 configuration: Solid and cutout terrain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SolidTerrainPassConfig {
    pub enable_depth_test: bool,
    pub enable_depth_write: bool,
    pub enable_backface_culling: bool,
}

impl Default for SolidTerrainPassConfig {
    fn default() -> Self {
        Self {
            enable_depth_test: true,
            enable_depth_write: true,
            enable_backface_culling: true,
        }
    }
}

/// Pass 3 configuration: Targeted block outline and entities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityOutlinePassConfig {
    pub wireframe_color: [f32; 4],
    pub reach_distance: f32,
    pub active_outline_box: Option<Aabb>,
}

impl Default for EntityOutlinePassConfig {
    fn default() -> Self {
        Self {
            wireframe_color: [0.0, 0.0, 0.0, 0.6],
            reach_distance: 4.5,
            active_outline_box: None,
        }
    }
}

/// Pass 4 configuration: Translucent terrain (Water, stained glass).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranslucentPassConfig {
    pub enable_alpha_blending: bool,
    pub enable_depth_write: bool,
}

impl Default for TranslucentPassConfig {
    fn default() -> Self {
        Self {
            enable_alpha_blending: true,
            enable_depth_write: false,
        }
    }
}

/// Pass 5 configuration: 2D Native UI overlay pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeUiPassConfig {
    pub render_hud: bool,
    pub render_crosshair: bool,
}

impl Default for NativeUiPassConfig {
    fn default() -> Self {
        Self {
            render_hud: true,
            render_crosshair: true,
        }
    }
}

/// 5-pass rendering pipeline manager.
#[derive(Debug, Default, Clone)]
pub struct RenderPipeline5Pass {
    pub pass1_sky: SkyPassConfig,
    pub pass2_solid: SolidTerrainPassConfig,
    pub pass3_outline: EntityOutlinePassConfig,
    pub pass4_translucent: TranslucentPassConfig,
    pub pass5_ui: NativeUiPassConfig,
    pub indirect_batch: IndirectBatchManager,
}

impl RenderPipeline5Pass {
    pub fn new() -> Self {
        Self {
            pass1_sky: SkyPassConfig::default(),
            pass2_solid: SolidTerrainPassConfig::default(),
            pass3_outline: EntityOutlinePassConfig::default(),
            pass4_translucent: TranslucentPassConfig::default(),
            pass5_ui: NativeUiPassConfig::default(),
            indirect_batch: IndirectBatchManager::new(),
        }
    }

    /// Sets the bounding box for the targeted block.
    pub fn set_targeted_block_outline(&mut self, aabb: Option<Aabb>) {
        self.pass3_outline.active_outline_box = aabb;
    }

    /// Updates time of day for sky and fog rendering.
    pub fn update_time_of_day(&mut self, time_ticks: i64) {
        self.pass1_sky.update_from_world_time(time_ticks);
    }

    /// Checks if a chunk section is visible within the frustum for Pass 2.
    pub fn is_section_visible(&self, frustum: &Frustum, section_aabb: &Aabb) -> bool {
        frustum.intersects_aabb(section_aabb)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::DVec3;

    #[test]
    fn test_render_pipeline_5_pass_configuration_and_time_update() {
        let mut pipeline = RenderPipeline5Pass::new();

        pipeline.update_time_of_day(6000);
        assert_eq!(pipeline.pass1_sky.sky_color[0], 0.47);

        pipeline.update_time_of_day(18000);
        assert_eq!(pipeline.pass1_sky.sky_color[0], 0.03);

        pipeline.update_time_of_day(12000);
        assert_eq!(pipeline.pass1_sky.sky_color[0], 0.85);

        assert!(!pipeline.pass4_translucent.enable_depth_write);
        assert!(pipeline.pass4_translucent.enable_alpha_blending);

        let box_target = Aabb::new(DVec3::new(1.0, 64.0, 2.0), DVec3::new(2.0, 65.0, 3.0));
        pipeline.set_targeted_block_outline(Some(box_target));
        assert!(pipeline.pass3_outline.active_outline_box.is_some());
    }
}
