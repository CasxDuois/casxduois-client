# GHI CHÚ KỸ THUẬT HORIZON (PHẦN 3): PHÂN TẦNG KIẾN TRÚC (`layers.md`), NATIVE RUST MODS & `casx-ctl`

* **Tham chiếu gốc:** `horizon-rust/horizon` (`docs/layers.md`, `AGENTS.md`, `native-hooks.json`, `horizon-mixins`, `horizon-ctl`)
* **Áp dụng cho:** **CasxDuois Client** (Hệ thống Mod Lunar nội sinh & Tự động hóa)

---

## 1. Kiến trúc Phân tầng Nghiêm ngặt (`docs/layers.md`)

Trong **Horizon**, các crate được chia thành những tầng phụ thuộc một chiều (Directed Acyclic Graph) để ngăn chặn sự phình to của mã nguồn và cho phép biên dịch song song cực nhanh:

```text
[Tầng 4: Ứng dụng & Giao diện]   casx_client  (winit, tokio runtime, vòng lặp chính)
                                      │
                   ┌──────────────────┼──────────────────┐
                   ▼                  ▼                  ▼
[Tầng 3: Tính năng]            casx_ui           casx_render        proto_1_12_2 / proto_26_2
                         (HoYo/Legacy/Lunar)   (wgpu, Mesher)      (Bộ dịch gói tin)
                   │                  │                  │
                   └──────────────────┼──────────────────┘
                                      ▼
[Tầng 2: Nhân Thế giới]          casx_core    (WorldCache, AABB Physics, Entities, NBT)
                                      │
                                      ▼
[Tầng 1: Nền tảng & Phần cứng]  casx_platform (Thông tin GPU/VRAM, OS, Bộ đếm hiệu năng)
```

### Áp dụng Chế độ `remote` từ `AGENTS.md` của Horizon
Trong tệp `AGENTS.md` của Horizon, cấu trúc `Game` phân tách rõ ràng giữa máy chủ tích hợp (`integrated`), máy chủ chuyên dụng (`serving`), và chế độ khách từ xa (`remote` $\rightarrow$ `tick_remote()`).
**CasxDuois Client** khóa cứng chế độ vận hành ở **`remote` mode**:
* Mọi hành động của người chơi (đập khối, đặt khối, mở rương, tấn công thực thể) không tự ý thay đổi trạng thái vĩnh viễn trên Client mà đi qua **Dự đoán phía Client (Client-side Prediction)** và chờ gói tin xác nhận (`BlockUpdate` / `ContainerSetItems`) từ Server để đồng bộ hóa.

---

## 2. Hệ thống Native Rust Mods (`native-hooks.json` & `horizon-mixins`)

Vì chạy trên nhân Rust thuần, **CasxDuois Client** không chạy các tệp `.jar` mod của Forge hay Fabric. Thay vào đó, chúng ta áp dụng trực tiếp kiến trúc **Native Hook Points** của Horizon để xây dựng toàn bộ các tính năng của **Lunar Client** (Zoom, Keystrokes, CPS, Freelook, ToggleSprint, Custom Crosshair, Waypoints, Minimap) dưới dạng **Native Rust Modules** biên dịch tĩnh không tốn chi phí gọi hàm ảo (`Zero-Overhead`):

### Danh sách Các Điểm Móc Sự Kiện (`CasxNativeHook`)

| Tên Hook (Tham chiếu Horizon) | Thời điểm Kích hoạt | Tác dụng trong CasxDuois Client |
| :--- | :--- | :--- |
| `native_tick_remote_post` | Cuối mỗi chu kỳ vật lý $20\text{ TPS}$ | Cập nhật trạng thái `ToggleSprint`, đếm thời gian hồi chiêu (Cooldowns), cập nhật tọa độ Waypoint. |
| `native_camera_compute_angles` | Trước khi tính ma trận View của GPU | Cho phép mod **Freelook 360** xoay góc nhìn camera tự do khi giữ phím `Alt` mà vẫn giữ nguyên `yaw`/`pitch` gửi lên Server (tránh bị Anti-Cheat phát hiện). |
| `native_camera_fov_modify` | Trước khi tính ma trận Projection | Cho phép mod **OptiFine/Lunar Zoom** thu hẹp góc nhìn $\text{FOV}$ xuống $15^\circ$ mượt mà theo đường cong nội suy mũ. |
| `native_screen_render_post` | Sau khi vẽ HUD và Container gốc | Vẽ lớp phủ kính mờ **Dual-Filter Kawase Blur** và các Widget kéo-thả của Lunar Client. |
| `native_screen_mouse_button_pressed_pre` | Trước khi xử lý chuột trên màn hình | Ghi nhận **LMB/RMB CPS** theo thời gian thực và chặn thao tác click xuyên qua bảng điều khiển `Right Shift`. |
| `native_use_item_on` | Khi người chơi bấm chuột phải vào khối | Hỗ trợ các tiện ích hiển thị khung viền khối (Block Overlay) và thông tin vật phẩm. |

---

