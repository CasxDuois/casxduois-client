# GHI CHÚ KỸ THUẬT HORIZON (PHẦN 3): PHÂN TẦNG KIẾN TRÚC (`layers.md`), NATIVE RUST MODS & `casx-ctl` (CHI TIẾT 100%)

* **Tham chiếu gốc:** `horizon-rust/horizon` (`docs/layers.md`, `AGENTS.md`, `native-hooks.json`, `horizon-mixins`, `horizon-ctl`)
* **Áp dụng cho:** **CasxDuois Client** (Nhân Rust Multiplayer-Only hỗ trợ `1.12.2` và `26.2`)

---

## 1. Kiến trúc Phân tầng Nghiêm ngặt & Quy tắc Phụ thuộc Crate (`docs/layers.md`)

Trong **Horizon**, toàn bộ mã nguồn được tổ chức thành một đồ thị có hướng không chu trình ($\text{DAG}$). Bất kỳ crate nào ở tầng thấp đều tuyệt đối không được phép nhập (`use`) kiểu dữ liệu từ crate ở tầng cao hơn. Điều này đảm bảo thời gian biên dịch tăng dần (Incremental Compilation) chỉ mất từ $0.4\text{ s}$ đến $1.2\text{ s}$ và tách biệt hoàn toàn logic mạng với đồ họa.

```text
[Tầng 4: Ứng dụng Thực thi]     casx_client   (winit EventLoop, tokio Runtime, Main Loop)
                                      │
                   ┌──────────────────┼──────────────────┬──────────────────┐
                   ▼                  ▼                  ▼                  ▼
[Tầng 3: Tính năng]            casx_ui            casx_render        proto_1_12_2       proto_26_2
                         (HoYo/Legacy/Lunar)   (wgpu, Mesher)      (Protocol 340)    (Protocol 770)
                   │                  │                  │                  │
                   └──────────────────┼──────────────────┴──────────────────┘
                                      ▼
[Tầng 2: Nhân Thế giới]          casx_core     (WorldCache, AABB, DDA Raycast, Native Mods, IPC)
                                      │
                                      ▼
[Tầng 1: Nền tảng & Phần cứng]  casx_platform  (CPU/GPU Telemetry, OS Paths, High-Res Timer)
```

### 1.1. Bảng Kiểm soát Biên giới Phụ thuộc Giữa Các Tầng

| Tầng | Tên Crate | Các Thư viện Được Phép Dùng | Tuyệt Đối Cấm Phụ Thuộc Vào | Trách nhiệm Cốt lõi |
| :--- | :--- | :--- | :--- | :--- |
| **Tầng 1** | `casx_platform` | `std`, `sysinfo` | `wgpu`, `egui`, `tokio`, `casx_core` | Đo thông số VRAM/CPU, lấy đường dẫn thư mục hệ điều hành, đồng hồ độ phân giải nano giây. |
| **Tầng 2** | `casx_core` | `glam`, `bytes`, `flate2`, `aes`, `cfb8`, `md-5`, `serde`, `serde_json` | `wgpu`, `winit`, `egui`, `proto_*` | Lưu trữ Chunk chuẩn hóa, vật lý $\text{AABB}$ $20\text{ TPS}$, dò tia DDA, NBT, giao diện `ProtocolAdapter` và `CasxNativeMod`. |
| **Tầng 3A** | `proto_1_12_2` | `casx_core`, `bytes` | `proto_26_2`, `wgpu`, `egui` | Giải mã/mã hóa toàn bộ gói tin của Minecraft `1.12.2` (Protocol $340$). |
| **Tầng 3B** | `proto_26_2` | `casx_core`, `bytes` | `proto_1_12_2`, `wgpu`, `egui` | Giải mã/mã hóa toàn bộ gói tin của Minecraft `26.2` (Protocol $770$, trạng thái `Configuration`). |
| **Tầng 3C** | `casx_render` | `casx_core`, `wgpu`, `rayon`, `glam`, `bytemuck` | `egui`, `winit`, `tokio`, `proto_*` | Greedy Meshing $6$ hướng, Voxel Ambient Occlusion, Frustum Culling, Shader WGSL & Dual Kawase Blur. |
| **Tầng 3D** | `casx_ui` | `casx_core`, `egui` | `wgpu`, `winit`, `tokio`, `proto_*` | Giao diện hợp nhất $\text{HoYoPlay} \times \text{Legacy Launcher} \times \text{Lunar Client HUD}$. |
| **Tầng 4** | `casx_client` | Tất cả các crate tầng dưới + `winit`, `tokio`, `egui-winit`, `egui-wgpu` | Không có (Binary cuối) | Kết nối tất cả các tầng lại thành một tệp thực thi duy nhất. |

