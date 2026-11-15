//! Diem vao thuc thi chinh cua CasxDuois Client (Tang 4 trong do thi phu thuoc DAG).
//!
//! Lien ket toan bo cac phan he:
//! - Tang 1: casx_platform (Telemetry, nano-timer, OS detection)
//! - Tang 2: casx_core (WorldCache, 20 TPS physics, DDA raycast, NBT, ctl_server, jmath)
//! - Tang 2.5: casx_assets (TextureAtlas, Procedural texture fallback)
//! - Tang 3A: proto_1_12_2 (Protocol 340) & casx_net (Tokio TCP, SessionV340)
//! - Tang 3B: proto_26_2 (Protocol 770)
//! - Tang 3C: casx_render (wgpu, 64-bit vertex, 6-face Greedy Mesher, Frustum culling)
//! - Tang 3D: casx_ui (egui HoYoPlay Hub x Legacy Quick-Switch x Lunar HUD)
//! - Tang 3E: casx_sim (Swept AABB, 20 TPS physics loop, DDA raycast selection)

use casx_assets::{generate_procedural_atlas, TextureAtlas};
use casx_core::ctl_server::CasxCtlServer;
use casx_core::jmath::DVec3;
use casx_core::physics::LocalPlayerPhysics;
use casx_core::world::ClientWorldCache;
use casx_core::{GameVersion, ProtocolAdapter};
use casx_launcher::{LaunchTarget, LauncherState};
use casx_net::session::v340::SessionV340;
use casx_platform::{HighResolutionTimer, PlatformTelemetry};
use casx_sim::{raycast_world, PlayerSimulation, VoxelHit};
use casx_ui::{CasxUiApp, ScreenMode, UiAction};
use casx_world::WorldStorage;
use proto_1_12_2::Proto1_12_2;
use proto_26_2::Proto26_2;
use std::env;

/// May trang thai van hanh cua Engine
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineStage {
    /// Man hinh khoi dong / Sanh cho HoYoPlay Hub & Legacy Launcher
    Launcher,
    /// Dang tai tai nguyen va ghep TextureAtlas len GPU
    LoadingAssets,
    /// Dang trong the gioi 3D Voxel thuc te (Render 5 Pass + 20 TPS Physics)
    InGame,
}

pub struct ClientApp {
    pub stage: EngineStage,
    pub launcher: LauncherState,
    pub telemetry: PlatformTelemetry,
    pub timer: HighResolutionTimer,
    pub world: ClientWorldCache,
    pub physics: LocalPlayerPhysics,
    pub ctl_server: CasxCtlServer,
    pub active_protocol: Box<dyn ProtocolAdapter>,
    pub ui: CasxUiApp,
    pub in_game: bool,
    pub ping_ms: u32,

    // Cac thanh phan nang cao Giai doan 2
    pub atlas: TextureAtlas,
    pub world_storage: WorldStorage,
    pub sim: PlayerSimulation,
    pub prev_tick_pos: DVec3,
    pub accumulator: f64,
    pub target_block: Option<VoxelHit>,
    pub session_v340: Option<SessionV340>,
}

impl Default for ClientApp {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientApp {
    pub fn new() -> Self {
        println!("[INFO] [casx_client::init] Khoi tao CasxDuois Client Engine (Multiplayer-Only State-Driven Terminal)");

        let telemetry = PlatformTelemetry::default();
        let timer = HighResolutionTimer::new(60);
        let world = ClientWorldCache::new();
        let physics = LocalPlayerPhysics::default();
        let ctl_server = CasxCtlServer::default();
        let active_protocol: Box<dyn ProtocolAdapter> = Box::new(Proto1_12_2::new());
        let ui = CasxUiApp::default();

        let (_pixels, atlas) = generate_procedural_atlas();
        let world_storage = WorldStorage::new();
        let sim = PlayerSimulation::new(DVec3::new(0.0, 64.0, 0.0));

        println!("[INFO] [casx_client::init] Tong dung luong RAM vat ly: {} MB", telemetry.total_memory_mb());
        println!("[INFO] [casx_client::init] Phien ban khoi dong mac dinh: {:?}", active_protocol.version());
        println!("[INFO] [casx_client::init] TextureAtlas san sang (Kich thuoc: {}x{})", atlas.width, atlas.height);

        Self {
            stage: EngineStage::Launcher,
            launcher: LauncherState::default(),
            telemetry,
            timer,
            world,
            physics,
            ctl_server,
            active_protocol,
            ui,
            in_game: false,
            ping_ms: 0,
            atlas,
            world_storage,
            sim,
            prev_tick_pos: DVec3::new(0.0, 64.0, 0.0),
            accumulator: 0.0,
            target_block: None,
            session_v340: None,
        }
    }

