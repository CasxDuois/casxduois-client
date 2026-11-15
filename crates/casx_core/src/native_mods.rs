//! Native Rust mods system for CasxDuois client engine.

use glam::DVec3;

/// Context passed into native mods each frame or tick
pub struct HookContext<'a> {
    pub player_pos: DVec3,
    pub server_yaw: f32,
    pub server_pitch: f32,
    pub render_yaw: &'a mut f32,
    pub render_pitch: &'a mut f32,
    pub render_fov: &'a mut f32,
    pub is_alt_held: bool,
    pub is_zoom_held: bool,
}

/// Standard trait for native Rust mods
pub trait CasxNativeMod: Send + Sync {
    fn mod_id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn is_enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);

    /// Hook intervening in camera rotation and FOV before View/Projection matrix calculation
    fn on_camera_update(&mut self, _ctx: &mut HookContext<'_>) {}
}

/// Mod 1: Freelook 360 (Hold Alt to rotate camera freely without changing server view angles)
pub struct FreelookMod {
    pub enabled: bool,
    pub free_yaw: f32,
    pub free_pitch: f32,
    was_active: bool,
}

impl Default for FreelookMod {
    fn default() -> Self {
        Self {
            enabled: true,
            free_yaw: 0.0,
            free_pitch: 0.0,
            was_active: false,
        }
    }
}

impl CasxNativeMod for FreelookMod {
    fn mod_id(&self) -> &'static str {
        "freelook"
    }

    fn display_name(&self) -> &'static str {
        "Freelook 360 (Hold Alt)"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn on_camera_update(&mut self, ctx: &mut HookContext<'_>) {
        if !self.enabled {
            return;
        }
        if ctx.is_alt_held {
            if !self.was_active {
                self.free_yaw = ctx.server_yaw;
                self.free_pitch = ctx.server_pitch;
                self.was_active = true;
            }
            *ctx.render_yaw = self.free_yaw;
            *ctx.render_pitch = self.free_pitch.clamp(-90.0, 90.0);
        } else {
            self.was_active = false;
            *ctx.render_yaw = ctx.server_yaw;
            *ctx.render_pitch = ctx.server_pitch;
        }
    }
}

/// Mod 2: Smooth Zoom (Smooth FOV interpolation when holding C)
pub struct SmoothZoomMod {
    pub enabled: bool,
    pub target_zoom_fov: f32,
    current_multiplier: f32,
}

impl Default for SmoothZoomMod {
    fn default() -> Self {
        Self {
            enabled: true,
            target_zoom_fov: 18.0,
            current_multiplier: 1.0,
        }
    }
}

impl CasxNativeMod for SmoothZoomMod {
    fn mod_id(&self) -> &'static str {
        "smooth_zoom"
    }

    fn display_name(&self) -> &'static str {
        "Smooth Zoom (Hold C)"
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    fn on_camera_update(&mut self, ctx: &mut HookContext<'_>) {
        if !self.enabled {
            return;
        }
        let base_fov = *ctx.render_fov;
        let goal = if ctx.is_zoom_held {
            self.target_zoom_fov / base_fov.max(1.0)
        } else {
            1.0
        };
        // Exponential smoothing interpolation
        self.current_multiplier += (goal - self.current_multiplier) * 0.25;
        *ctx.render_fov = base_fov * self.current_multiplier;
    }
}

/// Central manager for native Rust mods
pub struct ModManager {
    mods: Vec<Box<dyn CasxNativeMod>>,
}

impl Default for ModManager {
    fn default() -> Self {
        let mut mgr = Self { mods: Vec::new() };
        mgr.register(Box::new(FreelookMod::default()));
        mgr.register(Box::new(SmoothZoomMod::default()));
        mgr
    }
}

impl ModManager {
    pub fn register(&mut self, m: Box<dyn CasxNativeMod>) {
        self.mods.push(m);
    }

    pub fn set_mod_enabled(&mut self, mod_id: &str, enabled: bool) -> bool {
        for m in &mut self.mods {
            if m.mod_id() == mod_id {
                m.set_enabled(enabled);
                return true;
            }
        }
        false
    }

    pub fn dispatch_camera_update(&mut self, ctx: &mut HookContext<'_>) {
        for m in &mut self.mods {
            m.on_camera_update(ctx);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_freelook_mod_rotates_camera_independently() {
        let mut freelook = FreelookMod::default();
        let mut render_yaw = 45.0f32;
        let mut render_pitch = 10.0f32;
        let mut render_fov = 70.0f32;

        freelook.free_yaw = 180.0;
        freelook.free_pitch = -30.0;

        // When Alt is held, render_yaw equals free_yaw while server_yaw = 0.0
        let mut ctx = HookContext {
            player_pos: DVec3::ZERO,
            server_yaw: 0.0,
            server_pitch: 0.0,
            render_yaw: &mut render_yaw,
            render_pitch: &mut render_pitch,
            render_fov: &mut render_fov,
            is_alt_held: true,
            is_zoom_held: false,
        };

        freelook.on_camera_update(&mut ctx);
        assert_eq!(*ctx.render_yaw, 0.0); // Initial Alt activation syncs free_yaw to server_yaw

        // After activation with camera rotation:
        freelook.free_yaw = 120.0;
        freelook.on_camera_update(&mut ctx);
        assert_eq!(*ctx.render_yaw, 120.0);
        assert_eq!(ctx.server_yaw, 0.0); // server_yaw remains unchanged
    }

    #[test]
    fn test_smooth_zoom_mod_reduces_fov() {
        let mut zoom = SmoothZoomMod::default();
        let mut render_yaw = 0.0f32;
        let mut render_pitch = 0.0f32;
        let mut render_fov = 90.0f32;

        let mut ctx = HookContext {
            player_pos: DVec3::ZERO,
            server_yaw: 0.0,
            server_pitch: 0.0,
            render_yaw: &mut render_yaw,
            render_pitch: &mut render_pitch,
            render_fov: &mut render_fov,
            is_alt_held: false,
            is_zoom_held: true,
        };

        // Holding Zoom C narrows FOV smoothly across frames
        zoom.on_camera_update(&mut ctx);
        assert!(
            *ctx.render_fov < 90.0,
            "FOV must decrease when zoom is active"
        );
    }
}