## 3. Giao diện Điều khiển Dòng lệnh & Tự động hóa (`casx-ctl` / MCP Server)

Một tính năng đột phá của **Horizon** là công cụ `horizon-ctl` (CLI & Model Context Protocol Server) cho phép điều khiển nhân vật từ bên ngoài để kiểm thử hoặc tự động hóa. **CasxDuois Client** tích hợp phiên bản thu gọn mang tên **`casx-ctl`** phục vụ cho việc kiểm thử Server và gỡ lỗi mạng:

* `casx-ctl status`: Trả về tọa độ hiện tại, Ping tới Server, phiên bản giao thức đang chạy (`1.12.2` hoặc `26.2`) và số lượng Chunk đang lưu trong VRAM.
* `casx-ctl connect <host:port> --version <1.12.2|26.2> --user <name>`: Ra lệnh cho Client chuyển cảnh từ sảnh HoYoPlay vào thẳng Server mục tiêu.
* `casx-ctl chat "<nội_dung_hoặc_lệnh>"`: Gửi tin nhắn hoặc lệnh `/login`, `/hub` lên Server.
* `casx-ctl click-slot <window_id> <slot_index>`: Bấm vào ô vật phẩm chỉ định trong Menu Rương của Server.

---

## 4. Mã nguồn Hệ thống Native Hook & Bộ Điều Phối Mod (`crates/casx_core/src/native_mods.rs`)

```rust
use glam::DVec3;

/// Bối cảnh truyền vào các Native Rust Mods tại mỗi khung hình hoặc mỗi Tick
pub struct HookContext<'a> {
    pub player_pos: DVec3,
    pub server_yaw: f32,
    pub server_pitch: f32,
    pub render_yaw: &'a mut f32,
    pub render_pitch: &'a mut f32,
    pub render_fov: &'a mut f32,
    pub is_alt_held: bool,
    pub is_c_held: bool,
}

/// Trait chuẩn cho mọi Native Mod của CasxDuois Client (tương đương native-hooks.json của Horizon)
pub trait CasxNativeMod: Send + Sync {
    fn mod_id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;
    fn is_enabled(&self) -> bool;
    fn set_enabled(&mut self, enabled: bool);

    /// Hook tương đương `native_camera_compute_angles` & `native_camera_fov_modify`
    fn on_camera_update(&mut self, _ctx: &mut HookContext<'_>) {}
}

/// Mod 1: Freelook 360 (Giữ phím Alt xoay góc nhìn độc lập với hướng gửi lên Server)
pub struct FreelookMod {
    pub enabled: bool,
    pub free_yaw: f32,
    pub free_pitch: f32,
    was_active: bool,
}

impl Default for FreelookMod {
    fn default() -> Self {
        Self {
            enabled: true,
            free_yaw: 0.0,
            free_pitch: 0.0,
            was_active: false,
        }
    }
}

impl CasxNativeMod for FreelookMod {
    fn mod_id(&self) -> &'static str { "lunar_freelook" }
    fn display_name(&self) -> &'static str { "Freelook 360 (Giữ Alt)" }
    fn is_enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    fn on_camera_update(&mut self, ctx: &mut HookContext<'_>) {
        if !self.enabled {
            return;
        }
        if ctx.is_alt_held {
            if !self.was_active {
                self.free_yaw = ctx.server_yaw;
                self.free_pitch = ctx.server_pitch;
                self.was_active = true;
            }
            *ctx.render_yaw = self.free_yaw;
            *ctx.render_pitch = self.free_pitch.clamp(-90.0, 90.0);
        } else {
            self.was_active = false;
            *ctx.render_yaw = ctx.server_yaw;
            *ctx.render_pitch = ctx.server_pitch;
        }
    }
}

/// Mod 2: Smooth Zoom (Thu phóng góc nhìn mượt mà khi giữ phím C)
pub struct SmoothZoomMod {
    pub enabled: bool,
    pub target_zoom_fov: f32,
    current_multiplier: f32,
}

impl Default for SmoothZoomMod {
    fn default() -> Self {
        Self {
            enabled: true,
            target_zoom_fov: 18.0,
            current_multiplier: 1.0,
        }
    }
}

impl CasxNativeMod for SmoothZoomMod {
    fn mod_id(&self) -> &'static str { "lunar_smooth_zoom" }
    fn display_name(&self) -> &'static str { "Smooth Zoom (Giữ C)" }
    fn is_enabled(&self) -> bool { self.enabled }
    fn set_enabled(&mut self, enabled: bool) { self.enabled = enabled; }

    fn on_camera_update(&mut self, ctx: &mut HookContext<'_>) {
        if !self.enabled {
            return;
        }
        let base_fov = *ctx.render_fov;
        let goal = if ctx.is_c_held {
            self.target_zoom_fov / base_fov.max(1.0)
        } else {
            1.0
        };
        // Nội suy mượt (Exponential Smoothing)
        self.current_multiplier += (goal - self.current_multiplier) * 0.25;
        *ctx.render_fov = base_fov * self.current_multiplier;
    }
}
```