//! Trang thai tich hop cua Unified Launcher (HoYoPlay Hub x Legacy Quick-Switch x Lunar Client Modules).

use crate::afk_manager::{AfkSessionConfig, AfkSessionManager};
use crate::mods::ModConfig;
use crate::profile::{PlayerProfile, ProfileManager};
use crate::server_list::ServerListManager;
use casx_core::GameVersion;

/// Hanh dong khoi chay duoc phat ra tu Launcher
#[derive(Debug, Clone, PartialEq)]
pub enum LaunchTarget {
    NormalClient {
        host: String,
        port: u16,
        username: String,
        version: GameVersion,
    },
    EcoAfk {
        host: String,
        port: u16,
        username: String,
        version: GameVersion,
    },
}

/// Che do hien thi cua Launcher
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LauncherTab {
    Hub,
    ServerBrowser,
    ModSettings,
    AfkManager,
}

/// Toan bo trang thai cua Launcher duoc luu tru tap trung
#[derive(Debug, Clone)]
pub struct LauncherState {
    pub current_tab: LauncherTab,
    pub profile_manager: ProfileManager,
    pub server_list: ServerListManager,
    pub mod_config: ModConfig,
    pub afk_manager: AfkSessionManager,
    pub target_server_input: String,
    pub is_eco_afk_selected: bool,
}

impl Default for LauncherState {
    fn default() -> Self {
        let profile_mgr = ProfileManager::default();
        let server_list = ServerListManager::default();
        let default_addr = server_list
            .get_selected_server()
            .map(|s| s.address_string())
            .unwrap_or_else(|| "127.0.0.1:25565".to_string());

        Self {
            current_tab: LauncherTab::Hub,
            profile_manager: profile_mgr,
            server_list,
            mod_config: ModConfig::default(),
            afk_manager: AfkSessionManager::default(),
            target_server_input: default_addr,
            is_eco_afk_selected: false,
        }
    }
}

impl LauncherState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Chuyen doi phien ban dang chon giua 1.12.2 va 26.2
    pub fn cycle_version(&mut self) -> GameVersion {
        if let Some(prof) = self.profile_manager.get_active_profile_mut() {
            prof.selected_version = match prof.selected_version {
                GameVersion::V1_12_2 => GameVersion::V26_2,
                GameVersion::V26_2 => GameVersion::V1_12_2,
            };
            prof.selected_version
        } else {
            GameVersion::V1_12_2
        }
    }

    /// Tao lenh khoi chay game hoac khoi chay che do Eco-AFK
    pub fn build_launch_target(&self) -> LaunchTarget {
        let parts: Vec<&str> = self.target_server_input.split(':').collect();
        let host = parts.first().unwrap_or(&"127.0.0.1").trim().to_string();
        let port = parts.get(1).and_then(|p| p.trim().parse::<u16>().ok()).unwrap_or(25565);

        let active_prof = self.profile_manager.get_active_profile().cloned().unwrap_or_else(|| {
            PlayerProfile::new_offline("CasxPlayer")
        });

        if self.is_eco_afk_selected {
            LaunchTarget::EcoAfk {
                host,
                port,
                username: active_prof.username,
                version: active_prof.selected_version,
            }
        } else {
            LaunchTarget::NormalClient {
                host,
                port,
                username: active_prof.username,
                version: active_prof.selected_version,
            }
        }
    }

    /// Khoi chay mot bot AFK moi tu dia chi va ho so dang chon
    pub fn spawn_afk_bot_from_current_selection(&mut self) -> u64 {
        let target = self.build_launch_target();
        match target {
            LaunchTarget::EcoAfk { host, port, username, version }
            | LaunchTarget::NormalClient { host, port, username, version } => {
                let addr = format!("{}:{}", host, port);
                let cfg = AfkSessionConfig::new(&addr, &format!("{}_AFK", username), version);
                self.afk_manager.start_session(cfg)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_launcher_state_and_launch_targets() {
        let mut state = LauncherState::new();
        assert_eq!(state.current_tab, LauncherTab::Hub);

        let target = state.build_launch_target();
        match target {
            LaunchTarget::NormalClient { host, port, username, version } => {
                assert_eq!(host, "aemine.net");
                assert_eq!(port, 25565);
                assert_eq!(username, "CasxPlayer");
                assert_eq!(version, GameVersion::V1_12_2);
            }
            _ => panic!("Expected NormalClient"),
        }

        // Chuyen sang che do Eco-AFK
        state.is_eco_afk_selected = true;
        let afk_target = state.build_launch_target();
        match afk_target {
            LaunchTarget::EcoAfk { username, .. } => {
                assert_eq!(username, "CasxPlayer");
            }
            _ => panic!("Expected EcoAfk"),
        }

        // Kiem tra chuyen version
        let new_ver = state.cycle_version();
        assert_eq!(new_ver, GameVersion::V26_2);
    }
}