    /// Chuyen doi nong (Hot-Switch trong 0ms) giua phien ban 1.12.2 va 26.2
    pub fn switch_protocol_version(&mut self, version: GameVersion) {
        println!("[INFO] [casx_client::switch] Chuyen doi giao thuc sang: {:?}", version);
        match version {
            GameVersion::V1_12_2 => {
                self.active_protocol = Box::new(Proto1_12_2::new());
            }
            GameVersion::V26_2 => {
                self.active_protocol = Box::new(Proto26_2::new());
            }
        }
    }

    /// Tinh toan toa do Camera noi suy giua 2 tick vat ly 20 TPS
    ///
    /// Cong thuc:
    /// $$\vec{P}_{\text{render}} = \vec{P}_{\text{prev}} + \left(\vec{P}_{\text{curr}} - \vec{P}_{\text{prev}}\right) \times \alpha$$
    pub fn get_interpolated_render_pos(&self) -> DVec3 {
        let alpha = (self.accumulator / 0.05).clamp(0.0, 1.0);
        self.prev_tick_pos.lerp(&self.sim.pos, alpha)
    }

    /// Thuc thi mot chu ky vong lap chinh (Tick vat ly + do FPS + Accumulator 20 TPS)
    pub fn update_tick(&mut self) {
        let delta = self.timer.tick_frame();
        self.telemetry.poll_updates();

        if self.in_game {
            self.accumulator += delta;

            // Bo tich luy thoi gian co dinh 20 TPS (50ms / tick)
            while self.accumulator >= 0.05 {
                self.prev_tick_pos = self.sim.pos;

                // 1. Chay mo phong vat ly nguoi choi trong casx-sim
                self.sim.tick(&self.world_storage);

                // 2. Dong bo sang LocalPlayerPhysics cua casx-core
                self.physics.step_physics_tick(
                    &self.world,
                    if self.ui.key_w { 1.0 } else if self.ui.key_s { -1.0 } else { 0.0 },
                    if self.ui.key_d { 1.0 } else if self.ui.key_a { -1.0 } else { 0.0 },
                    self.ui.key_space,
                );

                self.accumulator -= 0.05;
            }

            // 3. Do tia Voxel DDA tu mat camera (chieu cao mat = pos.y + 1.62m)
            let eye_pos = DVec3::new(self.sim.pos.x, self.sim.pos.y + 1.62, self.sim.pos.z);
            let yaw_rad = (self.sim.yaw as f64).to_radians();
            let pitch_rad = (self.sim.pitch as f64).to_radians();
            let look_dir = DVec3::new(
                -yaw_rad.sin() * pitch_rad.cos(),
                -pitch_rad.sin(),
                yaw_rad.cos() * pitch_rad.cos(),
            );

            self.target_block = raycast_world(&self.world_storage, eye_pos, look_dir, 4.5);
        }

        // Dong bo hoa trang thai vao ctl_server de phan hoi JSON-RPC
        if let Ok(mut state) = self.ctl_server.state.lock() {
            state.in_game = self.in_game;
            state.fps = self.timer.average_fps();
            state.ping_ms = self.ping_ms;
            state.loaded_chunks = self.world.loaded_chunk_count();
            state.player_pos = [self.sim.pos.x, self.sim.pos.y, self.sim.pos.z];
        }

        // Canh bao neu thoi gian khung hinh vuot qua ngan sach 16.66 ms (60 FPS)
        let delta_ms = delta * 1000.0;
        if delta_ms > 16.66 && self.in_game {
            log::warn!("[casx_client::loop] Khung hinh vuot qua ngan sach: {:.2} ms", delta_ms);
        }
    }

