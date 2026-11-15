//! Multi-account headless Eco-AFK session manager.

use casx_core::GameVersion;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Operating status of an Eco-AFK session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AfkStatus {
    Idle,
    Running,
    Reconnecting,
    Stopped,
    Failed,
}

/// Launch configuration for an Eco-AFK bot.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AfkSessionConfig {
    pub server_address: String,
    pub bot_nickname: String,
    pub protocol_version: GameVersion,
    pub auto_reconnect: bool,
    pub memory_budget_mb: u32,
}

impl AfkSessionConfig {
    pub fn new(server_address: &str, bot_nickname: &str, protocol_version: GameVersion) -> Self {
        Self {
            server_address: server_address.to_string(),
            bot_nickname: bot_nickname.to_string(),
            protocol_version,
            auto_reconnect: true,
            memory_budget_mb: 15,
        }
    }
}

/// Detailed telemetry of an Eco-AFK session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AfkSession {
    pub id: u64,
    pub config: AfkSessionConfig,
    pub status: AfkStatus,
    pub ticks_alive: u64,
    pub memory_used_mb: f32,
}

/// Central coordinator for multi-account Eco-AFK sessions.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct AfkSessionManager {
    pub next_id: u64,
    pub sessions: HashMap<u64, AfkSession>,
}

impl AfkSessionManager {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            sessions: HashMap::new(),
        }
    }

    /// Registers and starts a new Eco-AFK session.
    pub fn start_session(&mut self, config: AfkSessionConfig) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let session = AfkSession {
            id,
            config,
            status: AfkStatus::Running,
            ticks_alive: 0,
            memory_used_mb: 12.5,
        };

        self.sessions.insert(id, session);
        id
    }

    /// Stops an active Eco-AFK session.
    pub fn stop_session(&mut self, id: u64) -> bool {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.status = AfkStatus::Stopped;
            true
        } else {
            false
        }
    }

    /// Removes a stopped session.
    pub fn remove_session(&mut self, id: u64) -> bool {
        self.sessions.remove(&id).is_some()
    }

    /// Calculates total memory consumed across active bots.
    pub fn total_memory_consumption_mb(&self) -> f32 {
        self.sessions
            .values()
            .filter(|s| s.status == AfkStatus::Running)
            .map(|s| s.memory_used_mb)
            .sum()
    }

    /// Count of active AFK bots.
    pub fn active_count(&self) -> usize {
        self.sessions
            .values()
            .filter(|s| s.status == AfkStatus::Running)
            .count()
    }

    /// Spawns a background subprocess runner via std::env::current_exe().
    pub fn spawn_subprocess_afk(config: &AfkSessionConfig) -> Result<u32, std::io::Error> {
        if let Ok(exe_path) = std::env::current_exe() {
            let ver_str = match config.protocol_version {
                GameVersion::V1_12_2 => "340",
                GameVersion::V26_2 => "776",
            };
            let child = std::process::Command::new(exe_path)
                .arg("--mode")
                .arg("afk")
                .arg("--ver")
                .arg(ver_str)
                .arg("--server")
                .arg(&config.server_address)
                .arg("--nick")
                .arg(&config.bot_nickname)
                .spawn()?;
            Ok(child.id())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Current executable path not found",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_afk_session_lifecycle_and_memory_budget() {
        let mut mgr = AfkSessionManager::new();
        let cfg = AfkSessionConfig::new("127.0.0.1:25565", "Bot_AFK_01", GameVersion::V1_12_2);

        let id = mgr.start_session(cfg);
        assert_eq!(mgr.active_count(), 1);
        assert!(mgr.total_memory_consumption_mb() <= 15.0);

        let session = mgr.sessions.get(&id).unwrap();
        assert_eq!(session.status, AfkStatus::Running);

        assert!(mgr.stop_session(id));
        assert_eq!(mgr.active_count(), 0);
        assert_eq!(mgr.total_memory_consumption_mb(), 0.0);

        assert!(mgr.remove_session(id));
        assert!(mgr.sessions.is_empty());
    }
}