### 1.2. Áp dụng Chế độ `remote` từ `AGENTS.md` của Horizon

Trong tài liệu `AGENTS.md` của Horizon, vòng lặp trò chơi phân tách thành `tick_integrated()`, `tick_serving()`, và `tick_remote()`. **CasxDuois Client** khóa cứng kiến trúc ở chế độ **`remote` (Client-Server Authoritative)** với quy trình **Dự đoán phía Client & Đồng bộ Server (Client-Side Prediction & Server Reconciliation)**:

1. **Vòng lặp Dự đoán Cục bộ ($20\text{ TPS}$):** Khi người chơi nhấn phím di chuyển hoặc đặt khối, Client cập nhật ngay lập tức trên `ClientWorldCache` và `LocalPlayerPhysics` để người chơi cảm nhận độ trễ bằng $0\text{ ms}$.
2. **Đóng dấu Trình tự (Sequence Stamping):** Mỗi hành động tương tác khối (`PlayerDigging` / `UseItemOn`) được gắn một số thứ tự tăng dần `sequence_id: i32` (bắt buộc trên `26.2` qua gói `BlockChangedAck`).
3. **Khôi phục Trạng thái Khi Server Từ Chối (Rollback):** Nếu Server gửi gói `BlockUpdate` trả khối về trạng thái cũ (do khu vực Spawn được bảo vệ bởi WorldGuard), Client lập tức ghi đè lại `ClientWorldCache` và đánh dấu `dirty_mesh = true` để `rayon` dựng lại lưới trong khung hình kế tiếp.

---

## 2. Đặc tả Hệ thống Native Rust Mods (`native-hooks.json` & `horizon-mixins`)

Thay vì dùng Java Bytecode Injection (Mixin) tốn bộ nhớ và dễ gây xung đột phiên bản như Forge/Fabric, **CasxDuois Client** áp dụng cơ chế **Native Hook Points** của Horizon. Tất cả các tính năng của **Lunar Client** được viết bằng Rust thuần và gắn trực tiếp vào $12$ điểm móc (Hook Points) cố định trong vòng lặp Engine.

### 2.1. Bảng Đặc tả 12 Điểm Móc Sự Kiện (`native-hooks.json`)

