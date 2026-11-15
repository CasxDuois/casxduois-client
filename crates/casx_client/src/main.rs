#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! Primary client executable integrating launcher, simulation, and rendering.
//!
//! Packaged into a single standalone binary `CasxDuois.exe` supporting:
//! - Graphical 3D client mode with launcher viewport and in-game HUD
//! - Headless Eco-AFK subprocess execution

use casx_assets::{generate_procedural_atlas, AssetBootstrapper, TextureAtlas};
use casx_core::ctl_server::CasxCtlServer;
use casx_core::jmath::DVec3;
use casx_core::physics::LocalPlayerPhysics;
use casx_core::world::ClientWorldCache;
use casx_core::{GameVersion, ProtocolAdapter};
use casx_launcher::{LaunchTarget, LauncherState};
use casx_net::session::v340::SessionV340;
use casx_platform::{HighResolutionTimer, PlatformTelemetry};
use casx_render::RenderPipeline5Pass;
use casx_sim::{raycast_world, PlayerSimulation, VoxelHit};
use casx_ui::{CasxUiApp, ScreenMode, UiAction};
use casx_world::WorldStorage;
use clap::Parser;
use log::{debug, error, info, warn};
use proto_1_12_2::Proto1_12_2;
use proto_26_2::Proto26_2;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowId};

/// Command-line arguments for CasxDuois.
#[derive(Parser, Debug, Clone, PartialEq)]
#[command(
    name = "CasxDuois",
    version = "0.1.0",
    about = "CasxDuois Multiplayer-Only Client Engine"
)]
pub struct CasxCliArgs {
    /// Runtime mode: "client" (graphical 3D client) or "afk" (headless Eco-AFK runner)
    #[arg(short, long, default_value = "client")]
    pub mode: String,

    /// Protocol version: "340" (1.12.2), "776" (26.2), or "auto"
    #[arg(long, default_value = "340")]
    pub ver: String,

    /// Target server address (host:port, e.g. 127.0.0.1:25565)
    #[arg(short, long)]
    pub server: Option<String>,

    /// Player username or offline nickname
    #[arg(short, long)]
    pub nick: Option<String>,

    /// Backward compatibility flag for headless AFK mode
    #[arg(long)]
    pub headless_afk: bool,

    /// Smoke test flag to run headless simulation ticks and exit
    #[arg(long)]
    pub smoke_test: bool,
}

/// Client operating state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineStage {
    /// Initial launcher viewport
    Launcher,
    /// Loading textures and allocating GPU atlas
    LoadingAssets,
    /// Active 3D voxel world simulation and rendering
    InGame,
}

pub struct ClientApp {
    pub stage: EngineStage,
    pub launcher: LauncherState,
    pub bootstrapper: AssetBootstrapper,
    pub pipeline: RenderPipeline5Pass,
    pub telemetry: PlatformTelemetry,
    pub timer: HighResolutionTimer,
    pub world: ClientWorldCache,
    pub physics: LocalPlayerPhysics,
    pub ctl_server: CasxCtlServer,
    pub active_protocol: Box<dyn ProtocolAdapter>,
    pub ui: CasxUiApp,
    pub in_game: bool,
    pub ping_ms: u32,

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
        info!(target: "casx_client::init", "Initializing CasxDuois Client Engine v{}", env!("CARGO_PKG_VERSION"));

        let telemetry = PlatformTelemetry::default();
        let timer = HighResolutionTimer::new(60);
        let world = ClientWorldCache::new();
        let physics = LocalPlayerPhysics::default();
        let ctl_server = CasxCtlServer::default();
        let active_protocol: Box<dyn ProtocolAdapter> = Box::new(Proto1_12_2::new());
        let ui = CasxUiApp::default();

        let mut bootstrapper = AssetBootstrapper::new();
        let (_pixels, fallback_atlas) = generate_procedural_atlas();
        let atlas = bootstrapper.bootstrap().cloned().unwrap_or(fallback_atlas);
        let pipeline = RenderPipeline5Pass::new();

        let world_storage = WorldStorage::new();
        let sim = PlayerSimulation::new(DVec3::new(0.0, 64.0, 0.0));

        info!(target: "casx_client::sys", "Detected system physical memory: {} MiB", telemetry.total_memory_mb());
        info!(target: "casx_client::init", "Default protocol target: 1.12.2 (v340)");
        info!(target: "casx_render::atlas", "Allocated CPU atlas buffer ({}x{})", atlas.width, atlas.height);

