# ĐẶC TẢ KỸ THUẬT CHI TIẾT 100%: CÁC PHÂN HỆ LÕI CỦA CASXDUOIS CLIENT

Tài liệu này hoàn thiện chi tiết $100\%$ cho mọi phân hệ còn lại của **CasxDuois Client** nhằm đảm bảo đội ngũ kỹ thuật có thể triển khai trực tiếp mà không gặp bất kỳ khoảng trống thiết kế nào.

---

## PHẦN 1: HỆ THỐNG XÁC THỰC ĐA NỀN TẢNG (OFFLINE / ELY.BY / MICROSOFT OAUTH)

Để người chơi có thể kết nối vào cả máy chủ cộng đồng Việt Nam (`online-mode=false` hoặc AuthMe), máy chủ dùng hệ thống tài khoản **Ely.by**, lẫn các máy chủ bản quyền quốc tế (`online-mode=true`), **CasxDuois Client** triển khai luồng xác thực $3$ chế độ trên thanh **Legacy Quick-Switch**.

### 1.1. Luồng Bắt Tay Mã Hóa `EncryptionRequest (0x01)` $\rightarrow$ `EncryptionResponse (0x01)`

Khi máy chủ bật `online-mode=true` (hoặc Ely.by Authlib), ngay sau gói `LoginStart (0x00)`, Server sẽ gửi gói `EncryptionRequest (0x01)` gồm:
* `ServerID` ($\text{String}$, tối đa $20$ ký tự, thường là rỗng trên bản hiện đại).
* `PublicKey` ($\text{ByteArray}$ theo chuẩn $\text{DER X.509 RSA-1024}$).
* `VerifyToken` ($\text{ByteArray}$, thường dài $4\text{ bytes}$).