| STT | Định danh Hook Point | Vị trí Gọi trong Vòng lặp | Dữ liệu Truyền vào (`HookContext`) | Các Native Mod Sử dụng Hook này |
| :--- | :--- | :--- | :--- | :--- |
| $01$ | `native_client_init` | Khởi tạo `CasxApp` | Cấu hình lưu trữ `.local/mods.json` | Nạp cấu hình bật/tắt và vị trí HUD của tất cả các Mod. |
| $02$ | `native_tick_remote_pre` | Đầu mỗi tick vật lý $20\text{ TPS}$ | Trạng thái phím bấm (`KeyState`) | **ToggleSprint**, **ToggleSneak**. |
| $03$ | `native_tick_remote_post` | Cuối mỗi tick vật lý $20\text{ TPS}$ | Vị trí người chơi, Ping, Hiệu ứng thuốc | **PotionEffects HUD**, **ArmorStatus HUD**, **Waypoint Distance**. |
| $04$ | `native_camera_compute_angles` | Trước khi tính ma trận View | `server_yaw`, `server_pitch`, `render_yaw`, `render_pitch` | **Freelook 360** (xoay camera độc lập với gói tin mạng). |
| $05$ | `native_camera_fov_modify` | Trước khi tính ma trận Projection | `base_fov`, `is_sprinting`, `is_zoom_held` | **Smooth Zoom** (thu phóng mượt), **Dynamic FOV Control**. |
| $06$ | `native_render_world_post` | Sau khi vẽ xong toàn bộ Chunk 3D | `CameraUniform`, `target_block_pos` | **Block Overlay** (viền phát sáng khối đang nhìn), **3D Waypoints**, **Hitboxes (`F3+B`)**. |
| $07$ | `native_screen_render_pre` | Trước khi vẽ UI 2D | Độ phân giải màn hình, trạng thái mở Menu | Kích hoạt shader **Dual-Filter Kawase Blur** làm mờ nền khi mở Lunar Menu. |
| $08$ | `native_screen_render_post` | Sau khi vẽ HUD mặc định | `egui::Context`, bảng điều khiển HUD | Vẽ **Keystrokes**, **CPS Counter**, **FPS/Ping**, **Custom Scoreboard**, **Custom Crosshair**. |
| $09$ | `native_mouse_button_pre` | Khi người chơi nhấn chuột | `MouseButton`, `Instant` | **CPS Counter** (cửa sổ trượt $1000\text{ ms}$), **Auto-Clicker Prevention**. |
| $10$ | `native_packet_inbound_pre` | Ngay sau khi giải mã gói tin mạng | `&ServerEvent` | **Chat Filter / Anti-Spam**, **Auto-GG**, **Auto-Tip**, **Ping Tracker**. |
| $11$ | `native_packet_outbound_pre` | Trước khi gửi gói tin lên Server | `&mut ClientAction` | **Chat Macro / Quick Commands**, kiểm soát gói tin di chuyển. |
| $12$ | `native_world_unload` | Khi ngắt kết nối hoặc đổi Server | Lý do ngắt kết nối | Dọn dẹp bộ nhớ Waypoints theo từng Server và lưu cấu hình HUD. |

---

## 3. Giao thức Điều khiển & Tự động hóa `casx-ctl` (Local IPC & MCP Server)

Lấy cảm hứng từ **`horizon-ctl`** của Horizon, **CasxDuois Client** tích hợp một máy chủ điều khiển cục bộ chạy qua **Local TCP Loopback (`127.0.0.1:25599`)** sử dụng giao thức **JSON-RPC 2.0 (tương thích Model Context Protocol - MCP)**. Công cụ này cho phép:
* Tự động hóa kiểm thử (Automated Testing) khi phát triển Server `1.12.2` và `26.2`.
* Kết nối với các công cụ giám sát hiệu năng hoặc macro hợp lệ của người chơi.

### 3.1. Đặc tả Các Lệnh Điều Khiển JSON-RPC (`casx-ctl`)

| Phương thức RPC | Tham số Đầu vào (`params`) | Kết quả Trả về (`result`) | Mô tả Chức năng |
| :--- | :--- | :--- | :--- |
| `casx.status` | `{}` | `{ "in_game": true, "version": "V26_2", "pos": [x, y, z], "fps": 580.4, "ping_ms": 12, "loaded_chunks": 289 }` | Truy vấn trạng thái thời gian thực của toàn bộ Engine. |
| `casx.connect` | `{ "host": "127.0.0.1", "port": 25565, "version": "1.12.2", "username": "Tester" }` | `{ "accepted": true }` | Ra lệnh chuyển cảnh từ sảnh HoYoPlay vào Server ngay lập tức. |
| `casx.disconnect` | `{}` | `{ "disconnected": true }` | Ngắt kết nối hiện tại và quay về sảnh chờ. |
| `casx.chat` | `{ "message": "/spawn" }` | `{ "sent": true }` | Gửi tin nhắn hoặc lệnh lên Server đang kết nối. |
| `casx.click_slot` | `{ "window_id": 1, "slot": 13, "button": 0 }` | `{ "clicked": true }` | Bấm vào một ô vật phẩm trong Rương/Menu GUI của Server. |
| `casx.toggle_mod` | `{ "mod_id": "lunar_freelook", "enabled": true }` | `{ "mod_id": "lunar_freelook", "enabled": true }` | Bật hoặc tắt nóng một Native Rust Mod bất kỳ. |

---

## 4. Mã nguồn Hoàn chỉnh Hệ thống Native Mods & `casx-ctl` IPC Server

