//! Multi-version protocol adapter trait, server events, and client actions.

use crate::inventory::ItemStack;
use crate::world::{CanonicalBlockState, ChunkColumn};
use crate::ProtocolError;
use bytes::BytesMut;

/// Supported Minecraft protocol targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum GameVersion {
    V1_12_2,
    V26_2,
}

/// Minecraft network connection state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Handshaking,
    Status,
    Login,
    Configuration,
    Play,
}

/// Normalized server events decoded from inbound packets.
#[derive(Debug, Clone)]
pub enum ServerEvent {
    LoginSuccess {
        uuid: String,
        username: String,
    },
    CompressionEnabled {
        threshold: i32,
    },
    ConfigurationComplete,
    JoinServerWorld {
        entity_id: i32,
        dimension: i32,
        max_players: u8,
    },
    KeepAliveRequest {
        keep_alive_id: i64,
    },
    SynchronizePlayerPosition {
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        teleport_id: i32,
    },
    ChunkColumnLoad {
        column: ChunkColumn,
    },
    BlockChange {
        x: i32,
        y: i32,
        z: i32,
        state: CanonicalBlockState,
    },
    SpawnPlayer {
        entity_id: i32,
        name: String,
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
    },
    EntityMove {
        entity_id: i32,
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        is_relative: bool,
    },
    DestroyEntities {
        entity_ids: Vec<i32>,
    },
    OpenContainerWindow {
        window_id: u8,
        title: String,
        slot_count: usize,
    },
    ContainerSetItems {
        window_id: u8,
        state_id: i32,
        items: Vec<Option<ItemStack>>,
    },
    SystemChat {
        json_message: String,
        is_action_bar: bool,
    },
    Disconnected {
        reason: String,
    },
}

/// Client actions encoded into outbound network packets
#[derive(Debug, Clone)]
pub enum ClientAction {
    InitiateLogin {
        server_host: String,
        server_port: u16,
        username: String,
    },
    KeepAliveResponse {
        keep_alive_id: i64,
    },
    ConfirmTeleport {
        teleport_id: i32,
    },
    SendPositionAndRotation {
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
    SendChatMessage {
        message: String,
    },
    ClickContainerSlot {
        window_id: u8,
        state_id: i32,
        slot: i16,
        button: u8,
        mode: i32,
        action_id: i16,
    },
    CloseContainerWindow {
        window_id: u8,
    },
}

/// Protocol adapter trait enabling hot-switching between 1.12.2 and 26.2 with zero overhead
pub trait ProtocolAdapter: Send + Sync {
    fn version(&self) -> GameVersion;
    fn current_state(&self) -> ConnectionState;
    fn compression_threshold(&self) -> Option<i32>;

    fn encode_action(
        &mut self,
        action: ClientAction,
        out_buf: &mut BytesMut,
    ) -> Result<(), ProtocolError>;
    fn decode_packets(
        &mut self,
        in_buf: &mut BytesMut,
        out_auto_reply: &mut BytesMut,
    ) -> Result<Vec<ServerEvent>, ProtocolError>;
}
