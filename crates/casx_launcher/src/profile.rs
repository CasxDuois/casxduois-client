//! Player profile, authentication type, and memory allocation management.

use casx_core::crypto::{format_uuid, generate_offline_uuid};
use casx_core::GameVersion;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Authentication account type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum AccountType {
    #[default]
    Offline,
    ElyBy,
    Microsoft,
}

/// Complete player profile
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlayerProfile {
    pub uuid: String,
    pub username: String,
    pub account_type: AccountType,
    pub selected_version: GameVersion,
    pub ram_min_mb: u32,
    pub ram_max_mb: u32,
    pub custom_jvm_args: Vec<String>,
}

impl PlayerProfile {
    /// Creates offline profile with deterministically generated UUID from username
    pub fn new_offline(username: &str) -> Self {
        let uuid = format_uuid(generate_offline_uuid(username));
        Self {
            uuid,
            username: username.to_string(),
            account_type: AccountType::Offline,
            selected_version: GameVersion::V1_12_2,
            ram_min_mb: 512,
            ram_max_mb: 2048,
            custom_jvm_args: Vec::new(),
        }
    }
}

/// Player profile manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileManager {
    pub active_profile_uuid: String,
    pub profiles: HashMap<String, PlayerProfile>,
}

impl Default for ProfileManager {
    fn default() -> Self {
        let default_profile = PlayerProfile::new_offline("CasxPlayer");
        let active_uuid = default_profile.uuid.clone();
        let mut profiles = HashMap::new();
        profiles.insert(active_uuid.clone(), default_profile);

        Self {
            active_profile_uuid: active_uuid,
            profiles,
        }
    }
}

impl ProfileManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Retrieves currently selected player profile
    pub fn get_active_profile(&self) -> Option<&PlayerProfile> {
        self.profiles.get(&self.active_profile_uuid)
    }

    /// Retrieves currently selected player profile for mutation
    pub fn get_active_profile_mut(&mut self) -> Option<&mut PlayerProfile> {
        self.profiles.get_mut(&self.active_profile_uuid)
    }

    /// Adds a new player profile
    pub fn add_profile(&mut self, profile: PlayerProfile) {
        let uuid = profile.uuid.clone();
        self.profiles.insert(uuid.clone(), profile);
        self.active_profile_uuid = uuid;
    }

    /// Selects player profile by UUID
    pub fn select_profile(&mut self, uuid: &str) -> bool {
        if self.profiles.contains_key(uuid) {
            self.active_profile_uuid = uuid.to_string();
            true
        } else {
            false
        }
    }

    /// Switches active version of current profile.
    pub fn switch_active_version(&mut self, version: GameVersion) {
        if let Some(prof) = self.get_active_profile_mut() {
            prof.selected_version = version;
        }
    }

    /// Serialize cau hinh thanh chuoi JSON
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserialize cau hinh tu chuoi JSON
    pub fn from_json(json_str: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json_str)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_creation_and_json_roundtrip() {
        let mut manager = ProfileManager::new();
        let active = manager.get_active_profile().unwrap();
        assert_eq!(active.username, "CasxPlayer");
        assert_eq!(active.account_type, AccountType::Offline);

        let new_prof = PlayerProfile::new_offline("AlexMaster");
        let new_uuid = new_prof.uuid.clone();
        manager.add_profile(new_prof);

        assert_eq!(manager.active_profile_uuid, new_uuid);
        assert_eq!(manager.get_active_profile().unwrap().username, "AlexMaster");

        let json = manager.to_json().unwrap();
        let loaded = ProfileManager::from_json(&json).unwrap();
        assert_eq!(loaded.active_profile_uuid, new_uuid);
    }
}