### 4.1. Bộ Điều Phối Native Rust Mods (`crates/casx_core/src/native_mods.rs`)

```rust
use crate::{ClientAction, ServerEvent};
use glam::{DVec3, Vec3};
use std::collections::HashMap;

/// Bối cảnh tổng hợp truyền vào các Native Rust Mods
pub struct ModRuntimeContext<'a> {
    pub player_pos: DVec3,
    pub server_yaw: f32,
    pub server_pitch: f32,
    pub render_yaw: &'a mut f32,
    pub render_pitch: &'a mut f32,
    pub render_fov: &'a mut f32,
    pub is_alt_held: bool,
    pub is_zoom_held: bool,
    pub is_forward_held: bool,
    pub sprinting_state: &'a mut bool,
}

/// Trait chuẩn cho mọi Native Rust Mod của CasxDuois Client
pub trait CasxNativeMod: Send + Sync {
    fn mod_id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn category(&self) -> &'static str; // "PVP" | "HUD" | "WORLD" | "UTILITY"
    fn is_enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);

    fn on_tick_remote(&mut self, _ctx: &mut ModRuntimeContext<'_>) {}
    fn on_camera_update(&mut self, _ctx: &mut ModRuntimeContext<'_>) {}
    fn on_packet_inbound(&mut self, _event: &ServerEvent) {}
    fn on_packet_outbound(&mut self, _action: &mut ClientAction) {}
}

// ============================================================================
// MOD 1: FREELOOK 360 (XOAY CAMERA ĐỘC LẬP VỚI GÓC NHÌN GỬI LÊN SERVER)
// ============================================================================

pub struct FreelookMod {
    pub enabled: bool,
    pub free_yaw: f32,
    pub free_pitch: f32,
    active_last_frame: bool,
}

impl Default for FreelookMod {
    fn default() -> Self {
        Self { enabled: true, free_yaw: 0.0, free_pitch: 0.0, active_last_frame: false }
    }
}

impl FreelookMod {
    pub fn apply_mouse_delta(&mut self, delta_yaw: f32, delta_pitch: f32) {
        if self.active_last_frame {
            self.free_yaw += delta_yaw;
            self.free_pitch = (self.free_pitch + delta_pitch).clamp(-90.0, 90.0);
        }
    }
}

impl CasxNativeMod for FreelookMod {
    fn mod_id(&self) -> &'static str { "lunar_freelook" }
    fn display_name(&self) -> &'static str { "Freelook 360 (Giữ Alt)" }
    fn category(&self) -> &'static str { "PVP" }
    fn is_enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    fn on_camera_update(&mut self, ctx: &mut ModRuntimeContext<'_>) {
        if !self.enabled {
            self.active_last_frame = false;
            return;
        }
        if ctx.is_alt_held {
            if !self.active_last_frame {
                self.free_yaw = ctx.server_yaw;
                self.free_pitch = ctx.server_pitch;
                self.active_last_frame = true;
            }
            *ctx.render_yaw = self.free_yaw;
            *ctx.render_pitch = self.free_pitch;
        } else {
            self.active_last_frame = false;
            *ctx.render_yaw = ctx.server_yaw;
            *ctx.render_pitch = ctx.server_pitch;
        }
    }
}

// ============================================================================
// MOD 2: SMOOTH ZOOM (THU PHÓNG GÓC NHÌN MƯỢT MÀ THEO ĐƯỜNG CONG MŨ)
// ============================================================================

pub struct SmoothZoomMod {
    pub enabled: bool,
    pub target_fov: f32,
    current_factor: f32,
}

impl Default for SmoothZoomMod {
    fn default() -> Self {
        Self { enabled: true, target_fov: 16.0, current_factor: 1.0 }
    }
}

impl CasxNativeMod for SmoothZoomMod {
    fn mod_id(&self) -> &'static str { "lunar_smooth_zoom" }
    fn display_name(&self) -> &'static str { "Smooth Zoom (Giữ C)" }
    fn category(&self) -> &'static str { "UTILITY" }
    fn is_enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    fn on_camera_update(&mut self, ctx: &mut ModRuntimeContext<'_>) {
        if !self.enabled {
            return;
        }
        let base = (*ctx.render_fov).max(30.0);
        let target = if ctx.is_zoom_held { self.target_fov / base } else { 1.0 };
        self.current_factor += (target - self.current_factor) * 0.22;
        *ctx.render_fov = base * self.current_factor;
    }
}

// ============================================================================
// MOD 3: TOGGLE SPRINT (TỰ ĐỘNG CHẠY NƯỚC RÚT AN TOÀN CHO ANTI-CHEAT)
// ============================================================================

pub struct ToggleSprintMod {
    pub enabled: bool,
    pub locked_sprint: bool,
}

impl Default for ToggleSprintMod {
    fn default() -> Self {
        Self { enabled: true, locked_sprint: true }
    }
}

impl CasxNativeMod for ToggleSprintMod {
    fn mod_id(&self) -> &'static str { "lunar_toggle_sprint" }
    fn display_name(&self) -> &'static str { "Toggle Sprint" }
    fn category(&self) -> &'static str { "PVP" }
    fn is_enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    fn on_tick_remote(&mut self, ctx: &mut ModRuntimeContext<'_>) {
        if self.enabled && self.locked_sprint && ctx.is_forward_held {
            *ctx.sprinting_state = true;
        }
    }
}

// ============================================================================
// MOD 4: 3D SERVER WAYPOINTS (ĐÁNH DẤU TỌA ĐỘ BOSS / KHU VỰC FARM)
// ============================================================================

#[derive(Debug, Clone)]
pub struct WaypointMarker {
    pub name: String,
    pub position: Vec3,
    pub color_rgb: [u8; 3],
    pub visible: bool,
}

pub struct WaypointsMod {
    pub enabled: bool,
    pub markers: Vec<WaypointMarker>,
}

impl Default for WaypointsMod {
    fn default() -> Self {
        Self {
            enabled: true,
            markers: vec![WaypointMarker {
                name: "Khu Vực Trung Tâm (Spawn)".into(),
                position: Vec3::new(0.5, 65.0, 0.5),
                color_rgb: [90, 210, 255],
                visible: true,
            }],
        }
    }
}

impl CasxNativeMod for WaypointsMod {
    fn mod_id(&self) -> &'static str { "lunar_waypoints" }
    fn display_name(&self) -> &'static str { "3D Waypoints" }
    fn category(&self) -> &'static str { "WORLD" }
    fn is_enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }
}

// ============================================================================
// MOD 5: AUTO-GG & CHAT FILTER (TỰ ĐỘNG CHÚC MỪNG SAU TRẬN PVP / MINIGAME)
// ============================================================================

pub struct AutoGgMod {
    pub enabled: bool,
    pub trigger_phrases: Vec<&'static str>,
    pub reply_text: String,
    pub pending_auto_chat: Option<String>,
}

impl Default for AutoGgMod {
    fn default() -> Self {
        Self {
            enabled: true,
            trigger_phrases: vec!["Winner:", "Chiến thắng:", "1st Killer -", "Game Over!"],
            reply_text: "gg".to_string(),
            pending_auto_chat: None,
        }
    }
}

impl CasxNativeMod for AutoGgMod {
    fn mod_id(&self) -> &'static str { "lunar_auto_gg" }
    fn display_name(&self) -> &'static str { "Auto GG" }
    fn category(&self) -> &'static str { "UTILITY" }
    fn is_enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    fn on_packet_inbound(&mut self, event: &ServerEvent) {
        if !self.enabled {
            return;
        }
        if let ServerEvent::SystemChat { json_message, is_action_bar: false } = event {
            if self.trigger_phrases.iter().any(|p| json_message.contains(p)) {
                self.pending_auto_chat = Some(self.reply_text.clone());
            }
        }
    }
}

/// Bảng quản lý tập trung toàn bộ Native Mods
pub struct NativeModRegistry {
    pub freelook: FreelookMod,
    pub smooth_zoom: SmoothZoomMod,
    pub toggle_sprint: ToggleSprintMod,
    pub waypoints: WaypointsMod,
    pub auto_gg: AutoGgMod,
    pub custom_states: HashMap<String, bool>,
}

impl Default for NativeModRegistry {
    fn default() -> Self {
        Self {
            freelook: FreelookMod::default(),
            smooth_zoom: SmoothZoomMod::default(),
            toggle_sprint: ToggleSprintMod::default(),
            waypoints: WaypointsMod::default(),
            auto_gg: AutoGgMod::default(),
            custom_states: HashMap::new(),
        }
    }
}
```

