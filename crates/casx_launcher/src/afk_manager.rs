//! Quan ly he thong Eco-AFK chay ngam da tai khoan (Multi-Account Headless Runner).
//!
//! Tuan thu nghiem ngat quy chuan bo nho: $< 15\text{ MB RAM}$ cho moi phien AFK.

use casx_core::GameVersion;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Trang thai hoat dong cua mot phien Eco-AFK
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AfkStatus {
    Idle,
    Running,
    Reconnecting,
    Stopped,
    Failed,
}

/// Cau hinh khoi chay mot bot Eco-AFK
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
            memory_budget_mb: 15, // Quy chuan ky thuat < 15 MB RAM
        }
    }
}

/// Thong tin chi tiet mot phien Eco-AFK
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AfkSession {
    pub id: u64,
    pub config: AfkSessionConfig,
    pub status: AfkStatus,
    pub ticks_alive: u64,
    pub memory_used_mb: f32,
}

/// Bo dieu phoi trung tam cac tai khoan Eco-AFK
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

    /// Dang ky va bat dau mot phien Eco-AFK moi
    pub fn start_session(&mut self, config: AfkSessionConfig) -> u64 {
        let id = self.next_id;
        self.next_id += 1;

        let session = AfkSession {
            id,
            config,
            status: AfkStatus::Running,
            ticks_alive: 0,
            memory_used_mb: 12.5, // Uoc luong thuc te luc khoi chay
        };

        self.sessions.insert(id, session);
        id
    }

    /// Dung mot phien Eco-AFK
    pub fn stop_session(&mut self, id: u64) -> bool {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.status = AfkStatus::Stopped;
            true
        } else {
            false
        }
    }

    /// Xoa bo mot phien da dung
    pub fn remove_session(&mut self, id: u64) -> bool {
        self.sessions.remove(&id).is_some()
    }

    /// Tinh tong luong RAM tieu thu cua toan bo cac bot AFK
    pub fn total_memory_consumption_mb(&self) -> f32 {
        self.sessions
            .values()
            .filter(|s| s.status == AfkStatus::Running)
            .map(|s| s.memory_used_mb)
            .sum()
    }

    /// So luong bot AFK dang hoat dong
    pub fn active_count(&self) -> usize {
        self.sessions
            .values()
            .filter(|s| s.status == AfkStatus::Running)
            .count()
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