Quy trình xử lý chính xác từng bước của CasxDuois Client:
1. **Sinh khóa bí mật phiên (`SharedSecret`):** Tạo ngẫu nhiên $16\text{ bytes}$ bằng bộ sinh số ngẫu nhiên an toàn mật mã (`rand::rngs::OsRng`).
2. **Tính mã băm máy chủ (`ServerHash`):** Sử dụng thuật toán $\text{SHA-1}$ theo định dạng số nguyên lớn có dấu hai bù (Two's Complement Hex Digest) đặc trưng của Mojang:
   $$\text{Digest} = \text{SHA1}(\text{ServerID}_{\text{ASCII}} \parallel \text{SharedSecret}_{16\text{B}} \parallel \text{PublicKey}_{\text{DER}})$$
   Nếu bit cao nhất của byte đầu tiên bằng $1$ ($\text{Digest}[0] \ \& \ \text{0x80} \ne 0$), lấy bù hai toàn bộ mảng $20\text{ bytes}$ và thêm dấu trừ `-` phía trước chuỗi thập lục phân.
3. **Xác nhận phiên với máy chủ xác thực (`HTTP POST`):**
   * **Tài khoản Microsoft:** Gửi `POST https://sessionserver.mojang.com/session/minecraft/join`
   * **Tài khoản Ely.by:** Gửi `POST https://authserver.ely.by/session/minecraft/join`
   * Nội dung JSON: `{"accessToken": "<token>", "selectedProfile": "<uuid_no_dashes>", "serverId": "<ServerHash>"}`.
4. **Mã hóa $\text{RSA PKCS\#1 v1.5}$ & Kích hoạt `AES-128-CFB8`:**
   * Mã hóa `SharedSecret` và `VerifyToken` bằng `PublicKey` của Server, gửi lên qua gói `EncryptionResponse (0x01)`.
   * Ngay sau khi gửi xong gói `0x01`, bật `StreamCipherPair::new(&shared_secret)` cho toàn bộ byte đọc/ghi tiếp theo trên `TcpStream`.

### 1.2. Chuỗi Đăng Nhập Microsoft OAuth2 Device Code Flow (Không cần WebView nặng nề)

1. `POST https://login.microsoftonline.com/consumers/oauth2/v2.0/devicecode` (`client_id`, `scope=XboxLive.signin offline_access`) $\rightarrow$ Nhận `user_code` và `device_code` hiển thị gọn trên thanh Legacy Bar.
2. `POST https://login.microsoftonline.com/consumers/oauth2/v2.0/token` $\rightarrow$ Nhận `access_token` Microsoft.
3. `POST https://user.auth.xboxlive.com/user/authenticate` $\rightarrow$ Nhận `XBL Token` và `uhs`.
4. `POST https://xsts.auth.xboxlive.com/xsts/authorize` $\rightarrow$ Nhận `XSTS Token`.
5. `POST https://api.minecraftservices.com/authentication/login_with_xbox` $\rightarrow$ Nhận `Minecraft Access Token`.

---

## PHẦN 2: THUẬT TOÁN DÒ TIA VOXEL 3D DDA & TƯƠNG TÁC KHỐI / THỰC THỂ

Để người chơi có thể chỉ chuột vào khối (đặt khối, đập khối, mở rương) hoặc tấn công thực thể (PvP Hitbox) với độ chính xác tuyệt đối đến từng dấu phẩy động, **CasxDuois Client** sử dụng thuật toán **Amanatides & Woo Fast Voxel Traversal (3D DDA)** kết hợp **Ray-AABB Slab Intersection**.

### 2.1. Công thức Toán học 3D DDA Raycasting

Cho vị trí mắt người chơi $\mathbf{o} = (o_x, o_y, o_z)$ (với $o_y = p_y + 1.62$ khi đứng và $p_y + 1.27$ khi cúi người `Sneak`), góc nhìn $(\text{yaw}, \text{pitch})$ tính bằng radian:
$$\mathbf{d} = \begin{pmatrix} -\sin(\text{yaw}) \cos(\text{pitch}) \\ -\sin(\text{pitch}) \\ \cos(\text{yaw}) \cos(\text{pitch}) \end{pmatrix}$$

Với mỗi trục $i \in \{x, y, z\}$:
* Bước nhảy ô nguyên: $\text{step}_i = \text{sign}(d_i) \in \{-1, +1\}$
* Khoảng cách tia đi qua $1$ đơn vị khối theo trục $i$: $\text{tDelta}_i = \left|\frac{1}{d_i}\right|$
* Khoảng cách tới ranh giới khối đầu tiên:
  $$\text{tMax}_i = \begin{cases} (\lfloor o_i \rfloor + 1 - o_i) \cdot \text{tDelta}_i & \text{nếu } d_i > 0 \\ (o_i - \lfloor o_i \rfloor) \cdot \text{tDelta}_i & \text{nếu } d_i < 0 \end{cases}$$

Tại mỗi bước lặp, chọn trục có $\text{tMax}_i$ nhỏ nhất, tăng tọa độ khối theo $\text{step}_i$, ghi nhận mặt khối bị chạm (`BlockFace`), và dừng lại ngay khi gặp khối không phải không khí (`!state.is_air()`) hoặc vượt quá tầm với ($R_{\text{block}} = 4.5\text{ blocks}$).

### 2.2. Mã nguồn 3D DDA Raycaster & Ray-AABB Hitbox (`crates/casx_core/src/raycast.rs`)

```rust
use crate::{Aabb, ClientWorldCache};
use glam::{DVec3, IVec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockFace {
    Bottom = 0, // -Y
    Top = 1,    // +Y
    North = 2,  // -Z
    South = 3,  // +Z
    West = 4,   // -X
    East = 5,   // +X
}

#[derive(Debug, Clone, Copy)]
pub struct BlockHitResult {
    pub block_pos: IVec3,
    pub face: BlockFace,
    pub hit_point: DVec3,
    pub distance: f64,
}

#[inline]
pub fn view_direction_from_angles(yaw_deg: f32, pitch_deg: f32) -> DVec3 {
    let yaw = (yaw_deg as f64).to_radians();
    let pitch = (pitch_deg as f64).to_radians();
    let cos_p = pitch.cos();
    DVec3::new(-yaw.sin() * cos_p, -pitch.sin(), yaw.cos() * cos_p).normalize()
}

/// Dò tia Voxel chính xác theo thuật toán Amanatides & Woo 3D DDA
pub fn raycast_voxel_world(
    world: &ClientWorldCache,
    origin: DVec3,
    direction: DVec3,
    max_reach: f64,
) -> Option<BlockHitResult> {
    let mut voxel = IVec3::new(
        origin.x.floor() as i32,
        origin.y.floor() as i32,
        origin.z.floor() as i32,
    );

    let step_x = if direction.x >= 0.0 { 1 } else { -1 };
    let step_y = if direction.y >= 0.0 { 1 } else { -1 };
    let step_z = if direction.z >= 0.0 { 1 } else { -1 };

    let t_delta_x = if direction.x.abs() < 1.0e-12 { f64::INFINITY } else { (1.0 / direction.x).abs() };
    let t_delta_y = if direction.y.abs() < 1.0e-12 { f64::INFINITY } else { (1.0 / direction.y).abs() };
    let t_delta_z = if direction.z.abs() < 1.0e-12 { f64::INFINITY } else { (1.0 / direction.z).abs() };

    let mut t_max_x = if direction.x >= 0.0 {
        ((voxel.x + 1) as f64 - origin.x) * t_delta_x
    } else {
        (origin.x - voxel.x as f64) * t_delta_x
    };
    let mut t_max_y = if direction.y >= 0.0 {
        ((voxel.y + 1) as f64 - origin.y) * t_delta_y
    } else {
        (origin.y - voxel.y as f64) * t_delta_y
    };
    let mut t_max_z = if direction.z >= 0.0 {
        ((voxel.z + 1) as f64 - origin.z) * t_delta_z
    } else {
        (origin.z - voxel.z as f64) * t_delta_z
    };

    let mut last_face = BlockFace::Top;
    let mut traveled = 0.0;

    while traveled <= max_reach {
        let state = world.get_block(voxel.x, voxel.y, voxel.z);
        if !state.is_air() {
            return Some(BlockHitResult {
                block_pos: voxel,
                face: last_face,
                hit_point: origin + direction * traveled,
                distance: traveled,
            });
        }

        if t_max_x < t_max_y {
            if t_max_x < t_max_z {
                traveled = t_max_x;
                voxel.x += step_x;
                t_max_x += t_delta_x;
                last_face = if step_x > 0 { BlockFace::West } else { BlockFace::East };
            } else {
                traveled = t_max_z;
                voxel.z += step_z;
                t_max_z += t_delta_z;
                last_face = if step_z > 0 { BlockFace::North } else { BlockFace::South };
            }
        } else if t_max_y < t_max_z {
            traveled = t_max_y;
            voxel.y += step_y;
            t_max_y += t_delta_y;
            last_face = if step_y > 0 { BlockFace::Bottom } else { BlockFace::Top };
        } else {
            traveled = t_max_z;
            voxel.z += step_z;
            t_max_z += t_delta_z;
            last_face = if step_z > 0 { BlockFace::North } else { BlockFace::South };
        }
    }

    None
}

/// Kiểm tra tia đánh PvP cắt qua Hitbox AABB của thực thể (Ray-Slab Intersection)
pub fn ray_intersects_aabb(origin: DVec3, dir: DVec3, box_aabb: &Aabb, max_reach: f64) -> Option<f64> {
    let inv_x = 1.0 / dir.x;
    let inv_y = 1.0 / dir.y;
    let inv_z = 1.0 / dir.z;

    let t1 = (box_aabb.min.x - origin.x) * inv_x;
    let t2 = (box_aabb.max.x - origin.x) * inv_x;
    let t3 = (box_aabb.min.y - origin.y) * inv_y;
    let t4 = (box_aabb.max.y - origin.y) * inv_y;
    let t5 = (box_aabb.min.z - origin.z) * inv_z;
    let t6 = (box_aabb.max.z - origin.z) * inv_z;

    let tmin = t1.min(t2).max(t3.min(t4)).max(t5.min(t6));
    let tmax = t1.max(t2).min(t3.max(t4)).min(t5.max(t6));

    if tmax < 0.0 || tmin > tmax || tmin > max_reach {
        None
    } else {
        Some(tmin.max(0.0))
    }
}
```

---

## PHẦN 3: BỘ PHÂN TÍCH VĂN BẢN CHAT & SCOREBOARD (`§` LEGACY + JSON COMPONENT)

Các Server Multiplayer sử dụng đồng thời hai định dạng văn bản:
1. **Mã màu ký tự `§` (Section Sign):** Phổ biến trên `1.12.2` và các plugin Spigot/Paper (`§a`, `§l`, `§#FF55AA` Hex Color).
2. **Cây đối tượng JSON Text Component:** Phổ biến trên `26.2` (`{"text": "...", "color": "gold", "bold": true, "extra": [...]}`).

### 3.1. Mã nguồn Bộ Phân Tích Văn Bản Đa Định Dạng (`crates/casx_core/src/chat_parser.rs`)

```rust
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledTextSpan {
    pub text: String,
    pub rgb: [u8; 3],
    pub bold: bool,
    pub italic: bool,
    pub underlined: bool,
    pub strikethrough: bool,
}

impl Default for StyledTextSpan {
    fn default() -> Self {
        Self {
            text: String::new(),
            rgb: [255, 255, 255],
            bold: false,
            italic: false,
            underlined: false,
            strikethrough: false,
        }
    }
}

pub fn legacy_code_to_rgb(code: char) -> Option<[u8; 3]> {
    match code.to_ascii_lowercase() {
        '0' => Some([0, 0, 0]),
        '1' => Some([0, 0, 170]),
        '2' => Some([0, 170, 0]),
        '3' => Some([0, 170, 170]),
        '4' => Some([170, 0, 0]),
        '5' => Some([170, 0, 170]),
        '6' => Some([255, 170, 0]),
        '7' => Some([170, 170, 170]),
        '8' => Some([85, 85, 85]),
        '9' => Some([85, 85, 255]),
        'a' => Some([85, 255, 85]),
        'b' => Some([85, 255, 255]),
        'c' => Some([255, 85, 85]),
        'd' => Some([255, 85, 255]),
        'e' => Some([255, 255, 85]),
        'f' => Some([255, 255, 255]),
        _ => None,
    }
}

pub fn named_color_to_rgb(name: &str) -> [u8; 3] {
    if let Some(hex) = name.strip_prefix('#') {
        if hex.len() == 6 {
            if let Ok(val) = u32::from_str_radix(hex, 16) {
                return [((val >> 16) & 0xFF) as u8, ((val >> 8) & 0xFF) as u8, (val & 0xFF) as u8];
            }
        }
    }
    match name {
        "black" => [0, 0, 0],
        "dark_blue" => [0, 0, 170],
        "dark_green" => [0, 170, 0],
        "dark_aqua" => [0, 170, 170],
        "dark_red" => [170, 0, 0],
        "dark_purple" => [170, 0, 170],
        "gold" => [255, 170, 0],
        "gray" => [170, 170, 170],
        "dark_gray" => [85, 85, 85],
        "blue" => [85, 85, 255],
        "green" => [85, 255, 85],
        "aqua" => [85, 255, 255],
        "red" => [255, 85, 85],
        "light_purple" => [255, 85, 255],
        "yellow" => [255, 255, 85],
        _ => [255, 255, 255],
    }
}

/// Phân tích chuỗi chứa mã màu `§` thành danh sách đoạn văn bản có màu
pub fn parse_legacy_section_text(input: &str, base_style: StyledTextSpan) -> Vec<StyledTextSpan> {
    let mut spans = Vec::new();
    let mut current = base_style;
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '§' {
            if let Some(&code) = chars.peek() {
                chars.next();
                if !current.text.is_empty() {
                    spans.push(current.clone());
                    current.text.clear();
                }
                if let Some(rgb) = legacy_code_to_rgb(code) {
                    current.rgb = rgb;
                    current.bold = false;
                    current.italic = false;
                    current.underlined = false;
                    current.strikethrough = false;
                } else {
                    match code.to_ascii_lowercase() {
                        'l' => current.bold = true,
                        'o' => current.italic = true,
                        'n' => current.underlined = true,
                        'm' => current.strikethrough = true,
                        'r' => current = StyledTextSpan::default(),
                        _ => {}
                    }
                }
                continue;
            }
        }
        current.text.push(ch);
    }

    if !current.text.is_empty() {
        spans.push(current);
    }
    spans
}

/// Phân tích chuỗi JSON Chat Component của Minecraft 1.12.2 & 26.2
pub fn parse_chat_component_json(raw_json: &str) -> Vec<StyledTextSpan> {
    let Ok(val) = serde_json::from_str::<Value>(raw_json) else {
        return parse_legacy_section_text(raw_json, StyledTextSpan::default());
    };
    let mut out = Vec::new();
    flatten_json_node(&val, StyledTextSpan::default(), &mut out);
    out
}

fn flatten_json_node(node: &Value, mut inherited: StyledTextSpan, out: &mut Vec<StyledTextSpan>) {
    match node {
        Value::String(s) => out.extend(parse_legacy_section_text(s, inherited)),
        Value::Array(arr) => {
            for item in arr {
                flatten_json_node(item, inherited.clone(), out);
            }
        }
        Value::Object(map) => {
            if let Some(color) = map.get("color").and_then(|v| v.as_str()) {
                inherited.rgb = named_color_to_rgb(color);
            }
            if let Some(b) = map.get("bold").and_then(|v| v.as_bool()) { inherited.bold = b; }
            if let Some(i) = map.get("italic").and_then(|v| v.as_bool()) { inherited.italic = i; }
            if let Some(u) = map.get("underlined").and_then(|v| v.as_bool()) { inherited.underlined = u; }
            if let Some(s) = map.get("strikethrough").and_then(|v| v.as_bool()) { inherited.strikethrough = s; }

            if let Some(text) = map.get("text").and_then(|v| v.as_str()) {
                out.extend(parse_legacy_section_text(text, inherited.clone()));
            }
            if let Some(extra) = map.get("extra").and_then(|v| v.as_array()) {
                for child in extra {
                    flatten_json_node(child, inherited.clone(), out);
                }
            }
        }
        _ => {}
    }
}
```

---

## PHẦN 4: HỆ THỐNG SERVER RESOURCE PACK & ÂM THANH KHÔNG GIAN 3D

### 4.1. Quy trình Tải & Bảo Mật Server Resource Pack (`.zip`)

Khi kết nối vào Server Multiplayer (đặc biệt các Server dùng ItemsAdder / Oraxen / Nexo để thêm vũ khí 3D, danh hiệu và giao diện), Server gửi gói `ResourcePackSend` (`0x34` trên `1.12.2`) hoặc `AddResourcePack` (`0x46` trên `26.2`) kèm đường dẫn URL, mã băm $\text{SHA-1}$ và cờ `forced: bool`:

1. **Kiểm tra Bộ nhớ đệm Cục bộ:** Tìm tệp `.local/server_packs/<sha1>.zip`. Nếu đã tồn tại và khớp mã băm, bỏ qua bước tải mạng và nạp ngay lập tức trong $< 15\text{ ms}$.
2. **Bảo vệ Chống Lỗ hổng ZipSlip (Path Traversal Prevention):** Khi giải nén các mục bên trong tệp `.zip`, mọi đường dẫn mục (`entry_name`) bắt buộc phải được chuẩn hóa và kiểm tra không chứa chuỗi `..` hoặc ký tự ổ đĩa gốc trước khi ghi hoặc nạp vào bộ nhớ.
3. **Phản hồi Trạng thái Lên Server (`ResourcePackStatus`):**
   * Gửi trạng thái `3` (`Accepted`) ngay khi bắt đầu tải.
   * Gửi trạng thái `0` (`Successfully Loaded`) sau khi ghép xong các tầng Texture mới vào `wgpu::Texture` để Server không tự động kick người chơi nếu bật chế độ bắt buộc.

### 4.2. Âm thanh Không gian 3D (Spatial Audio Pipeline với `kira`)

Để tái tạo âm thanh bước chân, tiếng đặt/đập khối và tiếng kiếm chém trong PvP theo đúng hướng không gian:
* Sử dụng crate **`kira`** chạy trên luồng âm thanh ưu tiên thời gian thực (Real-time Audio Thread).
* Tính toán độ suy giảm cường độ âm thanh theo khoảng cách $r = \|\mathbf{p}_{\text{sound}} - \mathbf{p}_{\text{ear}}\|$ và tầm nghe tối đa $R_{\max} = 16.0 \times \max(1.0, \text{volume})$:
  $$\text{Gain}(r) = \text{clamp}\left(1.0 - \frac{r}{R_{\max}}, \ 0.0, \ 1.0\right)$$
* Tính hệ số cân bằng kênh Trái/Phải ($\text{Pan} \in [-1.0, +1.0]$) bằng tích vô hướng giữa vector đơn vị hướng tới nguồn âm $\hat{\mathbf{u}}$ và vector hướng tai phải $\hat{\mathbf{r}}_{\text{ear}}$:
  $$\text{Pan} = \hat{\mathbf{u}} \cdot \hat{\mathbf{r}}_{\text{ear}}$$

---

## PHẦN 5: NGÂN SÁCH KHUNG HÌNH (FRAME-TIME BUDGET) & KẾ HOẠCH KIỂM THỬ TOÀN DIỆN

Để đạt được mục tiêu **$\ge 600\text{ FPS}$ ổn định** (tương đương tổng thời gian xử lý mỗi khung hình $\le 1.66\text{ ms}$) ở tầm nhìn $16\text{ Chunks}$ ($1.089\text{ Chunk Columns}$), mỗi phân hệ trong **CasxDuois Client** phải tuân thủ ngân sách thời gian nghiêm ngặt sau:

| Phân hệ Xử lý | Ngân sách Thời gian Tối đa / Frame | Kỹ thuật Tối ưu Bắt buộc |
| :--- | :--- | :--- |
| **1. Rút sự kiện Mạng & Vật lý** | $\le 0.12\text{ ms}$ | Kênh không khóa `crossbeam_channel::try_recv()`, không dùng `Mutex` khóa toàn bộ World. |
| **2. View-Frustum Culling** | $\le 0.08\text{ ms}$ | Lọc $6$ mặt phẳng tầm nhìn trên CPU bằng phép nhân vô hướng SIMD (`glam::Vec4`). |
| **3. Chụp Dữ liệu Mesh Mới (Rayon)** | $\le 0.15\text{ ms}$ | Tải tối đa $4\text{ Chunk Meshes}$ hoàn thiện lên VRAM mỗi khung hình để tránh nghẽn băng thông PCIe. |
| **4. Vẽ Thế giới 3D (`wgpu`)** | $\le 0.85\text{ ms}$ | Gộp lệnh vẽ bằng `multi_draw_indexed_indirect`, đỉnh nén $64\text{-bit}$ ($8\text{ bytes}$), Early-Z Rejection. |
| **5. Hậu kỳ Kawase Blur & UI (`egui`)** | $\le 0.40\text{ ms}$ | Chỉ chạy pass Kawase Blur khi mở Lunar Mod Menu (`Right Shift`) hoặc ở Sảnh HoYoPlay. |
| **Tổng cộng 1 Khung hình** | **$\le 1.60\text{ ms}$ ($\approx 625\text{ FPS}$)** | Đạt độ mượt tuyệt đối trên màn hình tần số quét cao $240\text{Hz} - 540\text{Hz}$. |