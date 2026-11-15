//! Duong ong do hoa wgpu 5 Pass (Sky, Solid Terrain, Entity Outline, Translucent, Native UI).
//!
//! Tuan thu chat che dac ta Section 3.2.C trong Tai Lieu Doi Chieu Tong The:
//! - Pass 1: Sky Pass (Mau troi va suong mu theo gio server).
//! - Pass 2: Solid & Cutout Terrain Pass (Depth32Float, Back-face Culling, Frustum culling, Indirect Batching).
//! - Pass 3: Entity & Block Outline Pass (Khung day den quanh khoi qua DDA Raycast 4.5m).
//! - Pass 4: Translucent Terrain Pass (Alpha Blending, Depth Write = false cho nuoc va kinh).
//! - Pass 5: Native UI Pass (HUD, Hotbar, Chat, Scoreboard, HoYoPlay Hub).

use crate::frustum::Frustum;
use crate::indirect::IndirectBatchManager;
use casx_core::physics::Aabb;

/// Thong so Pass 1: Bau troi va suong mu
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
            sky_color: [0.47, 0.65, 1.0, 1.0], // Xanh ban ngay
            fog_color: [0.75, 0.85, 1.0, 1.0],
            fog_start: 48.0,
            fog_end: 128.0,
        }
    }
}

impl SkyPassConfig {
    /// Tinh toan mau bau troi va suong mu dua theo thoi gian the gioi server (0..24000 ticks)
    pub fn update_from_world_time(&mut self, world_time: i64) {
        let day_time = (world_time % 24000) as f32;

        if (1000.0..11000.0).contains(&day_time) {
            // Ban ngay
            self.sky_color = [0.47, 0.65, 1.0, 1.0];
            self.fog_color = [0.75, 0.85, 1.0, 1.0];
        } else if (11000.0..13000.0).contains(&day_time) {
            // Hoang hon / Binh minh
            self.sky_color = [0.85, 0.45, 0.2, 1.0];
            self.fog_color = [0.65, 0.35, 0.25, 1.0];
        } else {
            // Ban dem
            self.sky_color = [0.03, 0.03, 0.08, 1.0];
            self.fog_color = [0.05, 0.05, 0.1, 1.0];
        }
    }
}

/// Thong so Pass 2: Dia hinh dac va cutout
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

/// Thong so Pass 3: Vien khoi va thuc the
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityOutlinePassConfig {
    pub wireframe_color: [f32; 4],
    pub reach_distance: f32,
    pub active_outline_box: Option<Aabb>,
}

impl Default for EntityOutlinePassConfig {
    fn default() -> Self {
        Self {
            wireframe_color: [0.0, 0.0, 0.0, 0.6], // Vien den mo 60%
            reach_distance: 4.5,
            active_outline_box: None,
        }
    }
}

/// Thong so Pass 4: Dia hinh ban trong suot (Nuoc, Kinh mau)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TranslucentPassConfig {
    pub enable_alpha_blending: bool,
    pub enable_depth_write: bool,
}

impl Default for TranslucentPassConfig {
    fn default() -> Self {
        Self {
            enable_alpha_blending: true,
            enable_depth_write: false, // Tat depth write de khong che lap nuoc
        }
    }
}

/// Thong so Pass 5: Giao dien nguoi dung Native UI
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

/// Bo dieu phoi toan bo duong ong do hoa 5 Pass cua CasxDuois Engine
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

    /// Thiet lap hop vien cho khoi dang duoc chi toi boi DDA Raycast
    pub fn set_targeted_block_outline(&mut self, aabb: Option<Aabb>) {
        self.pass3_outline.active_outline_box = aabb;
    }

    /// Cap nhat thoi gian the gioi
    pub fn update_time_of_day(&mut self, time_ticks: i64) {
        self.pass1_sky.update_from_world_time(time_ticks);
    }

    /// Kiem tra mot Section co nam trong goc nhin Frustum de dua vao Pass 2 khong
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

        // Ban ngay (6000 ticks)
        pipeline.update_time_of_day(6000);
        assert_eq!(pipeline.pass1_sky.sky_color[0], 0.47);

        // Ban dem (18000 ticks)
        pipeline.update_time_of_day(18000);
        assert_eq!(pipeline.pass1_sky.sky_color[0], 0.03);

        // Hoang hon (12000 ticks)
        pipeline.update_time_of_day(12000);
        assert_eq!(pipeline.pass1_sky.sky_color[0], 0.85);

        // Kiem tra cau hinh Depth Write cua Pass 4 (Translucent phai tat depth write)
        assert!(!pipeline.pass4_translucent.enable_depth_write);
        assert!(pipeline.pass4_translucent.enable_alpha_blending);

        // Kiem tra Outline Box
        let box_target = Aabb::new(DVec3::new(1.0, 64.0, 2.0), DVec3::new(2.0, 65.0, 3.0));
        pipeline.set_targeted_block_outline(Some(box_target));
        assert!(pipeline.pass3_outline.active_outline_box.is_some());
    }
}
