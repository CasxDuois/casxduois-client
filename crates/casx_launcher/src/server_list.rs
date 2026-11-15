//! Server bookmarks management, ping caching, and protocol metadata.

use casx_core::GameVersion;
use serde::{Deserialize, Serialize};

/// Server entry in bookmarks directory
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ServerEntry {
    pub name: String,
    pub host: String,
    pub port: u16,
    pub protocol: GameVersion,
    pub ping_ms: Option<u32>,
    pub player_count: Option<u32>,
    pub max_players: Option<u32>,
    pub motd: Option<String>,
}

impl ServerEntry {
    pub fn new(name: &str, host: &str, port: u16, protocol: GameVersion) -> Self {
        Self {
            name: name.to_string(),
            host: host.to_string(),
            port,
            protocol,
            ping_ms: None,
            player_count: None,
            max_players: None,
            motd: None,
        }
    }

    /// Formats address string as `host:port`
    pub fn address_string(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

/// Server bookmarks list manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerListManager {
    pub servers: Vec<ServerEntry>,
    pub selected_index: usize,
}

impl Default for ServerListManager {
    fn default() -> Self {
        let default_servers = vec![
            ServerEntry::new(
                "Aemine Server 1.12.2",
                "aemine.net",
                25565,
                GameVersion::V1_12_2,
            ),
            ServerEntry::new(
                "Local Dev Server 26.2",
                "127.0.0.1",
                25565,
                GameVersion::V26_2,
            ),
        ];

        Self {
            servers: default_servers,
            selected_index: 0,
        }
    }
}

impl ServerListManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Retrieves currently selected server entry
    pub fn get_selected_server(&self) -> Option<&ServerEntry> {
        self.servers.get(self.selected_index)
    }

    /// Adds a server into bookmarks
    pub fn add_server(&mut self, entry: ServerEntry) {
        self.servers.push(entry);
    }

    /// Removes a server by index
    pub fn remove_server(&mut self, index: usize) -> bool {
        if index < self.servers.len() {
            self.servers.remove(index);
            if self.selected_index >= self.servers.len() && !self.servers.is_empty() {
                self.selected_index = self.servers.len() - 1;
            }
            true
        } else {
            false
        }
    }

    /// Updates ping and player statistics for a server
    pub fn update_ping(&mut self, index: usize, ping_ms: u32, players: u32, max_players: u32) {
        if let Some(srv) = self.servers.get_mut(index) {
            srv.ping_ms = Some(ping_ms);
            srv.player_count = Some(players);
            srv.max_players = Some(max_players);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_server_list_management() {
        let mut list = ServerListManager::new();
        assert_eq!(list.servers.len(), 2);

        let selected = list.get_selected_server().unwrap();
        assert_eq!(selected.host, "aemine.net");
        assert_eq!(selected.address_string(), "aemine.net:25565");

        list.add_server(ServerEntry::new(
            "Hypixel Proxy",
            "mc.hypixel.net",
            25565,
            GameVersion::V1_12_2,
        ));
        assert_eq!(list.servers.len(), 3);

        list.update_ping(2, 45, 35000, 100000);
        assert_eq!(list.servers[2].ping_ms, Some(45));

        assert!(list.remove_server(0));
        assert_eq!(list.servers.len(), 2);
    }
}
