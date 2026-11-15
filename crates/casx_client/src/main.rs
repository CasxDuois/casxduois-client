//! Diem vao thuc thi chinh cua CasxDuois Client (Tang 4 trong do thi phu thuoc DAG).
//!
//! Lien ket toan bo cac phan he:
//! - Tang 1: casx_platform (Telemetry, nano-timer, OS detection)
//! - Tang 2: casx_core (WorldCache, 20 TPS physics, DDA raycast, NBT, ctl_server)
//! - Tang 3A: proto_1_12_2 (Protocol 340)
//! - Tang 3B: proto_26_2 (Protocol 770)
//! - Tang 3C: casx_render (wgpu, 64-bit vertex, Greedy Mesher)
//! - Tang 3D: casx_ui (egui HoYoPlay Hub x Legacy Quick-Switch x Lunar HUD)

use casx_core::ctl_server::CasxCtlServer;
use casx_core::physics::LocalPlayerPhysics;
use casx_core::world::ClientWorldCache;
use casx_core::{GameVersion, ProtocolAdapter};
use casx_platform::{HighResolutionTimer, PlatformTelemetry};
use casx_ui::{CasxUiApp, ScreenMode, UiAction};
use proto_1_12_2::Proto1_12_2;
use proto_26_2::Proto26_2;

pub struct ClientApp {
    pub telemetry: PlatformTelemetry,
    pub timer: HighResolutionTimer,
    pub world: ClientWorldCache,
    pub physics: LocalPlayerPhysics,
    pub ctl_server: CasxCtlServer,
    pub active_protocol: Box<dyn ProtocolAdapter>,
    pub ui: CasxUiApp,
    pub in_game: bool,
    pub ping_ms: u32,
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

        println!("[INFO] [casx_client::init] Tong dung luong RAM vat ly: {} MB", telemetry.total_memory_mb());
        println!("[INFO] [casx_client::init] Phien ban khoi dong mac dinh: {:?}", active_protocol.version());

        Self {
            telemetry,
            timer,
            world,
            physics,
            ctl_server,
            active_protocol,
            ui,
            in_game: false,
            ping_ms: 0,
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

    /// Thuc thi mot chu ky vong lap chinh (Tick vat ly + do FPS)
    pub fn update_tick(&mut self) {
        let delta = self.timer.tick_frame();
        self.telemetry.poll_updates();

        if self.in_game {
            // Chay mot tick vat ly 20 TPS
            self.physics.step_physics_tick(
                &self.world,
                if self.ui.key_w { 1.0 } else if self.ui.key_s { -1.0 } else { 0.0 },
                if self.ui.key_d { 1.0 } else if self.ui.key_a { -1.0 } else { 0.0 },
                self.ui.key_space,
            );
        }

        // Dong bo hoa trang thai vao ctl_server de phan hoi JSON-RPC
        if let Ok(mut state) = self.ctl_server.state.lock() {
            state.in_game = self.in_game;
            state.fps = self.timer.average_fps();
            state.ping_ms = self.ping_ms;
            state.loaded_chunks = self.world.loaded_chunk_count();
            state.player_pos = [self.physics.curr_pos.x, self.physics.curr_pos.y, self.physics.curr_pos.z];
        }

        // Canh bao neu thoi gian khung hinh vuot qua ngan sach 1.60 ms
        let delta_ms = delta * 1000.0;
        if delta_ms > 16.66 && self.in_game {
            log::warn!("[casx_client::loop] Khung hinh vuot qua ngan sach: {:.2} ms", delta_ms);
        }
    }

    /// Xu ly hanh dong tu UI
    pub fn handle_ui_action(&mut self, action: UiAction) {
        match action {
            UiAction::SwitchVersion(ver) => {
                self.switch_protocol_version(ver);
            }
            UiAction::ConnectServer { host, port, username, version } => {
                println!(
                    "[INFO] [casx_client::net] Bat dau ket noi toi {}:{} voi username: {} (Phien ban: {:?})",
                    host, port, username, version
                );
                self.switch_protocol_version(version);
                self.in_game = true;
                self.ui.screen_mode = ScreenMode::InGame;
            }
            UiAction::Disconnect => {
                println!("[INFO] [casx_client::net] Ngat ket noi, quay ve sanh cho HoYoPlay Hub");
                self.in_game = false;
                self.ui.screen_mode = ScreenMode::Hub;
            }
            UiAction::None => {}
        }
    }
}

fn main() {
    let mut app = ClientApp::new();

    println!("[INFO] [casx_client::main] Khoi chay thanh cong Client Engine!");
    println!("[INFO] [casx_client::main] Sanh cho HoYoPlay Hub da san sang tren do phan giai goc.");

    // Chay thu nghiem 5 chu ky vong lap chinh de xac thuc hoat dong
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
        assert_eq!(app.ui.screen_mode, ScreenMode::InGame);

        // Chay 3 ticks vat ly trong game
        for _ in 0..3 {
            app.update_tick();
        }
    }
}
