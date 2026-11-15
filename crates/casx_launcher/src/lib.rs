//! Crate `casx_launcher` (Tang 3 trong do thi phu thuoc DAG).
//!
//! Trach nhiem:
//! - Hop nhat giao dien HoYoPlay Hub, thanh cau hinh Legacy Quick-Switch va Lunar Client Modules.
//! - Quan ly ho so nguoi choi (Profiles, UUID, RAM allocation).
//! - Quan ly danh ba may chu yeu thich (Server list, ping cache).
//! - Dieu phoi he thong da tai khoan Eco-AFK chay ngam (< 15 MB RAM/bot).

pub mod profile;
pub mod server_list;
pub mod mods;
pub mod afk_manager;
pub mod launcher_app;

pub use profile::{AccountType, PlayerProfile, ProfileManager};
pub use server_list::{ServerEntry, ServerListManager};
pub use mods::ModConfig;
pub use afk_manager::{AfkSession, AfkSessionConfig, AfkSessionManager, AfkStatus};
pub use launcher_app::{LaunchTarget, LauncherState, LauncherTab};