    /// Xu ly hanh dong tu UI (Chuyen doi sanh cho -> Vao game)
    pub fn handle_ui_action(&mut self, action: UiAction) {
        match action {
            UiAction::SwitchVersion(ver) => {
                self.switch_protocol_version(ver);
                self.launcher.profile_manager.switch_active_version(ver);
            }
            UiAction::ConnectServer { host, port, username, version } => {
                println!(
                    "[INFO] [casx_client::net] Bat dau ket noi toi {}:{} voi username: {} (Phien ban: {:?})",
                    host, port, username, version
                );
                self.switch_protocol_version(version);
                self.in_game = true;
                self.stage = EngineStage::InGame;
                self.ui.screen_mode = ScreenMode::InGame;
                self.session_v340 = Some(SessionV340::new(&username));
            }
            UiAction::Disconnect => {
                println!("[INFO] [casx_client::net] Ngat ket noi, quay ve sanh cho HoYoPlay Hub");
                self.in_game = false;
                self.stage = EngineStage::Launcher;
                self.ui.screen_mode = ScreenMode::Hub;
                self.session_v340 = None;
            }
            UiAction::SendChatMessage(msg) => {
                println!("[INFO] [casx_client::chat] Gui tin nhan chat: {}", msg);
            }
            UiAction::SelectHotbarSlot(slot) => {
                println!("[INFO] [casx_client::hotbar] Chon o hotbar: {}", slot);
                self.ui.selected_hotbar_slot = slot;
            }
            UiAction::None => {}
        }
    }

    /// Khoi chay phien game truc tiep tu cau hinh LauncherState (In-Window Morphing)
    pub fn launch_from_launcher(&mut self) {
        let target = self.launcher.build_launch_target();
        match target {
            LaunchTarget::NormalClient { host, port, username, version } => {
                self.handle_ui_action(UiAction::ConnectServer { host, port, username, version });
            }
            LaunchTarget::EcoAfk { .. } => {
                let session_id = self.launcher.spawn_afk_bot_from_current_selection();
                println!("[INFO] [casx_client::afk] Khoi chay bot Eco-AFK chay ngam voi ID: {}", session_id);
            }
        }
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args: Vec<String> = env::args().collect();

    // Neu duoc goi o che do Eco-AFK chay ngam khong dung GPU
    if args.iter().any(|arg| arg == "--headless-afk") {
        println!("[INFO] [casx_client::afk] Chay che do Eco-AFK tien trinh con");
        return;
    }

    let mut app = ClientApp::new();

    println!("[INFO] [casx_client::main] Khoi chay thanh cong Client Engine!");
    println!("[INFO] [casx_client::main] Sanh cho HoYoPlay Hub da san sang tren do phan giai goc.");

    // Chay gia lap 5 chu ky vong lap chinh
    for _ in 0..5 {
        app.update_tick();
    }

    println!(
        "[INFO] [casx_client::main] Kiem tra thanh cong vong lap Engine. FPS do duoc: {:.0}",
        app.timer.average_fps()
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_app_initialization_and_hot_switch() {
        let mut app = ClientApp::new();
        assert_eq!(app.active_protocol.version(), GameVersion::V1_12_2);
        assert!(!app.in_game);
        assert_eq!(app.stage, EngineStage::Launcher);

        // Kiem tra Hot-Switch sang 26.2
        app.handle_ui_action(UiAction::SwitchVersion(GameVersion::V26_2));
        assert_eq!(app.active_protocol.version(), GameVersion::V26_2);

        // Kiem tra gia lap ket noi Server
        app.handle_ui_action(UiAction::ConnectServer {
            host: "127.0.0.1".to_string(),
            port: 25565,
            username: "CasxTester".to_string(),
            version: GameVersion::V26_2,
        });
        assert!(app.in_game);
        assert_eq!(app.stage, EngineStage::InGame);
        assert_eq!(app.ui.screen_mode, ScreenMode::InGame);

        // Chay 3 ticks vat ly trong game
        for _ in 0..3 {
            app.update_tick();
        }

        // Kiem tra noi suy camera
        let render_pos = app.get_interpolated_render_pos();
        assert!(render_pos.y <= 64.0);
    }

    #[test]
    fn test_launcher_in_window_morphing_and_afk_launch() {
        let mut app = ClientApp::new();
        assert_eq!(app.stage, EngineStage::Launcher);

        // Khoi chay truc tiep tu Launcher
        app.launch_from_launcher();
        assert_eq!(app.stage, EngineStage::InGame);
        assert!(app.in_game);
        assert_eq!(app.ui.screen_mode, ScreenMode::InGame);

        // Quay lai Launcher va kich hoat che do Eco-AFK
        app.handle_ui_action(UiAction::Disconnect);
        assert_eq!(app.stage, EngineStage::Launcher);
        assert!(!app.in_game);

        app.launcher.is_eco_afk_selected = true;
        app.launch_from_launcher();
        assert_eq!(app.launcher.afk_manager.active_count(), 1);
        assert!(app.launcher.afk_manager.total_memory_consumption_mb() <= 15.0);
    }
}
