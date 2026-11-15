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
    pub atlas_pixels: Vec<u8>,
    pub world_storage: WorldStorage,
    pub world_mesh_dirty: bool,
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
        let (fallback_pixels, fallback_atlas) = generate_procedural_atlas();
        let atlas = bootstrapper.bootstrap().cloned().unwrap_or(fallback_atlas);
        let atlas_pixels = fallback_pixels;
        let pipeline = RenderPipeline5Pass::new();

        let mut world_storage = WorldStorage::new();
        world_storage.generate_flat_test_world();
        let sim = PlayerSimulation::new(DVec3::new(0.0, 70.0, 0.0));

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
            atlas_pixels,
            world_storage,
            world_mesh_dirty: true,
            sim,
            prev_tick_pos: DVec3::new(0.0, 70.0, 0.0),
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

    /// Executes a single 50ms (20 TPS) physics simulation tick.
    pub fn step_simulation_tick(&mut self) {
        self.prev_tick_pos = self.sim.pos;

        let forward = if self.ui.key_w {
            1.0
        } else if self.ui.key_s {
            -1.0
        } else {
            0.0
        };
        let strafe = if self.ui.key_d {
            1.0
        } else if self.ui.key_a {
            -1.0
        } else {
            0.0
        };

        self.sim.tick_with_input(
            &self.world_storage,
            forward,
            strafe,
            self.ui.key_space,
            self.physics.is_sneaking,
        );

        self.physics
            .step_physics_tick(&self.world, forward, strafe, self.ui.key_space);
    }

    /// Executes single main engine update tick.
    pub fn update_tick(&mut self) {
        let delta = self.timer.tick_frame();
        self.telemetry.poll_updates();

        if self.in_game {
            self.accumulator += delta;

            while self.accumulator >= 0.05 {
                self.step_simulation_tick();
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
                self.world_mesh_dirty = true;
            }
            UiAction::Disconnect => {
                info!(target: "casx_client::net", "Disconnected from server, returning to launcher");
                self.in_game = false;
                self.stage = EngineStage::Launcher;
                self.ui.screen_mode = ScreenMode::Hub;
                self.session_v340 = None;
                self.world_storage.generate_flat_test_world();
                self.world_mesh_dirty = true;
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

    /// Ingests network events from 1.12.2 session into world storage and updates simulation.
    pub fn process_session_events(
        &mut self,
        events: Vec<casx_net::session::v340::SessionEventV340>,
    ) {
        for event in events {
            match event {
                casx_net::session::v340::SessionEventV340::ChunkData {
                    chunk_x,
                    chunk_z,
                    ground_up_continuous,
                    primary_bit_mask,
                    data,
                } => {
                    let _ = self.world_storage.apply_chunk_packet_v340(
                        chunk_x,
                        chunk_z,
                        ground_up_continuous,
                        primary_bit_mask,
                        &data,
                    );
                    self.world_mesh_dirty = true;
                }
                casx_net::session::v340::SessionEventV340::BlockChange { pos, block_state } => {
                    let (x, y, z) = casx_core::codec::decode_block_pos_1_12_2(pos);
                    let (block_id, _) =
                        casx_core::palette::SectionPaletteV340::unpack_block_id_meta(
                            block_state as u16,
                        );
                    self.world_storage.set_block(x, y, z, block_id);
                    self.world_mesh_dirty = true;
                }
                casx_net::session::v340::SessionEventV340::MultiBlockChange {
                    chunk_x,
                    chunk_z,
                    records,
                } => {
                    for (rel_x, rel_y, rel_z, state) in records {
                        let wx = chunk_x * 16 + rel_x as i32;
                        let wy = rel_y as i32;
                        let wz = chunk_z * 16 + rel_z as i32;
                        let (block_id, _) =
                            casx_core::palette::SectionPaletteV340::unpack_block_id_meta(
                                state as u16,
                            );
                        self.world_storage.set_block(wx, wy, wz, block_id);
                    }
                    self.world_mesh_dirty = true;
                }
                casx_net::session::v340::SessionEventV340::PlayerPosLook {
                    x,
                    y,
                    z,
                    yaw,
                    pitch,
                    ..
                } => {
                    self.sim.set_position(DVec3::new(x, y, z), yaw, pitch);
                }
                casx_net::session::v340::SessionEventV340::Disconnect { reason } => {
                    warn!(target: "casx_client::net", "Disconnected from server: {}", reason);
                    self.handle_ui_action(UiAction::Disconnect);
                }
                _ => {}
            }
        }
    }

    /// Ingests section data from 26.2 container into world storage.
    pub fn apply_chunk_packet_v776(
        &mut self,
        chunk_x: i32,
        chunk_z: i32,
        sec_y: i8,
        container: casx_core::palette::PalettedContainerV776,
    ) {
        self.world_storage
            .insert_section_v776(chunk_x, chunk_z, sec_y, container);
        self.world_mesh_dirty = true;
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

/// Active GPU context and rendering resources.
struct GpuContext {
    window: std::sync::Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    depth_texture: wgpu::Texture,
    depth_view: wgpu::TextureView,
    terrain_pipeline: casx_render::TerrainGpuPipeline,
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    index_count: u32,
    egui_ctx: egui::Context,
    egui_winit: egui_winit::State,
    egui_renderer: egui_wgpu::Renderer,
}

/// Runner managing the winit application lifecycle.
struct AppRunner {
    client: ClientApp,
    gpu: Option<GpuContext>,
}

impl ApplicationHandler for AppRunner {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_none() {
            let window_attributes = Window::default_attributes()
                .with_title("CasxDuois Client Engine (Direct v340/v776)")
                .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));
            match event_loop.create_window(window_attributes) {
                Ok(w) => {
                    info!(target: "casx_client::window", "Created primary window (1280x720)");
                    let window = std::sync::Arc::new(w);
                    let instance = wgpu::Instance::default();
                    let surface = match instance.create_surface(std::sync::Arc::clone(&window)) {
                        Ok(s) => s,
                        Err(e) => {
                            error!(target: "casx_client::gpu", "Failed to create surface: {:?}", e);
                            return;
                        }
                    };

                    let adapter = match pollster::block_on(instance.request_adapter(
                        &wgpu::RequestAdapterOptions {
                            power_preference: wgpu::PowerPreference::HighPerformance,
                            compatible_surface: Some(&surface),
                            force_fallback_adapter: false,
                        },
                    )) {
                        Some(a) => a,
                        None => {
                            error!(target: "casx_client::gpu", "Failed to find suitable GPU adapter");
                            return;
                        }
                    };

                    info!(
                        target: "casx_client::gpu",
                        "Using GPU Adapter: {:?}",
                        adapter.get_info().name
                    );

                    let (device, queue) = match pollster::block_on(adapter.request_device(
                        &wgpu::DeviceDescriptor {
                            label: Some("CasxDevice"),
                            required_features: wgpu::Features::empty(),
                            required_limits: wgpu::Limits::default(),
                            memory_hints: Default::default(),
                        },
                        None,
                    )) {
                        Ok(pair) => pair,
                        Err(e) => {
                            error!(target: "casx_client::gpu", "Failed to create GPU device: {:?}", e);
                            return;
                        }
                    };

                    let size = window.inner_size();
                    let caps = surface.get_capabilities(&adapter);
                    let surface_format = caps
                        .formats
                        .iter()
                        .copied()
                        .find(|f| f.is_srgb())
                        .unwrap_or(caps.formats[0]);

                    let config = wgpu::SurfaceConfiguration {
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                        format: surface_format,
                        width: size.width.max(1),
                        height: size.height.max(1),
                        present_mode: wgpu::PresentMode::AutoVsync,
                        alpha_mode: caps.alpha_modes[0],
                        view_formats: vec![],
                        desired_maximum_frame_latency: 2,
                    };
                    surface.configure(&device, &config);

                    let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("DepthTexture"),
                        size: wgpu::Extent3d {
                            width: config.width,
                            height: config.height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: wgpu::TextureFormat::Depth32Float,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    });
                    let depth_view =
                        depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

                    let terrain_pipeline = casx_render::TerrainGpuPipeline::new(
                        &device,
                        &queue,
                        config.format,
                        &self.client.atlas,
                        &self.client.atlas_pixels,
                    );

                    let egui_ctx = egui::Context::default();
                    let egui_winit = egui_winit::State::new(
                        egui_ctx.clone(),
                        egui_ctx.viewport_id(),
                        &window,
                        Some(window.scale_factor() as f32),
                        None,
                        None,
                    );
                    let egui_renderer =
                        egui_wgpu::Renderer::new(&device, config.format, None, 1, false);

                    self.client.world_storage.generate_flat_test_world();
                    let mesh = casx_render::GreedyMesher::mesh_entire_world(
                        &self.client.world_storage,
                        &self.client.atlas,
                    );

                    let (vertex_buffer, index_buffer, index_count) = if !mesh.is_empty() {
                        let v_buf = device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("TerrainVertexBuffer"),
                            size: (mesh.vertices.len()
                                * std::mem::size_of::<casx_render::PackedVertex>())
                                as u64,
                            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        });
                        queue.write_buffer(&v_buf, 0, bytemuck::cast_slice(&mesh.vertices));

                        let i_buf = device.create_buffer(&wgpu::BufferDescriptor {
                            label: Some("TerrainIndexBuffer"),
                            size: (mesh.indices.len() * std::mem::size_of::<u32>()) as u64,
                            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                            mapped_at_creation: false,
                        });
                        queue.write_buffer(&i_buf, 0, bytemuck::cast_slice(&mesh.indices));

                        (Some(v_buf), Some(i_buf), mesh.indices.len() as u32)
                    } else {
                        (None, None, 0)
                    };

                    self.gpu = Some(GpuContext {
                        window,
                        surface,
                        device,
                        queue,
                        config,
                        depth_texture,
                        depth_view,
                        terrain_pipeline,
                        vertex_buffer,
                        index_buffer,
                        index_count,
                        egui_ctx,
                        egui_winit,
                        egui_renderer,
                    });
                }
                Err(e) => {
                    error!(target: "casx_client::window", "Failed to create primary window: {:?}", e);
                }
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let Some(ref mut gpu) = self.gpu {
            let res = gpu.egui_winit.on_window_event(&gpu.window, &event);
            if res.consumed {
                return;
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                info!(target: "casx_client::window", "Close requested, terminating event loop");
                event_loop.exit();
            }
            WindowEvent::Resized(new_size) => {
                if let Some(ref mut gpu) = self.gpu {
                    if new_size.width > 0 && new_size.height > 0 {
                        gpu.config.width = new_size.width;
                        gpu.config.height = new_size.height;
                        gpu.surface.configure(&gpu.device, &gpu.config);

                        gpu.depth_texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                            label: Some("DepthTexture"),
                            size: wgpu::Extent3d {
                                width: gpu.config.width,
                                height: gpu.config.height,
                                depth_or_array_layers: 1,
                            },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: wgpu::TextureFormat::Depth32Float,
                            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                                | wgpu::TextureUsages::TEXTURE_BINDING,
                            view_formats: &[],
                        });
                        gpu.depth_view = gpu
                            .depth_texture
                            .create_view(&wgpu::TextureViewDescriptor::default());
                    }
                }
            }
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        physical_key,
                        state,
                        ..
                    },
                ..
            } => {
                let pressed = state.is_pressed();
                match physical_key {
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyW) => {
                        self.client.ui.key_w = pressed;
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyA) => {
                        self.client.ui.key_a = pressed;
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyS) => {
                        self.client.ui.key_s = pressed;
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::KeyD) => {
                        self.client.ui.key_d = pressed;
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Space) => {
                        self.client.ui.key_space = pressed;
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::ShiftLeft)
                    | winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::ShiftRight) => {
                        self.client.physics.is_sneaking = pressed;
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Digit1)
                        if pressed =>
                    {
                        self.client.ui.selected_hotbar_slot = 0;
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Digit2)
                        if pressed =>
                    {
                        self.client.ui.selected_hotbar_slot = 1;
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Digit3)
                        if pressed =>
                    {
                        self.client.ui.selected_hotbar_slot = 2;
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Digit4)
                        if pressed =>
                    {
                        self.client.ui.selected_hotbar_slot = 3;
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Digit5)
                        if pressed =>
                    {
                        self.client.ui.selected_hotbar_slot = 4;
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Digit6)
                        if pressed =>
                    {
                        self.client.ui.selected_hotbar_slot = 5;
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Digit7)
                        if pressed =>
                    {
                        self.client.ui.selected_hotbar_slot = 6;
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Digit8)
                        if pressed =>
                    {
                        self.client.ui.selected_hotbar_slot = 7;
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Digit9)
                        if pressed =>
                    {
                        self.client.ui.selected_hotbar_slot = 8;
                    }
                    winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::Escape)
                        if pressed && self.client.in_game =>
                    {
                        self.client.handle_ui_action(casx_ui::UiAction::Disconnect);
                        if let Some(ref gpu) = self.gpu {
                            let _ = gpu
                                .window
                                .set_cursor_grab(winit::window::CursorGrabMode::None);
                            gpu.window.set_cursor_visible(true);
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested => {
                self.client.update_tick();

                if let Some(ref mut gpu) = self.gpu {
                    if self.client.world_mesh_dirty {
                        let mesh = casx_render::GreedyMesher::mesh_entire_world(
                            &self.client.world_storage,
                            &self.client.atlas,
                        );
                        if !mesh.is_empty() {
                            let v_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                                label: Some("TerrainVertexBuffer"),
                                size: (mesh.vertices.len()
                                    * std::mem::size_of::<casx_render::PackedVertex>())
                                    as u64,
                                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                                mapped_at_creation: false,
                            });
                            gpu.queue
                                .write_buffer(&v_buf, 0, bytemuck::cast_slice(&mesh.vertices));

                            let i_buf = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                                label: Some("TerrainIndexBuffer"),
                                size: (mesh.indices.len() * std::mem::size_of::<u32>()) as u64,
                                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                                mapped_at_creation: false,
                            });
                            gpu.queue
                                .write_buffer(&i_buf, 0, bytemuck::cast_slice(&mesh.indices));

                            gpu.vertex_buffer = Some(v_buf);
                            gpu.index_buffer = Some(i_buf);
                            gpu.index_count = mesh.indices.len() as u32;
                        }
                        self.client.world_mesh_dirty = false;
                    }

                    let render_pos = self.client.get_interpolated_render_pos();
                    let eye_pos = glam::Vec3::new(
                        render_pos.x as f32,
                        (render_pos.y + 1.62) as f32,
                        render_pos.z as f32,
                    );
                    let yaw_rad = self.client.sim.yaw.to_radians();
                    let pitch_rad = self.client.sim.pitch.to_radians();
                    let forward = glam::Vec3::new(
                        -yaw_rad.sin() * pitch_rad.cos(),
                        -pitch_rad.sin(),
                        yaw_rad.cos() * pitch_rad.cos(),
                    )
                    .normalize_or_zero();

                    let target = eye_pos + forward;
                    let view = glam::Mat4::look_at_rh(eye_pos, target, glam::Vec3::Y);
                    let aspect = gpu.config.width as f32 / gpu.config.height.max(1) as f32;
                    let proj =
                        glam::Mat4::perspective_rh(70.0f32.to_radians(), aspect, 0.05, 500.0);
                    let view_proj = proj * view;

                    let camera_uniform = casx_render::CameraUniform {
                        view_proj: view_proj.to_cols_array_2d(),
                        camera_pos: [eye_pos.x, eye_pos.y, eye_pos.z, 1.0],
                    };
                    gpu.terrain_pipeline
                        .update_camera(&gpu.queue, &camera_uniform);

                    let frame = match gpu.surface.get_current_texture() {
                        Ok(f) => f,
                        Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                            gpu.surface.configure(&gpu.device, &gpu.config);
                            return;
                        }
                        Err(e) => {
                            error!(target: "casx_client::render", "Surface error: {:?}", e);
                            return;
                        }
                    };

                    let view = frame
                        .texture
                        .create_view(&wgpu::TextureViewDescriptor::default());
                    let mut encoder =
                        gpu.device
                            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                                label: Some("PrimaryEncoder"),
                            });

                    let sky = self.client.pipeline.pass1_sky.sky_color;

                    {
                        let mut terrain_pass =
                            encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("TerrainRenderPass"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: &view,
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(wgpu::Color {
                                            r: sky[0] as f64,
                                            g: sky[1] as f64,
                                            b: sky[2] as f64,
                                            a: 1.0,
                                        }),
                                        store: wgpu::StoreOp::Store,
                                    },
                                })],
                                depth_stencil_attachment: Some(
                                    wgpu::RenderPassDepthStencilAttachment {
                                        view: &gpu.depth_view,
                                        depth_ops: Some(wgpu::Operations {
                                            load: wgpu::LoadOp::Clear(1.0),
                                            store: wgpu::StoreOp::Store,
                                        }),
                                        stencil_ops: None,
                                    },
                                ),
                                ..Default::default()
                            });

                        if gpu.index_count > 0 {
                            if let (Some(ref v_buf), Some(ref i_buf)) =
                                (&gpu.vertex_buffer, &gpu.index_buffer)
                            {
                                terrain_pass.set_pipeline(&gpu.terrain_pipeline.render_pipeline);
                                terrain_pass.set_bind_group(
                                    0,
                                    &gpu.terrain_pipeline.camera_bind_group,
                                    &[],
                                );
                                terrain_pass.set_bind_group(
                                    1,
                                    &gpu.terrain_pipeline.texture_bind_group,
                                    &[],
                                );
                                terrain_pass.set_vertex_buffer(0, v_buf.slice(..));
                                terrain_pass
                                    .set_index_buffer(i_buf.slice(..), wgpu::IndexFormat::Uint32);
                                terrain_pass.draw_indexed(0..gpu.index_count, 0, 0..1);
                            }
                        }
                    }

                    let raw_input = gpu.egui_winit.take_egui_input(&gpu.window);
                    let fps = self.client.timer.average_fps();
                    let ping = self.client.ping_ms;
                    let in_game = self.client.in_game;

                    let mut ui_action = casx_ui::UiAction::None;
                    let full_output = gpu.egui_ctx.run(raw_input, |ctx| {
                        ui_action = self.client.ui.update(ctx, in_game, fps, ping);
                    });

                    if ui_action != casx_ui::UiAction::None {
                        self.client.handle_ui_action(ui_action);
                        if self.client.in_game {
                            let _ = gpu
                                .window
                                .set_cursor_grab(winit::window::CursorGrabMode::Locked);
                            gpu.window.set_cursor_visible(false);
                        } else {
                            let _ = gpu
                                .window
                                .set_cursor_grab(winit::window::CursorGrabMode::None);
                            gpu.window.set_cursor_visible(true);
                        }
                    }

                    gpu.egui_winit
                        .handle_platform_output(&gpu.window, full_output.platform_output);

                    let tris = gpu
                        .egui_ctx
                        .tessellate(full_output.shapes, full_output.pixels_per_point);

                    for (id, delta) in &full_output.textures_delta.set {
                        gpu.egui_renderer
                            .update_texture(&gpu.device, &gpu.queue, *id, delta);
                    }

                    let screen_descriptor = egui_wgpu::ScreenDescriptor {
                        size_in_pixels: [gpu.config.width, gpu.config.height],
                        pixels_per_point: gpu.window.scale_factor() as f32,
                    };

                    gpu.egui_renderer.update_buffers(
                        &gpu.device,
                        &gpu.queue,
                        &mut encoder,
                        &tris,
                        &screen_descriptor,
                    );

                    {
                        let mut ui_pass = encoder
                            .begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("EguiRenderPass"),
                                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                    view: &view,
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Load,
                                        store: wgpu::StoreOp::Store,
                                    },
                                })],
                                depth_stencil_attachment: None,
                                ..Default::default()
                            })
                            .forget_lifetime();
                        gpu.egui_renderer
                            .render(&mut ui_pass, &tris, &screen_descriptor);
                    }

                    for id in &full_output.textures_delta.free {
                        gpu.egui_renderer.free_texture(id);
                    }

                    gpu.queue.submit(Some(encoder.finish()));
                    frame.present();
                    gpu.window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: winit::event::DeviceId,
        event: winit::event::DeviceEvent,
    ) {
        if let winit::event::DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            if self.client.in_game {
                let sensitivity = 0.15f32;
                self.client.sim.yaw = (self.client.sim.yaw + (dx as f32 * sensitivity)) % 360.0;
                self.client.sim.pitch =
                    (self.client.sim.pitch + (dy as f32 * sensitivity)).clamp(-89.9, 89.9);
                self.client.physics.yaw = self.client.sim.yaw;
                self.client.physics.pitch = self.client.sim.pitch;
            }
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

        let mut world = WorldStorage::new();
        world.generate_flat_test_world();
        let mut sim = PlayerSimulation::new(DVec3::new(0.0, 70.0, 0.0));
        for _ in 0..20 {
            sim.tick(&world);
        }
        info!(
            target: "casx_client::afk",
            "Headless Eco-AFK test cycle completed: landed at Y = {:.2}, on_ground = {}",
            sim.pos.y, sim.on_ground
        );
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
        app.world_storage.generate_flat_test_world();
        let mesh = casx_render::GreedyMesher::mesh_entire_world(&app.world_storage, &app.atlas);
        info!(
            target: "casx_client::smoke",
            "Headless smoke test: generated flat test world with {} chunks, {} quads in mesh",
            app.world_storage.chunks.len(),
            mesh.quad_count()
        );
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
                gpu: None,
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

        for _ in 0..20 {
            app.step_simulation_tick();
        }

        let render_pos = app.get_interpolated_render_pos();
        assert_eq!(render_pos.y, 64.0);
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
