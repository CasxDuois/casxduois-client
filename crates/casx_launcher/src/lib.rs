//! Crate `casx_launcher` (Layer 3 in DAG).
//!
//! Responsibilities:
//! - Profile management (Profiles, UUID, RAM allocation).
//! - Server list management and ping cache.
//! - Eco-AFK multi-account background orchestration.

pub mod afk_manager;
pub mod launcher_app;
pub mod mods;
pub mod profile;
pub mod server_list;

pub use afk_manager::{AfkSession, AfkSessionConfig, AfkSessionManager, AfkStatus};
pub use launcher_app::{LaunchTarget, LauncherState, LauncherTab};
pub use mods::ModConfig;
pub use profile::{AccountType, PlayerProfile, ProfileManager};
pub use server_list::{ServerEntry, ServerListManager};