### 4.2. Bộ Điều Khiển Từ Xa `casx-ctl` JSON-RPC Server (`crates/casx_core/src/ctl_ipc.rs`)

```rust
use crate::GameVersion;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<Value>,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
}

/// Các lệnh điều khiển từ tiến trình `casx-ctl` bên ngoài gửi vào Client
#[derive(Debug, Clone)]
pub enum ExternalCtlCommand {
    RequestStatus,
    ConnectServer {
        host: String,
        port: u16,
        version: GameVersion,
        username: String,
    },
    DisconnectServer,
    SendChat {
        message: String,
    },
    ClickContainerSlot {
        window_id: u8,
        slot: i16,
        button: u8,
    },
    ToggleMod {
        mod_id: String,
        enabled: bool,
    },
}

/// Phân tích chuỗi JSON-RPC nhận từ cổng 127.0.0.1:25599 thành lệnh nội bộ
pub fn parse_ctl_rpc_line(line: &str) -> Result<(Value, ExternalCtlCommand), JsonRpcResponse> {
    let req: JsonRpcRequest = serde_json::from_str(line).map_err(|e| JsonRpcResponse {
        jsonrpc: "2.0",
        id: Value::Null,
        result: None,
        error: Some(JsonRpcError { code: -32700, message: format!("Lỗi cú pháp JSON: {e}") }),
    })?;

    let req_id = req.id.unwrap_or(Value::Null);

    let cmd = match req.method.as_str() {
        "casx.status" => ExternalCtlCommand::RequestStatus,
        "casx.connect" => {
            let host = req.params["host"].as_str().unwrap_or("127.0.0.1").to_string();
            let port = req.params["port"].as_u64().unwrap_or(25565) as u16;
            let ver_str = req.params["version"].as_str().unwrap_or("26.2");
            let version = if ver_str == "1.12.2" { GameVersion::V1_12_2 } else { GameVersion::V26_2 };
            let username = req.params["username"].as_str().unwrap_or("CasxDuois").to_string();
            ExternalCtlCommand::ConnectServer { host, port, version, username }
        }
        "casx.disconnect" => ExternalCtlCommand::DisconnectServer,
        "casx.chat" => {
            let message = req.params["message"].as_str().unwrap_or("").to_string();
            ExternalCtlCommand::SendChat { message }
        }
        "casx.click_slot" => {
            let window_id = req.params["window_id"].as_u64().unwrap_or(0) as u8;
            let slot = req.params["slot"].as_i64().unwrap_or(0) as i16;
            let button = req.params["button"].as_u64().unwrap_or(0) as u8;
            ExternalCtlCommand::ClickContainerSlot { window_id, slot, button }
        }
        "casx.toggle_mod" => {
            let mod_id = req.params["mod_id"].as_str().unwrap_or("").to_string();
            let enabled = req.params["enabled"].as_bool().unwrap_or(true);
            ExternalCtlCommand::ToggleMod { mod_id, enabled }
        }
        other => {
            return Err(JsonRpcResponse {
                jsonrpc: "2.0",
                id: req_id,
                result: None,
                error: Some(JsonRpcError { code: -32601, message: format!("Không hỗ trợ phương thức: {other}") }),
            });
        }
    };

    Ok((req_id, cmd))
}

pub fn build_status_response(
    id: Value,
    in_game: bool,
    version: GameVersion,
    pos: [f64; 3],
    fps: f32,
    ping_ms: u32,
    loaded_chunks: usize,
) -> String {
    let resp = JsonRpcResponse {
        jsonrpc: "2.0",
        id,
        result: Some(json!({
            "in_game": in_game,
            "version": format!("{:?}", version),
            "position": pos,
            "fps": fps,
            "ping_ms": ping_ms,
            "loaded_chunks": loaded_chunks,
        })),
        error: None,
    };
    serde_json::to_string(&resp).unwrap_or_default()
}
```