        Self {
            stage: EngineStage::Launcher,
            launcher: LauncherState::default(),
            bootstrapper,
            pipeline,
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

    /// Switches protocol adapter version.
    pub fn switch_protocol_version(&mut self, version: GameVersion) {
        info!(target: "casx_client::switch", "Switching protocol adapter to: {:?}", version);
        match version {
            GameVersion::V1_12_2 => {
                self.active_protocol = Box::new(Proto1_12_2::new());
            }
            GameVersion::V26_2 => {
                self.active_protocol = Box::new(Proto26_2::new());
            }
        }
    }

    /// Computes interpolated camera position between physics ticks:
    /// $$\vec{P}_{\text{render}} = \vec{P}_{\text{prev}} + \left(\vec{P}_{\text{curr}} - \vec{P}_{\text{prev}}\right) \times \alpha$$
    pub fn get_interpolated_render_pos(&self) -> DVec3 {
        let alpha = (self.accumulator / 0.05).clamp(0.0, 1.0);
        self.prev_tick_pos.lerp(&self.sim.pos, alpha)
    }

    /// Executes single main engine update tick.
    pub fn update_tick(&mut self) {
        let delta = self.timer.tick_frame();
        self.telemetry.poll_updates();

        if self.in_game {
            self.accumulator += delta;

            while self.accumulator >= 0.05 {
                self.prev_tick_pos = self.sim.pos;

                self.sim.tick(&self.world_storage);

                self.physics.step_physics_tick(
                    &self.world,
                    if self.ui.key_w {
                        1.0
                    } else if self.ui.key_s {
                        -1.0
                    } else {
                        0.0
                    },
                    if self.ui.key_d {
                        1.0
                    } else if self.ui.key_a {
                        -1.0
                    } else {
                        0.0
                    },
                    self.ui.key_space,
                );

                self.accumulator -= 0.05;
            }

            let eye_pos = DVec3::new(self.sim.pos.x, self.sim.pos.y + 1.62, self.sim.pos.z);
            let yaw_rad = (self.sim.yaw as f64).to_radians();
            let pitch_rad = (self.sim.pitch as f64).to_radians();
            let look_dir = DVec3::new(
                -yaw_rad.sin() * pitch_rad.cos(),
                -pitch_rad.sin(),
                yaw_rad.cos() * pitch_rad.cos(),
            );

            self.target_block = raycast_world(&self.world_storage, eye_pos, look_dir, 4.5);
            if let Some(ref hit) = self.target_block {
                self.pipeline.set_targeted_block_outline(Some(
                    casx_core::physics::Aabb::from_block(glam::IVec3::new(
                        hit.block_pos.x,
                        hit.block_pos.y,
                        hit.block_pos.z,
                    )),
                ));
            } else {
                self.pipeline.set_targeted_block_outline(None);
            }
        }

        if let Ok(mut state) = self.ctl_server.state.lock() {
            state.in_game = self.in_game;
            state.fps = self.timer.average_fps();
            state.ping_ms = self.ping_ms;
            state.loaded_chunks = self.world.loaded_chunk_count();
            state.player_pos = [self.sim.pos.x, self.sim.pos.y, self.sim.pos.z];
        }

        let delta_ms = delta * 1000.0;
        if delta_ms > 16.66 && self.in_game {
            warn!(target: "casx_client::loop", "Frame time exceeded target budget: {:.2} ms", delta_ms);
        }
    }

    /// Dispatches UI actions.
    pub fn handle_ui_action(&mut self, action: UiAction) {
        match action {
            UiAction::SwitchVersion(ver) => {
                self.switch_protocol_version(ver);
                self.launcher.profile_manager.switch_active_version(ver);
            }
            UiAction::ConnectServer {
                host,
                port,
                username,
                version,
            } => {
                info!(
                    target: "casx_client::net",
                    "Connecting to {}:{} as '{}' (protocol: {:?})",
                    host, port, username, version
                );
                self.switch_protocol_version(version);
                self.in_game = true;
                self.stage = EngineStage::InGame;
                self.ui.screen_mode = ScreenMode::InGame;
                self.session_v340 = Some(SessionV340::new(&username));
            }
            UiAction::Disconnect => {
                info!(target: "casx_client::net", "Disconnected from server, returning to launcher");
                self.in_game = false;
                self.stage = EngineStage::Launcher;
                self.ui.screen_mode = ScreenMode::Hub;
                self.session_v340 = None;
            }
            UiAction::SendChatMessage(msg) => {
                debug!(target: "casx_client::chat", "Outgoing chat message: {}", msg);
            }
            UiAction::SelectHotbarSlot(slot) => {
                debug!(target: "casx_client::hotbar", "Selected hotbar slot: {}", slot);
                self.ui.selected_hotbar_slot = slot;
            }
            UiAction::None => {}
        }
    }

    /// Launches session from launcher profile configuration.
    pub fn launch_from_launcher(&mut self) {
        let target = self.launcher.build_launch_target();
        match target {
            LaunchTarget::NormalClient {
                host,
                port,
                username,
                version,
            } => {
                self.handle_ui_action(UiAction::ConnectServer {
                    host,
                    port,
                    username,
                    version,
                });
            }
            LaunchTarget::EcoAfk { .. } => {
                let session_id = self.launcher.spawn_afk_bot_from_current_selection();
                info!(target: "casx_client::afk", "Spawned background Eco-AFK session ID: {}", session_id);
            }
        }
    }
}

/// Runner managing the winit application lifecycle.
struct AppRunner {
    client: ClientApp,
    window: Option<Window>,
}

impl ApplicationHandler for AppRunner {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_none() {
            let window_attributes = Window::default_attributes()
                .with_title("CasxDuois Client Engine")
                .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));
            match event_loop.create_window(window_attributes) {
                Ok(w) => {
                    info!(target: "casx_client::window", "Created primary window ({}x{})", 1280, 720);
                    self.window = Some(w);
                }
                Err(e) => {
                    error!(target: "casx_client::window", "Failed to create primary window: {:?}", e);
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                info!(target: "casx_client::window", "Close requested, terminating event loop");
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.client.update_tick();
                if let Some(w) = self.window.as_ref() {
                    w.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let args = CasxCliArgs::parse();

    if args.mode == "afk" || args.headless_afk {
        let host = args.server.unwrap_or_else(|| "127.0.0.1:25565".to_string());
        let nick = args.nick.unwrap_or_else(|| "EcoBot".to_string());
        info!(
            target: "casx_client::afk",
            "Starting headless Eco-AFK session (server: {}, nick: {}, ver: {})",
            host, nick, args.ver
        );

        let world = WorldStorage::new();
        let mut sim = PlayerSimulation::new(DVec3::new(0.0, 64.0, 0.0));
        for _ in 0..10 {
            sim.tick(&world);
        }
        info!(target: "casx_client::afk", "Headless Eco-AFK test cycle completed");
        return;
    }

    let mut app = ClientApp::new();

    if let Some(srv) = args.server {
        let nick = args.nick.unwrap_or_else(|| "CasxPlayer".to_string());
        let ver = if args.ver == "776" {
            GameVersion::V26_2
        } else {
            GameVersion::V1_12_2
        };
        let parts: Vec<&str> = srv.split(':').collect();
        let host = parts.first().unwrap_or(&"127.0.0.1").to_string();
        let port = parts
            .get(1)
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(25565);

        info!(target: "casx_client::cli", "Direct connection requested to {}:{}", host, port);
        app.handle_ui_action(UiAction::ConnectServer {
            host,
            port,
            username: nick,
            version: ver,
        });
    }

    if args.smoke_test {
        for _ in 0..5 {
            app.update_tick();
        }
        info!(target: "casx_client::smoke", "Headless smoke test completed successfully");
        return;
    }

    match EventLoop::new() {
        Ok(event_loop) => {
            let mut runner = AppRunner {
                client: app,
                window: None,
            };
            let _ = event_loop.run_app(&mut runner);
        }
        Err(e) => {
            warn!(target: "casx_client::window", "Headless environment detected ({:?}), running tick fallback", e);
            for _ in 0..5 {
                app.update_tick();
            }
        }
    }
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

        app.handle_ui_action(UiAction::SwitchVersion(GameVersion::V26_2));
        assert_eq!(app.active_protocol.version(), GameVersion::V26_2);

        app.handle_ui_action(UiAction::ConnectServer {
            host: "127.0.0.1".to_string(),
            port: 25565,
            username: "CasxTester".to_string(),
            version: GameVersion::V26_2,
        });
        assert!(app.in_game);
        assert_eq!(app.stage, EngineStage::InGame);
        assert_eq!(app.ui.screen_mode, ScreenMode::InGame);

        for _ in 0..3 {
            app.update_tick();
        }

        let render_pos = app.get_interpolated_render_pos();
        assert!(render_pos.y <= 64.0);
    }

    #[test]
    fn test_launcher_in_window_morphing_and_afk_launch() {
        let mut app = ClientApp::new();
        assert_eq!(app.stage, EngineStage::Launcher);

        app.launch_from_launcher();
        assert_eq!(app.stage, EngineStage::InGame);
        assert!(app.in_game);
        assert_eq!(app.ui.screen_mode, ScreenMode::InGame);

        app.handle_ui_action(UiAction::Disconnect);
        assert_eq!(app.stage, EngineStage::Launcher);
        assert!(!app.in_game);

        app.launcher.is_eco_afk_selected = true;
        app.launch_from_launcher();
        assert_eq!(app.launcher.afk_manager.active_count(), 1);
        assert!(app.launcher.afk_manager.total_memory_consumption_mb() <= 15.0);
    }

    #[test]
    fn test_cli_args_parsing() {
        let args_default = CasxCliArgs::parse_from(["CasxDuois"]);
        assert_eq!(args_default.mode, "client");
        assert_eq!(args_default.ver, "340");
        assert!(args_default.server.is_none());

        let args_afk = CasxCliArgs::parse_from([
            "CasxDuois",
            "--mode",
            "afk",
            "--ver",
            "776",
            "--server",
            "aemine.net:25565",
            "--nick",
            "BotAlpha",
        ]);
        assert_eq!(args_afk.mode, "afk");
        assert_eq!(args_afk.ver, "776");
        assert_eq!(args_afk.server.as_deref(), Some("aemine.net:25565"));
        assert_eq!(args_afk.nick.as_deref(), Some("BotAlpha"));
    }
}
