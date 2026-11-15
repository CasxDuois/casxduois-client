//! Native Rust mod configuration and toggles.

use serde::{Deserialize, Serialize};

/// Summary configuration for built-in client mods
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModConfig {
    pub toggle_sprint: bool,
    pub fullbright: bool,
    pub keystrokes_hud: bool,
    pub fps_ping_hud: bool,
    pub cps_hud: bool,
    pub memory_hud: bool,
    pub smooth_zoom: bool,
    pub freelook: bool,
    pub zoom_fov: f32,
}

impl Default for ModConfig {
    fn default() -> Self {
        Self {
            toggle_sprint: true,
            fullbright: false,
            keystrokes_hud: true,
            fps_ping_hud: true,
            cps_hud: true,
            memory_hud: true,
            smooth_zoom: true,
            freelook: true,
            zoom_fov: 18.0,
        }
    }
}

impl ModConfig {
    pub fn new() -> Self {
        Self::default()
    }

    /// Toggles a mod by identifier
    pub fn toggle_by_id(&mut self, mod_id: &str) -> bool {
        match mod_id {
            "toggle_sprint" => {
                self.toggle_sprint = !self.toggle_sprint;
                true
            }
            "fullbright" => {
                self.fullbright = !self.fullbright;
                true
            }
            "keystrokes_hud" => {
                self.keystrokes_hud = !self.keystrokes_hud;
                true
            }
            "fps_ping_hud" => {
                self.fps_ping_hud = !self.fps_ping_hud;
                true
            }
            "cps_hud" => {
                self.cps_hud = !self.cps_hud;
                true
            }
            "memory_hud" => {
                self.memory_hud = !self.memory_hud;
                true
            }
            "smooth_zoom" => {
                self.smooth_zoom = !self.smooth_zoom;
                true
            }
            "freelook" => {
                self.freelook = !self.freelook;
                true
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mod_config_toggles() {
        let mut cfg = ModConfig::default();
        assert!(cfg.toggle_sprint);

        assert!(cfg.toggle_by_id("toggle_sprint"));
        assert!(!cfg.toggle_sprint);

        assert!(cfg.toggle_by_id("fullbright"));
        assert!(cfg.fullbright);

        assert!(!cfg.toggle_by_id("unknown_mod"));
    }
}
