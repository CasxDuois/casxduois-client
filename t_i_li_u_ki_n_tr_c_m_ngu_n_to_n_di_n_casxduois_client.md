# TÀI LIỆU ĐẶC TẢ KỸ THUẬT & MÃ NGUỒN TOÀN DIỆN: CASXDUOIS CLIENT

* **Kiến trúc:** Rust-Native Multiplayer-Only Minecraft Engine
* **Phiên bản hỗ trợ:** Minecraft `1.12.2` (Protocol $340$) & Minecraft `26.2` (Protocol $770$ / Next-Gen)
* **Giao diện Hợp nhất:** $\text{HoYoPlay} \times \text{Legacy Launcher} \times \text{Lunar Client}$

---

## PHẦN 1: TRIẾT LÝ KỸ THUẬT & KIẾN TRÚC TỔNG THỂ (SYSTEM ARCHITECTURE)

### 1.1. Triết lý "Multiplayer-Only State-Driven Terminal"

Trong kiến trúc Minecraft truyền thống, ngay cả khi chơi đơn (Singleplayer), trò chơi vẫn phải khởi chạy một Integrated Server ngầm bên trong để tính toán sinh địa hình (World Generation), trí tuệ nhân tạo của thực thể (Mob AI), luồng chảy chất lỏng, mạch Redstone và đọc/ghi tệp vùng (Anvil `.mca`).

Vì **CasxDuois Client** được thiết kế **chỉ hỗ trợ chơi Server (Multiplayer-Only)** và viết mới hoàn toàn bằng **Rust**, chúng ta loại bỏ triệt để Integrated Server. Client hoạt động như một **Thiết bị đầu cuối hướng trạng thái hiệu năng cao (State-Driven Terminal)**:

1. **Không tốn RAM cho WorldGen & AI:** Tiết kiệm từ $60\%$ đến $75\%$ dung lượng bộ nhớ so với client thông thường. Dung lượng RAM tiêu thụ mục tiêu ở mức $250\text{ MB} - 600\text{ MB}$ ngay cả khi tải $16\text{ Render Distance}$.
2. **Không khởi tạo cửa sổ phụ (Zero-Process Spawn):** Launcher và Game Engine là **một tệp thực thi duy nhất (`.exe` / binary)**. Khi người dùng mở CasxDuois Client, ngữ cảnh đồ họa `wgpu` được khởi tạo ngay lập tức để hiển thị sảnh chờ HoYoPlay/Legacy. Khi nhấn **"KẾT NỐI SERVER"**, hệ thống chỉ cần mở luồng TCP bất đồng bộ và chuyển trạng thái Render từ 2D Hub sang 3D World ngay trong cùng một cửa sổ trong chưa đầy $100\text{ ms}$.

### 1.2. Mô hình Đa luồng Không Khóa (Lock-Free Multi-Threading Model)

Để đạt mức khung hình trên $500\text{ FPS}$ mà không bao giờ bị giật (stutter) khi nhận hàng loạt gói tin Chunk từ Server, CasxDuois Client chia tải công việc thành 4 cụm luồng độc lập giao tiếp qua kênh không khóa (`crossbeam_channel` và `tokio::sync::mpsc`):

* **Thread 1 - Main Render & UI Thread (`winit` + `wgpu` + `egui`):** Chạy theo tần số quét màn hình (VSync hoặc Unlimited FPS). Đọc bản chụp trạng thái thế giới, nội suy vị trí thực thể theo thời gian thực và gửi lệnh vẽ (Draw Calls) xuống GPU.
* **Thread 2 - Fixed Physics Tick Thread ($20\text{ TPS}$ / $\Delta t = 50\text{ ms}$):** Chạy bộ đếm thời gian cố định $20$ lần mỗi giây để xử lý đầu vào bàn phím/chuột của người chơi, giải bài toán va chạm hộp giới hạn $\text{AABB}$ với thế giới Voxel cục bộ và đẩy gói tin tọa độ sang luồng mạng.
* **Thread 3 - Async Network Runtime (`tokio`):** Đọc/ghi luồng socket TCP với `TCP_NODELAY`, giải mã luồng `AES-128-CFB8`, giải nén `zlib` và chạy bộ chuyển đổi giao thức (`proto_1_12_2` hoặc `proto_26_2`) để biến byte thô thành sự kiện chuẩn hóa `ServerEvent`.
* **Thread Pool 4 - Voxel Meshing Workers (`rayon`):** Khi nhận được dữ liệu Chunk mới hoặc cập nhật Block từ Server, các luồng Worker chạy thuật toán **6-Direction Greedy Meshing** trên nền CPU để tạo mảng đỉnh (`Vertex Buffer`) rồi tải trực tiếp lên VRAM của `wgpu::Buffer`.

### 1.3. Cấu trúc Thư mục Dự án (Cargo Workspace)

```text
casxduois-client/
├── Cargo.toml                      # Cấu hình Workspace tổng
├── assets/                         # Chứa Shaders (WGSL), Font UI, Icons HoYo/Lunar
└── crates/
    ├── casx_core/                  # Nhân trung tâm: World, Chunk, NBT, AES-CFB8, AABB, Entities
    │   ├── Cargo.toml
    │   └── src/lib.rs
    ├── proto_1_12_2/               # Bộ dịch giao thức 1.12.2 (Protocol 340, Block ID:Meta)
    │   ├── Cargo.toml
    │   └── src/lib.rs
    ├── proto_26_2/                 # Bộ dịch giao thức 26.2 (Protocol 770, Config State, Palette)
    │   ├── Cargo.toml
    │   └── src/lib.rs
    ├── casx_render/                # Engine đồ họa wgpu: 6-Face Greedy Mesher, Frustum, Kawase Blur
    │   ├── Cargo.toml
    │   └── src/lib.rs
    ├── casx_ui/                    # Hệ thống UI thực tế (egui): HoYoPlay Hub + Legacy + Lunar HUD
    │   ├── Cargo.toml
    │   └── src/lib.rs
    └── casx_client/                # Ứng dụng thực thi chính: winit + wgpu + tokio Runtime
        ├── Cargo.toml
        └── src/main.rs
```

---

## PHẦN 2: ĐẶC TẢ GIAO THỨC MẠNG ĐA PHIÊN BẢN (`1.12.2` VS `26.2`)

### 2.1. Khung Gói Tin (Packet Framing), Mã hóa `AES-128-CFB8` & Nén `Zlib`

Thứ tự xử lý luồng byte đi từ tầng thấp lên tầng cao của CasxDuois Client tuân theo quy trình chuẩn:
$$\text{TCP Socket} \xrightarrow{\text{AES-128-CFB8}} \text{Packet Framing} \xrightarrow{\text{Zlib Decompress}} \text{Packet ID + Payload}$$

* **Khi chưa bật nén (Uncompressed):**
  $$\text{Packet} = [\text{Packet Length (VarInt)}] \parallel [\text{Packet ID (VarInt)}] \parallel [\text{Payload Bytes}]$$
* **Khi đã bật nén (Sau gói `SetCompression` từ Server với ngưỡng $T_{\text{threshold}}$):**
  $$\text{Packet} = [\text{Packet Length (VarInt)}] \parallel [\text{Data Length (VarInt)}] \parallel [\text{Compressed/Raw Data}]$$
  * Nếu $\text{Data Length} = 0$: Payload phía sau **không nén**, bao gồm $[\text{Packet ID (VarInt)}] \parallel [\text{Payload}]$.
  * Nếu $\text{Data Length} \ge T_{\text{threshold}}$: Phần byte phía sau được nén bằng thuật toán `zlib` (Deflate chuẩn RFC 1950). Sau khi giải nén ra đúng $\text{Data Length}$ byte, ta mới đọc $[\text{Packet ID (VarInt)}] \parallel [\text{Payload}]$.

### 2.2. Đối chiếu Máy Trạng Thái (State Machine) Giữa `1.12.2` và `26.2`

| Giai đoạn | Phiên bản `1.12.2` (Protocol $340$) | Phiên bản `26.2` (Protocol $770$) |
| :--- | :--- | :--- |
| **1. Handshake** | Gửi `Handshake (0x00)` với $\text{Protocol} = 340$, $\text{NextState} = 2$. | Gửi `Handshake (0x00)` với $\text{Protocol} = 770$, $\text{NextState} = 2$. |
| **2. Login** | Gửi `LoginStart (0x00)` chỉ gồm `Username (String)`. Khi nhận `LoginSuccess (0x02)` (UUID dạng chuỗi có gạch nối), **chuyển thẳng sang `Play`**. | Gửi `LoginStart (0x00)` gồm `Username (String)` + `UUID (u128)`. Khi nhận `LoginSuccess (0x02)`, Client **bắt buộc phải gửi `LoginAcknowledged (0x03)`** và chuyển sang trạng thái **`Configuration`**. |
| **3. Configuration** | *Không tồn tại.* | Server gửi `RegistryData` (Dimension, Biome, ChatType), `SelectKnownPacks (0x0E)` và `FinishConfiguration (0x03)`. Client phải phản hồi `SelectKnownPacks (0x07)` và `AcknowledgeFinishConfiguration (0x03)` trước khi vào **`Play`**. |
| **4. Play (Vào Server)** | Nhận `JoinGame (0x23)` $\rightarrow$ `PlayerPositionAndLook (0x2F)` $\rightarrow$ phản hồi `TeleportConfirm (0x00)` + `PlayerPositionAndLook (0x0E)`. | Nhận `LoginPlay (0x2B)` $\rightarrow$ `SynchronizePlayerPosition (0x40)` $\rightarrow$ phản hồi `ConfirmTeleportation (0x00)` + `MovePlayerPosRot (0x1B)`. |

### 2.3. Định dạng Tọa độ Nén 64-Bit (`BlockPos`) & Binary NBT Giữa Hai Phiên Bản

1. **Đóng gói Tọa độ Khối 64-Bit (`Position`):**
   * **Trên `1.12.2`:** $X$ ($26\text{ bits}$ cao) $\mid$ $Y$ ($12\text{ bits}$ giữa) $\mid$ $Z$ ($26\text{ bits}$ thấp):
     $$\text{Pos}_{1.12.2} = ((x \ \& \ \text{0x3FFFFFF}) \ll 38) \mid ((y \ \& \ \text{0xFFF}) \ll 26) \mid (z \ \& \ \text{0x3FFFFFF})$$
   * **Trên `26.2`:** $X$ ($26\text{ bits}$ cao) $\mid$ $Z$ ($26\text{ bits}$ giữa) $\mid$ $Y$ ($12\text{ bits}$ thấp):
     $$\text{Pos}_{26.2} = ((x \ \& \ \text{0x3FFFFFF}) \ll 38) \mid ((z \ \& \ \text{0x3FFFFFF}) \ll 12) \mid (y \ \& \ \text{0xFFF})$$

2. **Định dạng Binary NBT (Named Binary Tag):**
   * **Trên `1.12.2`:** Thẻ gốc (`TAG_Compound = 0x0A`) **luôn có trường độ dài tên (2 bytes `u16`)** theo sau (thường là `0x0000` cho tên rỗng).
   * **Trên `26.2` (Network Nameless NBT):** Thẻ gốc (`TAG_Compound = 0x0A`) **bỏ qua hoàn toàn 2 bytes độ dài tên** và đi thẳng vào danh sách các thẻ con bên trong. Bộ giải mã của `casx_core` hỗ trợ cả hai chế độ thông qua cờ `nameless_root: bool`.

### 2.4. Giải mã Chunk & Chuẩn hóa BlockState (`CanonicalBlockState`)

1. **Trên `1.12.2` (Block ID $12\text{-bit}$ + Metadata $4\text{-bit}$):**
   $$\text{StateID}_{1.12.2} = (\text{BlockID} \ll 4) \mid (\text{Metadata} \ \& \ \text{0x0F})$$
   Các chỉ số bit trong Chunk Section ($16 \times 16 \times 16 = 4096\text{ blocks}$) **nối tràn qua ranh giới giữa hai số nguyên `u64` liên tiếp** (Spanning Longs). Với khối thứ $i \in [0, 4095]$ và số bit mỗi khối $b$:
   $$\text{BitIndex} = i \times b, \quad \text{StartLong} = \left\lfloor \frac{\text{BitIndex}}{64} \right\rfloor, \quad \text{EndLong} = \left\lfloor \frac{\text{BitIndex} + b - 1}{64} \right\rfloor, \quad \text{BitOffset} = \text{BitIndex} \bmod 64$$

2. **Trên `26.2` (Modern Flattened PalettedContainer):**
   Các chỉ số bit **không bao giờ tràn qua ranh giới `u64`** (Non-Spanning / Padded Longs). Số phần tử tối đa trong một từ $64\text{-bit}$ là:
   $$E_{\text{per\_long}} = \left\lfloor \frac{64}{b} \right\rfloor, \quad \text{LongIndex} = \left\lfloor \frac{i}{E_{\text{per\_long}}} \right\rfloor, \quad \text{BitOffset} = (i \bmod E_{\text{per\_long}}) \times b$$

---

## PHẦN 3: KIẾN TRÚC ĐỒ HỌA `wgpu` & VOXEL RENDERING PIPELINE

### 3.1. Nén Đỉnh 64-Bit (64-Bit Vertex Compression)

CasxDuois Client nén toàn bộ thông tin của 1 đỉnh Voxel vào đúng **1 số nguyên `u64` ($8\text{ bytes/đỉnh}$)** — giảm $83.3\%$ băng thông bộ nhớ GPU so với định dạng đỉnh không nén ($48\text{ bytes/đỉnh}$):

$$\text{PackedVertex}_{64} = \underbrace{X}_{6\text{b}} \mid \underbrace{(Y \ll 6)}_{6\text{b}} \mid \underbrace{(Z \ll 12)}_{6\text{b}} \mid \underbrace{(W \ll 18)}_{5\text{b}} \mid \underbrace{(H \ll 23)}_{5\text{b}} \mid \underbrace{(\text{Normal} \ll 28)}_{3\text{b}} \mid \underbrace{(\text{AO} \ll 31)}_{2\text{b}} \mid \underbrace{(\text{TexID} \ll 33)}_{16\text{b}} \mid \underbrace{(\text{Light} \ll 49)}_{8\text{b}}$$

### 3.2. Thuật toán Greedy Meshing Đầy Đủ 6 Hướng Mặt Khối & Voxel Ambient Occlusion

Để thế giới có chiều sâu tự nhiên mà không cần Raytracing nặng nề, mỗi đỉnh của mặt khối được tính mức **Voxel Ambient Occlusion ($\text{AO} \in \{0, 1, 2, 3\}$)** dựa trên 3 khối láng giềng tại góc đó ($\text{Side}_1, \text{Side}_2 \in \{0, 1\}$ và $\text{Corner} \in \{0, 1\}$):

$$\text{AO}(\text{Side}_1, \text{Side}_2, \text{Corner}) = \begin{cases} 0 & \text{nếu } \text{Side}_1 = 1 \text{ và } \text{Side}_2 = 1 \\ 3 - (\text{Side}_1 + \text{Side}_2 + \text{Corner}) & \text{trong các trường hợp còn lại} \end{cases}$$

Hai mặt khối liền kề chỉ được phép gộp chung thành một Greedy Quad khi chúng có **cùng loại khối**, **cùng chỉ số Texture**, và **cùng giá trị $\text{AO}$ tại cả 4 góc** để tránh hiện tượng đổ bóng sai lệch (AO Flipping).

### 3.3. Frustum Culling 6 Mặt Phẳng (View-Frustum Chunk Culling)

Trước khi gửi lệnh vẽ một Chunk Section có hộp giới hạn $[\mathbf{b}_{\min}, \mathbf{b}_{\max}]$ lên GPU, CPU trích xuất $6$ mặt phẳng tầm nhìn $\pi_k = (\mathbf{n}_k, d_k)$ từ ma trận $\mathbf{M}_{\text{view\_proj}}$ (phương pháp Gribb-Hartmann). Nếu tồn tại bất kỳ mặt phẳng nào thỏa mãn:

$$\max_{x \in \{x_{\min}, x_{\max}\}} (n_{k,x} x) + \max_{y \in \{y_{\min}, y_{\max}\}} (n_{k,y} y) + \max_{z \in \{z_{\min}, z_{\max}\}} (n_{k,z} z) + d_k < 0$$

thì toàn bộ Chunk Section đó nằm ngoài tầm nhìn Camera và bị loại bỏ ngay lập tức trên CPU.

### 3.4. Shader Hậu Kỳ Dual-Filter Kawase Blur (Cho Lunar Overlay & HoYoPlay Hub)

Để tạo hiệu ứng kính mờ (Frosted Glass) tốc độ cao cho bảng điều khiển `Right Shift` của Lunar Client mà không làm tụt FPS như Gaussian Blur truyền thống ($O(N^2)$), CasxDuois Client sử dụng **Dual-Filter Kawase Blur** chạy qua 2 pha:
* **Downsample Pass:** Lấy mẫu $5$ điểm (tâm và $4$ góc nửa pixel $\pm 0.5\mathbf{t}_{\text{half}}$) để giảm độ phân giải khung hình xuống $1/2$ và $1/4$.
* **Upsample Pass:** Lấy mẫu $8$ điểm xung quanh với trọng số đối xứng để khôi phục độ phân giải mượt mà mà không xuất hiện dải màu (Banding).

---

## PHẦN 4: HỆ THỐNG VẬT LÝ $20\text{ TPS}$, THỰC THỂ & TƯƠNG TÁC CONTAINER

### 4.1. Vật lý Người chơi Chính ($\text{AABB}$ Collision $20\text{ TPS}$)

Hộp giới hạn người chơi $\text{AABB}(\mathbf{p}) = [p_x - 0.3, p_x + 0.3] \times [p_y, p_y + 1.8] \times [p_z - 0.3, p_z + 0.3]$ được giải va chạm theo thứ tự trục $Y \rightarrow X \rightarrow Z$ với vận tốc cập nhật mỗi Tick ($\Delta t = 50\text{ ms}$):

$$v_y^{(t+1)} = \left(v_y^{(t)} - 0.08\right) \times 0.9800000190734863, \quad v_{xz}^{(t+1)} = v_{xz}^{(t)} \times D_{xz}$$

### 4.2. Bộ Theo Dõi Thực Thể & Nội Suy Vị Trí Từ Xa (Remote Entity Interpolation)

Trên mạng Multiplayer, Server gửi vị trí người chơi khác và quái vật thông qua các gói tin tọa độ tương đối (`EntityRelativeMove`, độ phân giải $1/4096\text{ block}$) hoặc tọa độ tuyệt đối (`EntityTeleport`). Mỗi thực thể được lưu một bộ đệm mục tiêu $(\mathbf{p}_{\text{start}}, \mathbf{p}_{\text{target}})$ và nội suy mượt trong $3\text{ ticks}$ ($150\text{ ms}$) để triệt tiêu hoàn toàn hiện tượng giật cục khi mạng dao động (Jitter).

### 4.3. Quản Lý Kho Đồ & Giao Diện Rương/Menu Server (`Inventory & Container Window`)

Hầu hết các Server Multiplayer (`1.12.2` và `26.2`) sử dụng Chest GUI (Rương ảo) làm Menu chọn chế độ chơi, Shop, hoặc Hệ thống Tu Luyện/Kỹ Năng. CasxDuois Client duy trì `ContainerWindow` đồng bộ thông qua mã giao dịch (`ActionNumber` trên `1.12.2` và `StateID` trên `26.2`) để người chơi bấm vào các vật phẩm trong Menu Server mượt mà không bị lỗi kẹt vật phẩm (Ghost Items).

---

## PHẦN 5: ĐẶC TẢ GIAO DIỆN HỢP NHẤT ($\text{HOYOPLAY} \times \text{LEGACY} \times \text{LUNAR}$)

### 5.1. Sơ đồ Kiến trúc Giao diện Thống nhất

```text
+-----------------------------------------------------------------------------------+
| [CASXDUOIS CLIENT v1.0]                                     [_] [Square] [X Close]|
|-------+---------------------------------------------------------------------------|
|       |                                                                           |
| (H)   |   [HOYOPLAY DYNAMIC BACKGROUND: SHADER PANORAMA / SERVER LIVE SCENE]      |
| Hub   |                                                                           |
|       |   +-----------------------------------+                                   |
| (M)   |   | TIN TỨC & TRẠNG THÁI MÁY CHỦ      |                                   |
| Lunar |   | - Ping hiện tại: 14ms (Hà Nội)    |                                   |
| Mods  |   | - Sự kiện Server đang diễn ra     |                                   |
|       |   | - Cập nhật Resource Pack tự động  |                                   |
| (R)   |   +-----------------------------------+                                   |
| Packs |                                                                           |
|       |                                                                           |
| (S)   |---------------------------------------------------------------------------|
| Cài   | [LEGACY QUICK BAR]                         [HOYOPLAY MAIN CTA]            |
| đặt   | Account: [CasxDuois v] | Auth: [Offline/MS]  +--------------------------+ |
|       | Server : [play.server.vn:25565        ]      |     KẾT NỐI SERVER       | |
|       | Kernel : (o) 26.2 Next-Gen  ( ) 1.12.2       |   [Chuyển cảnh < 50ms]   | |
+-------+---------------------------------------------------------------------------+
```

### 5.2. Hệ thống Lunar Client HUD & Bảng Điều Khiển `Right Shift`

* **Kéo-thả Từ tính (Magnetic Grid Snapping):** Tự động hút về đường trục giữa màn hình khi khoảng cách $\Delta d \le 6.0\text{ px}$.
* **Các Module Tích hợp Sẵn:**
  1. `FPS & Ping Counter`
  2. `Keystrokes + LMB/RMB Rolling CPS Counter` ($1000\text{ ms}$ sliding window)
  3. `Custom Scoreboard` (Tùy chọn ẩn cột số đỏ bên phải, bo góc kính mờ)
  4. `Armor & Cooldown HUD`
  5. `Freelook 360` (Giữ phím `Alt` xoay camera độc lập với hướng gửi lên Server) & `ToggleSprint`.

---

## PHẦN 6: LỘ TRÌNH PHÁT TRIỂN CHI TIẾT 12 TUẦN (ENGINEERING ROADMAP)

| Giai đoạn | Tuần | Hạng mục Công việc Cụ thể | Tiêu chí Nghiệm thu (Deliverables & KPIs) |
| :--- | :--- | :--- | :--- |
| **Phase 1: Core, NBT & Dual-Protocol** | Tuần 1 - 2 | Hoàn thiện `casx_core` (NBT Classic/Nameless, Zlib, AES-CFB8, UUIDv3) và hai crate `proto_1_12_2` + `proto_26_2`. | Kết nối đồng thời vào Server `1.12.2` và `26.2`, hoàn tất Login/Config/KeepAlive, đọc được Chat Component và NBT. |
| **Phase 2: 6-Face Mesher & AO** | Tuần 3 - 4 | Giải nén mảng bit `u64` của Chunk `1.12.2` và `26.2`. Cài đặt 6-Direction Greedy Meshing đa luồng + Ambient Occlusion. | Tạo lưới đầy đủ $6$ hướng cho $1$ Chunk Column ($16 \times 384 \times 16$) trong thời gian $< 0.8\text{ ms}$ trên CPU. |
| **Phase 3: wgpu Engine & Kawase Blur** | Tuần 5 - 6 | Tích hợp `winit` + `wgpu`, Frustum Culling $6$ mặt phẳng, Texture Array và Shader hậu kỳ Dual-Filter Kawase Blur. | Render tầm nhìn $16\text{ Chunks}$ ở tốc độ $\ge 500\text{ FPS}$ trên GPU tầm trung, VRAM $< 400\text{ MB}$. |
| **Phase 4: Physics, Entities & GUI** | Tuần 7 - 8 | Cài đặt vật lý $20\text{ TPS}$, nội suy người chơi từ xa (`EntityTracker`), đồng bộ `ContainerWindow` (Chest Menu). | Di chuyển vượt qua GrimAC/Vulcan với $0$ cảnh báo; thao tác mượt mà trên Menu Rương của Server. |
| **Phase 5: HoYo $\times$ Legacy $\times$ Lunar UI** | Tuần 9 - 10 | Xây dựng giao diện `egui` thực tế: HoYoPlay Hub, thanh chuyển nhanh Legacy Launcher, bảng Mod `Right Shift` kéo-thả. | Chuyển đổi qua lại giữa `1.12.2` và `26.2` ngay trên thanh Legacy và kết nối vào Server trong $< 0.5\text{ giây}$. |
| **Phase 6: Audio, Resource Pack & Release** | Tuần 11 - 12 | Tự động tải & giải nén Server Resource Pack (`.zip`), âm thanh không gian (`kira`), tối ưu LTO biên dịch tệp thực thi. | Đóng gói tệp thực thi `.exe` độc lập duy nhất với dung lượng $< 15\text{ MB}$ (`strip = true`, `lto = "fat"`). |

---

## PHẦN 7: TOÀN BỘ MÃ NGUỒN HOÀN CHỈNH (PRODUCTION-READY RUST WORKSPACE)

### 7.1. Tệp Cấu hình Gốc: `Cargo.toml`

```toml
[workspace]
members = [
    "crates/casx_core",
    "crates/proto_1_12_2",
    "crates/proto_26_2",
    "crates/casx_render",
    "crates/casx_ui",
    "crates/casx_client",
]
resolver = "2"

[workspace.dependencies]
bytes = "1.7"
tokio = { version = "1.40", features = ["net", "io-util", "rt-multi-thread", "sync", "macros", "time"] }
thiserror = "1.0"
flate2 = "1.0"
rayon = "1.10"
crossbeam-channel = "0.5"
glam = "0.29"
bytemuck = { version = "1.18", features = ["derive"] }
aes = "0.8"
cfb8 = "0.8"
md-5 = "0.10"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
winit = "0.30"
wgpu = "22.1"
egui = "0.29"
egui-winit = "0.29"
egui-wgpu = "0.29"
pollster = "0.3"
log = "0.4"
env_logger = "0.11"

[profile.release]
opt-level = 3
lto = "fat"
codegen-units = 1
strip = true
```

---

### 7.2. Crate 1: `crates/casx_core` (World, NBT, AES-128-CFB8, Physics, Entities & Containers)

#### `crates/casx_core/Cargo.toml`

```toml
[package]
name = "casx_core"
version = "0.1.0"
edition = "2021"

[dependencies]
bytes.workspace = true
thiserror.workspace = true
flate2.workspace = true
glam.workspace = true
aes.workspace = true
cfb8.workspace = true
md-5.workspace = true
serde.workspace = true
serde_json.workspace = true
```

#### `crates/casx_core/src/lib.rs`

```rust
use aes::cipher::{AsyncStreamCipher, KeyIvInit};
use bytes::{Buf, BufMut, BytesMut};
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use glam::DVec3;
use md5::{Digest, Md5};
use std::collections::HashMap;
use std::io::{Read, Write};
use thiserror::Error;

type Aes128Cfb8Enc = cfb8::Encryptor<aes::Aes128>;
type Aes128Cfb8Dec = cfb8::Decryptor<aes::Aes128>;

// ============================================================================
// 1. ĐỊNH DANH PHIÊN BẢN, TRẠNG THÁI MẠNG & OFFLINE UUIDV3
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameVersion {
    V1_12_2, // Protocol 340
    V26_2,   // Protocol 770 (Modern 26.2)
}

impl GameVersion {
    pub fn display_name(&self) -> &'static str {
        match self {
            GameVersion::V1_12_2 => "Rust MC 1.12.2 (Protocol 340)",
            GameVersion::V26_2 => "Rust MC 26.2 (Next-Gen Protocol 770)",
        }
    }

    pub fn protocol_id(&self) -> i32 {
        match self {
            GameVersion::V1_12_2 => 340,
            GameVersion::V26_2 => 770,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Handshaking,
    Login,
    Configuration, // Dành riêng cho 26.2
    Play,
    Disconnected,
}

/// Tạo UUID phiên bản 3 chuẩn Minecraft Offline ("OfflinePlayer:<username>")
pub fn generate_offline_uuid(username: &str) -> u128 {
    let mut hasher = Md5::new();
    hasher.update(format!("OfflinePlayer:{username}").as_bytes());
    let mut bytes: [u8; 16] = hasher.finalize().into();
    bytes[6] = (bytes[6] & 0x0F) | 0x30; // Version 3
    bytes[8] = (bytes[8] & 0x3F) | 0x80; // Variant RFC 4122
    u128::from_be_bytes(bytes)
}

// ============================================================================
// 2. BỘ MÃ HÓA LUỒNG MẠNG AES-128-CFB8 (CHO ONLINE / ELY.BY AUTH)
// ============================================================================

pub struct StreamCipherPair {
    encryptor: Aes128Cfb8Enc,
    decryptor: Aes128Cfb8Dec,
}

impl StreamCipherPair {
    pub fn new(shared_secret: &[u8; 16]) -> Self {
        Self {
            encryptor: Aes128Cfb8Enc::new_from_slices(shared_secret, shared_secret)
                .expect("Khóa AES-128 hợp lệ"),
            decryptor: Aes128Cfb8Dec::new_from_slices(shared_secret, shared_secret)
                .expect("Khóa AES-128 hợp lệ"),
        }
    }

    #[inline]
    pub fn encrypt_in_place(&mut self, data: &mut [u8]) {
        self.encryptor.encrypt(data);
    }

    #[inline]
    pub fn decrypt_in_place(&mut self, data: &mut [u8]) {
        self.decryptor.decrypt(data);
    }
}

// ============================================================================
// 3. BỘ GIẢI MÃ BINARY NBT (HỖ TRỢ CẢ 1.12.2 VÀ NAMELESS NBT 26.2)
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum NbtTag {
    End,
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    ByteArray(Vec<i8>),
    String(String),
    List(Vec<NbtTag>),
    Compound(HashMap<String, NbtTag>),
    IntArray(Vec<i32>),
    LongArray(Vec<i64>),
}

impl NbtTag {
    /// Giải mã thẻ gốc NBT từ luồng byte:
    /// - `nameless_root = false`: Chuẩn 1.12.2 (có 2 byte độ dài tên ở thẻ gốc)
    /// - `nameless_root = true` : Chuẩn Network NBT 26.2 (bỏ qua tên ở thẻ gốc)
    pub fn decode_root(buf: &mut impl Buf, nameless_root: bool) -> Result<(String, NbtTag), ProtocolError> {
        if !buf.has_remaining() {
            return Err(ProtocolError::Incomplete);
        }
        let tag_type = buf.get_u8();
        if tag_type == 0 {
            return Ok((String::new(), NbtTag::End));
        }
        let root_name = if nameless_root {
            String::new()
        } else {
            Self::read_nbt_string(buf)?
        };
        let payload = Self::decode_payload(buf, tag_type)?;
        Ok((root_name, payload))
    }

    fn read_nbt_string(buf: &mut impl Buf) -> Result<String, ProtocolError> {
        if buf.remaining() < 2 {
            return Err(ProtocolError::Incomplete);
        }
        let len = buf.get_u16() as usize;
        if buf.remaining() < len {
            return Err(ProtocolError::Incomplete);
        }
        let mut raw = vec![0u8; len];
        buf.copy_to_slice(&mut raw);
        Ok(String::from_utf8_lossy(&raw).into_owned())
    }

    fn decode_payload(buf: &mut impl Buf, tag_type: u8) -> Result<NbtTag, ProtocolError> {
        match tag_type {
            0 => Ok(NbtTag::End),
            1 => {
                if buf.remaining() < 1 { return Err(ProtocolError::Incomplete); }
                Ok(NbtTag::Byte(buf.get_i8()))
            }
            2 => {
                if buf.remaining() < 2 { return Err(ProtocolError::Incomplete); }
                Ok(NbtTag::Short(buf.get_i16()))
            }
            3 => {
                if buf.remaining() < 4 { return Err(ProtocolError::Incomplete); }
                Ok(NbtTag::Int(buf.get_i32()))
            }
            4 => {
                if buf.remaining() < 8 { return Err(ProtocolError::Incomplete); }
                Ok(NbtTag::Long(buf.get_i64()))
            }
            5 => {
                if buf.remaining() < 4 { return Err(ProtocolError::Incomplete); }
                Ok(NbtTag::Float(buf.get_f32()))
            }
            6 => {
                if buf.remaining() < 8 { return Err(ProtocolError::Incomplete); }
                Ok(NbtTag::Double(buf.get_f64()))
            }
            7 => {
                if buf.remaining() < 4 { return Err(ProtocolError::Incomplete); }
                let len = buf.get_i32() as usize;
                if buf.remaining() < len { return Err(ProtocolError::Incomplete); }
                let mut arr = Vec::with_capacity(len);
                for _ in 0..len { arr.push(buf.get_i8()); }
                Ok(NbtTag::ByteArray(arr))
            }
            8 => Ok(NbtTag::String(Self::read_nbt_string(buf)?)),
            9 => {
                if buf.remaining() < 5 { return Err(ProtocolError::Incomplete); }
                let elem_type = buf.get_u8();
                let len = buf.get_i32() as usize;
                let mut list = Vec::with_capacity(len);
                for _ in 0..len {
                    list.push(Self::decode_payload(buf, elem_type)?);
                }
                Ok(NbtTag::List(list))
            }
            10 => {
                let mut map = HashMap::new();
                loop {
                    if !buf.has_remaining() { return Err(ProtocolError::Incomplete); }
                    let child_type = buf.get_u8();
                    if child_type == 0 {
                        break;
                    }
                    let name = Self::read_nbt_string(buf)?;
                    let val = Self::decode_payload(buf, child_type)?;
                    map.insert(name, val);
                }
                Ok(NbtTag::Compound(map))
            }
            11 => {
                if buf.remaining() < 4 { return Err(ProtocolError::Incomplete); }
                let len = buf.get_i32() as usize;
                if buf.remaining() < len * 4 { return Err(ProtocolError::Incomplete); }
                let mut arr = Vec::with_capacity(len);
                for _ in 0..len { arr.push(buf.get_i32()); }
                Ok(NbtTag::IntArray(arr))
            }
            12 => {
                if buf.remaining() < 4 { return Err(ProtocolError::Incomplete); }
                let len = buf.get_i32() as usize;
                if buf.remaining() < len * 8 { return Err(ProtocolError::Incomplete); }
                let mut arr = Vec::with_capacity(len);
                for _ in 0..len { arr.push(buf.get_i64()); }
                Ok(NbtTag::LongArray(arr))
            }
            other => Err(ProtocolError::MalformedPacket(format!("Mã thẻ NBT không hợp lệ: {other}"))),
        }
    }
}

// ============================================================================
// 4. DỮ LIỆU KHỐI & CHUNK CHUẨN HÓA (CANONICAL WORLD STORAGE)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct CanonicalBlockState(pub u32);

impl CanonicalBlockState {
    pub const AIR: Self = Self(0);

    #[inline]
    pub fn is_air(self) -> bool {
        self.0 == 0
    }

    #[inline]
    pub fn is_opaque_solid(self) -> bool {
        !self.is_air() && (self.0 >> 4) != 20 && (self.0 >> 4) != 8 && (self.0 >> 4) != 9
    }

    #[inline]
    pub fn texture_layer_id(self) -> u16 {
        (self.0 & 0xFFFF) as u16
    }
}

#[derive(Clone)]
pub struct ChunkSection {
    pub blocks: Box<[CanonicalBlockState; 4096]>,
    pub non_air_count: u16,
}

impl Default for ChunkSection {
    fn default() -> Self {
        Self::new_empty()
    }
}

impl ChunkSection {
    pub fn new_empty() -> Self {
        Self {
            blocks: Box::new([CanonicalBlockState::AIR; 4096]),
            non_air_count: 0,
        }
    }

    #[inline]
    pub const fn index(x: usize, y: usize, z: usize) -> usize {
        ((y & 0x0F) << 8) | ((z & 0x0F) << 4) | (x & 0x0F)
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> CanonicalBlockState {
        self.blocks[Self::index(x, y, z)]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, state: CanonicalBlockState) {
        let idx = Self::index(x, y, z);
        let old = self.blocks[idx];
        if old.is_air() && !state.is_air() {
            self.non_air_count = self.non_air_count.saturating_add(1);
        } else if !old.is_air() && state.is_air() {
            self.non_air_count = self.non_air_count.saturating_sub(1);
        }
        self.blocks[idx] = state;
    }
}

#[derive(Clone)]
pub struct ChunkColumn {
    pub chunk_x: i32,
    pub chunk_z: i32,
    pub min_y: i32, // 0 trên 1.12.2, -64 trên 26.2
    pub sections: Vec<Option<ChunkSection>>,
    pub dirty_mesh: bool,
}

impl ChunkColumn {
    pub fn new(chunk_x: i32, chunk_z: i32, min_y: i32, section_count: usize) -> Self {
        Self {
            chunk_x,
            chunk_z,
            min_y,
            sections: vec![None; section_count],
            dirty_mesh: true,
        }
    }
}

#[derive(Default)]
pub struct ClientWorldCache {
    pub chunks: HashMap<(i32, i32), ChunkColumn>,
}

impl ClientWorldCache {
    pub fn insert_chunk(&mut self, column: ChunkColumn) {
        self.chunks.insert((column.chunk_x, column.chunk_z), column);
    }

    pub fn set_block(&mut self, world_x: i32, world_y: i32, world_z: i32, state: CanonicalBlockState) {
        let chunk_x = world_x.div_euclid(16);
        let chunk_z = world_z.div_euclid(16);

        if let Some(column) = self.chunks.get_mut(&(chunk_x, chunk_z)) {
            let rel_y = world_y - column.min_y;
            if rel_y >= 0 {
                let section_idx = (rel_y as usize) / 16;
                if section_idx < column.sections.len() {
                    let section = column.sections[section_idx].get_or_insert_with(ChunkSection::new_empty);
                    let lx = world_x.rem_euclid(16) as usize;
                    let ly = (rel_y as usize) % 16;
                    let lz = world_z.rem_euclid(16) as usize;
                    section.set(lx, ly, lz, state);
                    column.dirty_mesh = true;
                }
            }
        }
    }

    pub fn get_block(&self, world_x: i32, world_y: i32, world_z: i32) -> CanonicalBlockState {
        let chunk_x = world_x.div_euclid(16);
        let chunk_z = world_z.div_euclid(16);

        let Some(column) = self.chunks.get(&(chunk_x, chunk_z)) else {
            return CanonicalBlockState::AIR;
        };

        let rel_y = world_y - column.min_y;
        if rel_y < 0 {
            return CanonicalBlockState::AIR;
        }
        let section_idx = (rel_y as usize) / 16;
        if section_idx >= column.sections.len() {
            return CanonicalBlockState::AIR;
        }

        let Some(section) = &column.sections[section_idx] else {
            return CanonicalBlockState::AIR;
        };

        let local_x = world_x.rem_euclid(16) as usize;
        let local_y = (rel_y as usize) % 16;
        let local_z = world_z.rem_euclid(16) as usize;

        section.get(local_x, local_y, local_z)
    }
}

// ============================================================================
// 5. HỆ THỐNG THỰC THỂ TỪ XA (ENTITY INTERPOLATION) & CONTAINER GUI
// ============================================================================

#[derive(Debug, Clone)]
pub struct RemoteEntity {
    pub entity_id: i32,
    pub name: String,
    pub start_pos: DVec3,
    pub target_pos: DVec3,
    pub current_render_pos: DVec3,
    pub yaw: f32,
    pub pitch: f32,
    pub interp_steps: u8,
}

impl RemoteEntity {
    pub fn new(entity_id: i32, name: String, pos: DVec3, yaw: f32, pitch: f32) -> Self {
        Self {
            entity_id,
            name,
            start_pos: pos,
            target_pos: pos,
            current_render_pos: pos,
            yaw,
            pitch,
            interp_steps: 0,
        }
    }

    pub fn push_network_target(&mut self, new_target: DVec3, yaw: f32, pitch: f32) {
        self.start_pos = self.current_render_pos;
        self.target_pos = new_target;
        self.yaw = yaw;
        self.pitch = pitch;
        self.interp_steps = 3; // Nội suy mượt trong 3 ticks (150ms)
    }

    pub fn tick_interpolation(&mut self) {
        if self.interp_steps > 0 {
            let t = 1.0 / (self.interp_steps as f64);
            self.current_render_pos = self.current_render_pos.lerp(self.target_pos, t);
            self.interp_steps -= 1;
        }
    }
}

#[derive(Default)]
pub struct EntityTracker {
    pub entities: HashMap<i32, RemoteEntity>,
}

impl EntityTracker {
    pub fn tick_all(&mut self) {
        for ent in self.entities.values_mut() {
            ent.tick_interpolation();
        }
    }
}

#[derive(Debug, Clone)]
pub struct ItemStack {
    pub item_id: i32,
    pub count: u8,
    pub metadata: i16,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ContainerWindow {
    pub window_id: u8,
    pub title: String,
    pub slot_count: usize,
    pub slots: Vec<Option<ItemStack>>,
    pub state_id: i32, // Dùng cho 26.2
    pub next_action_id: i16, // Dùng cho 1.12.2
}

impl ContainerWindow {
    pub fn new(window_id: u8, title: String, slot_count: usize) -> Self {
        Self {
            window_id,
            title,
            slot_count,
            slots: vec![None; slot_count],
            state_id: 0,
            next_action_id: 1,
        }
    }
}

// ============================================================================
// 6. HỆ THỐNG VẬT LÝ AABB 20 TPS & NỘI SUY KHUNG HÌNH
// ============================================================================

#[derive(Debug, Clone, Copy)]
pub struct Aabb {
    pub min: DVec3,
    pub max: DVec3,
}

impl Aabb {
    pub fn from_player_pos(pos: DVec3) -> Self {
        Self {
            min: DVec3::new(pos.x - 0.3, pos.y, pos.z - 0.3),
            max: DVec3::new(pos.x + 0.3, pos.y + 1.8, pos.z + 0.3),
        }
    }

    pub fn offset(&self, delta: DVec3) -> Self {
        Self { min: self.min + delta, max: self.max + delta }
    }

    pub fn expand(&self, delta: DVec3) -> Self {
        let mut min = self.min;
        let mut max = self.max;
        if delta.x < 0.0 { min.x += delta.x; } else { max.x += delta.x; }
        if delta.y < 0.0 { min.y += delta.y; } else { max.y += delta.y; }
        if delta.z < 0.0 { min.z += delta.z; } else { max.z += delta.z; }
        Self { min, max }
    }

    pub fn clip_y_collide(&self, other: &Aabb, mut dy: f64) -> f64 {
        if other.max.x > self.min.x && other.min.x < self.max.x && other.max.z > self.min.z && other.min.z < self.max.z {
            if dy > 0.0 && other.max.y <= self.min.y {
                let d = self.min.y - other.max.y;
                if d < dy { dy = d; }
            } else if dy < 0.0 && other.min.y >= self.max.y {
                let d = self.max.y - other.min.y;
                if d > dy { dy = d; }
            }
        }
        dy
    }

    pub fn clip_x_collide(&self, other: &Aabb, mut dx: f64) -> f64 {
        if other.max.y > self.min.y && other.min.y < self.max.y && other.max.z > self.min.z && other.min.z < self.max.z {
            if dx > 0.0 && other.max.x <= self.min.x {
                let d = self.min.x - other.max.x;
                if d < dx { dx = d; }
            } else if dx < 0.0 && other.min.x >= self.max.x {
                let d = self.max.x - other.min.x;
                if d > dx { dx = d; }
            }
        }
        dx
    }

    pub fn clip_z_collide(&self, other: &Aabb, mut dz: f64) -> f64 {
        if other.max.x > self.min.x && other.min.x < self.max.x && other.max.y > self.min.y && other.min.y < self.max.y {
            if dz > 0.0 && other.max.z <= self.min.z {
                let d = self.min.z - other.max.z;
                if d < dz { dz = d; }
            } else if dz < 0.0 && other.min.z >= self.max.z {
                let d = self.max.z - other.min.z;
                if d > dz { dz = d; }
            }
        }
        dz
    }
}

#[derive(Debug, Clone)]
pub struct LocalPlayerPhysics {
    pub prev_pos: DVec3,
    pub curr_pos: DVec3,
    pub velocity: DVec3,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub sprinting: bool,
}

impl Default for LocalPlayerPhysics {
    fn default() -> Self {
        Self {
            prev_pos: DVec3::new(0.0, 80.0, 0.0),
            curr_pos: DVec3::new(0.0, 80.0, 0.0),
            velocity: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            on_ground: false,
            sprinting: false,
        }
    }
}

impl LocalPlayerPhysics {
    pub fn step_physics_tick(&mut self, world: &ClientWorldCache, forward_input: f64, strafe_input: f64, jump: bool) {
        self.prev_pos = self.curr_pos;

        if self.velocity.x.abs() < 0.003 { self.velocity.x = 0.0; }
        if self.velocity.y.abs() < 0.003 { self.velocity.y = 0.0; }
        if self.velocity.z.abs() < 0.003 { self.velocity.z = 0.0; }

        if jump && self.on_ground {
            self.velocity.y = 0.42;
            if self.sprinting {
                let rad = (self.yaw as f64).to_radians();
                self.velocity.x -= rad.sin() * 0.2;
                self.velocity.z += rad.cos() * 0.2;
            }
        }

        let slipperiness = if self.on_ground { 0.6 } else { 1.0 };
        let friction = slipperiness * 0.91;
        let base_speed = if self.sprinting { 0.13 } else { 0.1 };
        let accel = if self.on_ground {
            base_speed * (0.16277136 / (friction * friction * friction))
        } else {
            0.02
        };

        let dist_sq = forward_input * forward_input + strafe_input * strafe_input;
        if dist_sq >= 1.0e-4 {
            let dist = dist_sq.sqrt().max(1.0);
            let f = forward_input / dist * accel;
            let s = strafe_input / dist * accel;
            let rad = (self.yaw as f64).to_radians();
            let sin_yaw = rad.sin();
            let cos_yaw = rad.cos();
            self.velocity.x += s * cos_yaw - f * sin_yaw;
            self.velocity.z += f * cos_yaw + s * sin_yaw;
        }

        let mut player_box = Aabb::from_player_pos(self.curr_pos);
        let search_box = player_box.expand(self.velocity);
        let mut block_boxes = Vec::new();

        let min_bx = search_box.min.x.floor() as i32 - 1;
        let max_bx = search_box.max.x.ceil() as i32 + 1;
        let min_by = search_box.min.y.floor() as i32 - 1;
        let max_by = search_box.max.y.ceil() as i32 + 1;
        let min_bz = search_box.min.z.floor() as i32 - 1;
        let max_bz = search_box.max.z.ceil() as i32 + 1;

        for bx in min_bx..=max_bx {
            for by in min_by..=max_by {
                for bz in min_bz..=max_bz {
                    if world.get_block(bx, by, bz).is_opaque_solid() {
                        block_boxes.push(Aabb {
                            min: DVec3::new(bx as f64, by as f64, bz as f64),
                            max: DVec3::new((bx + 1) as f64, (by + 1) as f64, (bz + 1) as f64),
                        });
                    }
                }
            }
        }

        let orig_vel = self.velocity;
        let mut dx = orig_vel.x;
        let mut dy = orig_vel.y;
        let mut dz = orig_vel.z;

        for b in &block_boxes { dy = b.clip_y_collide(&player_box, dy); }
        player_box = player_box.offset(DVec3::new(0.0, dy, 0.0));

        for b in &block_boxes { dx = b.clip_x_collide(&player_box, dx); }
        player_box = player_box.offset(DVec3::new(dx, 0.0, 0.0));

        for b in &block_boxes { dz = b.clip_z_collide(&player_box, dz); }
        player_box = player_box.offset(DVec3::new(0.0, 0.0, dz));

        self.curr_pos = DVec3::new(
            (player_box.min.x + player_box.max.x) * 0.5,
            player_box.min.y,
            (player_box.min.z + player_box.max.z) * 0.5,
        );

        self.on_ground = orig_vel.y != dy && orig_vel.y < 0.0;
        if orig_vel.x != dx { self.velocity.x = 0.0; }
        if orig_vel.y != dy { self.velocity.y = 0.0; }
        if orig_vel.z != dz { self.velocity.z = 0.0; }

        self.velocity.y = (self.velocity.y - 0.08) * 0.9800000190734863;
        self.velocity.x *= friction;
        self.velocity.z *= friction;
    }

    #[inline]
    pub fn interpolated_position(&self, alpha: f64) -> DVec3 {
        self.prev_pos.lerp(self.curr_pos, alpha.clamp(0.0, 1.0))
    }
}

// ============================================================================
// 7. SỰ KIỆN MẠNG & TRAIT ADAPTER ĐA PHIÊN BẢN
// ============================================================================

#[derive(Debug, Clone)]
pub enum ServerEvent {
    LoginSuccess { uuid: String, username: String },
    CompressionEnabled { threshold: i32 },
    ConfigurationComplete,
    JoinServerWorld { entity_id: i32, dimension: i32, max_players: u8 },
    KeepAliveRequest { keep_alive_id: i64 },
    SynchronizePlayerPosition { x: f64, y: f64, z: f64, yaw: f32, pitch: f32, teleport_id: i32 },
    ChunkColumnLoad { column: ChunkColumn },
    BlockChange { x: i32, y: i32, z: i32, state: CanonicalBlockState },
    SpawnPlayer { entity_id: i32, name: String, x: f64, y: f64, z: f64, yaw: f32, pitch: f32 },
    EntityMove { entity_id: i32, x: f64, y: f64, z: f64, yaw: f32, pitch: f32, is_relative: bool },
    DestroyEntities { entity_ids: Vec<i32> },
    OpenContainerWindow { window_id: u8, title: String, slot_count: usize },
    ContainerSetItems { window_id: u8, state_id: i32, items: Vec<Option<ItemStack>> },
    SystemChat { json_message: String, is_action_bar: bool },
    Disconnected { reason: String },
}

#[derive(Debug, Clone)]
pub enum ClientAction {
    InitiateLogin { server_host: String, server_port: u16, username: String },
    KeepAliveResponse { keep_alive_id: i64 },
    ConfirmTeleport { teleport_id: i32 },
    SendPositionAndRotation { x: f64, y: f64, z: f64, yaw: f32, pitch: f32, on_ground: bool },
    SendChatMessage { message: String },
    ClickContainerSlot { window_id: u8, state_id: i32, slot: i16, button: u8, mode: i32, action_id: i16 },
    CloseContainerWindow { window_id: u8 },
}

#[derive(Error, Debug)]
pub enum ProtocolError {
    #[error("Gói tin chưa nhận đủ byte (Incomplete Frame)")]
    Incomplete,
    #[error("Gói tin sai định dạng: {0}")]
    MalformedPacket(String),
    #[error("Lỗi nén/giải nén Zlib: {0}")]
    CompressionError(String),
}

pub trait ProtocolAdapter: Send + Sync {
    fn version(&self) -> GameVersion;
    fn current_state(&self) -> ConnectionState;
    fn compression_threshold(&self) -> Option<i32>;

    fn encode_action(&mut self, action: ClientAction, out_buf: &mut BytesMut) -> Result<(), ProtocolError>;
    fn decode_packets(&mut self, in_buf: &mut BytesMut, out_auto_reply: &mut BytesMut) -> Result<Vec<ServerEvent>, ProtocolError>;
}

// ============================================================================
// 8. BỘ MÃ HÓA KHUNG GÓI TIN (VARINT + BLOCKPOS + ZLIB FRAMING)
// ============================================================================

pub mod codec {
    use super::*;

    pub fn write_varint(buf: &mut BytesMut, mut value: i32) {
        loop {
            if (value & !0x7F) == 0 {
                buf.put_u8(value as u8);
                return;
            }
            buf.put_u8(((value & 0x7F) | 0x80) as u8);
            value = ((value as u32) >> 7) as i32;
        }
    }

    pub fn read_varint(buf: &mut impl Buf) -> Result<i32, ProtocolError> {
        let mut num_read = 0;
        let mut result = 0i32;
        loop {
            if !buf.has_remaining() {
                return Err(ProtocolError::Incomplete);
            }
            let read = buf.get_u8();
            let value = (read & 0x7F) as i32;
            result |= value << (7 * num_read);
            num_read += 1;
            if num_read > 5 {
                return Err(ProtocolError::MalformedPacket("VarInt vượt quá 5 bytes".into()));
            }
            if (read & 0x80) == 0 {
                break;
            }
        }
        Ok(result)
    }

    pub fn write_string(buf: &mut BytesMut, s: &str) {
        let bytes = s.as_bytes();
        write_varint(buf, bytes.len() as i32);
        buf.put_slice(bytes);
    }

    pub fn read_string(buf: &mut impl Buf) -> Result<String, ProtocolError> {
        let len = read_varint(buf)? as usize;
        if buf.remaining() < len {
            return Err(ProtocolError::Incomplete);
        }
        let mut str_bytes = vec![0u8; len];
        buf.copy_to_slice(&mut str_bytes);
        String::from_utf8(str_bytes)
            .map_err(|e| ProtocolError::MalformedPacket(format!("Chuỗi UTF-8 lỗi: {e}")))
    }

    #[inline]
    pub fn decode_block_pos_1_12_2(raw: u64) -> (i32, i32, i32) {
        let x = ((raw as i64) >> 38) as i32;
        let y = (((raw as i64) >> 26) & 0xFFF) as i32;
        let z = (((raw as i64) << 38) >> 38) as i32;
        (x, y, z)
    }

    #[inline]
    pub fn decode_block_pos_26_2(raw: u64) -> (i32, i32, i32) {
        let x = ((raw as i64) >> 38) as i32;
        let z = (((raw as i64) << 26) >> 38) as i32;
        let y = (((raw as i64) << 52) >> 52) as i32;
        (x, y, z)
    }

    pub fn write_framed_packet(
        out_buf: &mut BytesMut,
        packet_id: i32,
        payload: &[u8],
        compression_threshold: Option<i32>,
    ) -> Result<(), ProtocolError> {
        let mut raw_packet = BytesMut::with_capacity(5 + payload.len());
        write_varint(&mut raw_packet, packet_id);
        raw_packet.put_slice(payload);

        match compression_threshold {
            None => {
                write_varint(out_buf, raw_packet.len() as i32);
                out_buf.put_slice(&raw_packet);
            }
            Some(threshold) => {
                if (raw_packet.len() as i32) < threshold {
                    let mut data_len_buf = BytesMut::with_capacity(1);
                    write_varint(&mut data_len_buf, 0);
                    write_varint(out_buf, (data_len_buf.len() + raw_packet.len()) as i32);
                    out_buf.put_slice(&data_len_buf);
                    out_buf.put_slice(&raw_packet);
                } else {
                    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
                    encoder.write_all(&raw_packet).map_err(|e| ProtocolError::CompressionError(e.to_string()))?;
                    let compressed = encoder.finish().map_err(|e| ProtocolError::CompressionError(e.to_string()))?;

                    let mut data_len_buf = BytesMut::with_capacity(5);
                    write_varint(&mut data_len_buf, raw_packet.len() as i32);

                    write_varint(out_buf, (data_len_buf.len() + compressed.len()) as i32);
                    out_buf.put_slice(&data_len_buf);
                    out_buf.put_slice(&compressed);
                }
            }
        }
        Ok(())
    }

    pub fn try_read_framed_packet(
        in_buf: &mut BytesMut,
        compression_threshold: Option<i32>,
    ) -> Result<Option<(i32, BytesMut)>, ProtocolError> {
        let mut peek = in_buf.clone();
        let packet_len = match read_varint(&mut peek) {
            Ok(len) => len as usize,
            Err(ProtocolError::Incomplete) => return Ok(None),
            Err(e) => return Err(e),
        };

        let header_len = in_buf.len() - peek.len();
        if in_buf.len() < header_len + packet_len {
            return Ok(None);
        }

        in_buf.advance(header_len);
        let mut frame = in_buf.split_to(packet_len);

        let mut payload_buf = match compression_threshold {
            None => frame,
            Some(_) => {
                let uncompressed_len = read_varint(&mut frame)? as usize;
                if uncompressed_len == 0 {
                    frame
                } else {
                    let mut decoder = ZlibDecoder::new(&frame[..]);
                    let mut decompressed = vec![0u8; uncompressed_len];
                    decoder.read_exact(&mut decompressed).map_err(|e| ProtocolError::CompressionError(e.to_string()))?;
                    BytesMut::from(&decompressed[..])
                }
            }
        };

        let packet_id = read_varint(&mut payload_buf)?;
        Ok(Some((packet_id, payload_buf)))
    }
}
```

---

### 7.3. Crate 2: `crates/proto_1_12_2` (Giao thức 1.12.2 Đầy Đủ: Chunk, Entities & Containers)

#### `crates/proto_1_12_2/Cargo.toml`

```toml
[package]
name = "proto_1_12_2"
version = "0.1.0"
edition = "2021"

[dependencies]
casx_core = { path = "../casx_core" }
bytes.workspace = true
```

#### `crates/proto_1_12_2/src/lib.rs`

```rust
use bytes::{Buf, BufMut, BytesMut};
use casx_core::{
    codec::{
        decode_block_pos_1_12_2, read_string, read_varint, try_read_framed_packet,
        write_framed_packet, write_string, write_varint,
    },
    CanonicalBlockState, ChunkColumn, ChunkSection, ClientAction, ConnectionState, GameVersion,
    ItemStack, NbtTag, ProtocolAdapter, ProtocolError, ServerEvent,
};

pub struct Proto1_12_2 {
    state: ConnectionState,
    compression_threshold: Option<i32>,
}

impl Default for Proto1_12_2 {
    fn default() -> Self {
        Self::new()
    }
}

impl Proto1_12_2 {
    pub fn new() -> Self {
        Self {
            state: ConnectionState::Handshaking,
            compression_threshold: None,
        }
    }

    /// Giải mã một Slot vật phẩm trên 1.12.2 (Block/Item ID i16 + Count u8 + Damage i16 + Classic NBT)
    fn read_slot_1_12_2(buf: &mut impl Buf) -> Result<Option<ItemStack>, ProtocolError> {
        if buf.remaining() < 2 {
            return Err(ProtocolError::Incomplete);
        }
        let item_id = buf.get_i16();
        if item_id == -1 {
            return Ok(None);
        }
        if buf.remaining() < 3 {
            return Err(ProtocolError::Incomplete);
        }
        let count = buf.get_u8();
        let metadata = buf.get_i16();
        let (_, _nbt) = NbtTag::decode_root(buf, false)?;
        Ok(Some(ItemStack {
            item_id: item_id as i32,
            count,
            metadata,
            display_name: None,
        }))
    }

    pub fn decode_section_1_12_2(buf: &mut impl Buf, has_skylight: bool) -> Result<ChunkSection, ProtocolError> {
        if !buf.has_remaining() {
            return Err(ProtocolError::Incomplete);
        }
        let mut bits_per_block = buf.get_u8() as usize;
        if bits_per_block < 4 {
            bits_per_block = 4;
        }
        if bits_per_block > 8 {
            bits_per_block = 13;
        }

        let palette_len = read_varint(buf)? as usize;
        let mut palette = Vec::with_capacity(palette_len);
        for _ in 0..palette_len {
            let state_id = read_varint(buf)? as u32;
            palette.push(CanonicalBlockState(state_id));
        }

        let long_count = read_varint(buf)? as usize;
        if buf.remaining() < long_count * 8 {
            return Err(ProtocolError::Incomplete);
        }
        let mut longs = Vec::with_capacity(long_count);
        for _ in 0..long_count {
            longs.push(buf.get_u64());
        }

        let mut section = ChunkSection::new_empty();
        let max_entry_value = (1u64 << bits_per_block) - 1;

        for block_idx in 0..4096 {
            let bit_index = block_idx * bits_per_block;
            let start_long = bit_index / 64;
            let end_long = (bit_index + bits_per_block - 1) / 64;
            let start_offset = bit_index % 64;

            if start_long >= longs.len() {
                break;
            }

            let raw_palette_idx = if start_long == end_long {
                (longs[start_long] >> start_offset) & max_entry_value
            } else if end_long < longs.len() {
                let end_offset = 64 - start_offset;
                ((longs[start_long] >> start_offset) | (longs[end_long] << end_offset)) & max_entry_value
            } else {
                0
            } as usize;

            let canonical_state = if bits_per_block <= 8 {
                palette.get(raw_palette_idx).copied().unwrap_or(CanonicalBlockState::AIR)
            } else {
                CanonicalBlockState(raw_palette_idx as u32)
            };

            if !canonical_state.is_air() {
                section.blocks[block_idx] = canonical_state;
                section.non_air_count += 1;
            }
        }

        if buf.remaining() >= 2048 {
            buf.advance(2048);
        }
        if has_skylight && buf.remaining() >= 2048 {
            buf.advance(2048);
        }

        Ok(section)
    }

    fn decode_chunk_column_packet(&self, mut data: BytesMut) -> Result<ChunkColumn, ProtocolError> {
        let chunk_x = data.get_i32();
        let chunk_z = data.get_i32();
        let _ground_up_continuous = data.get_u8() != 0;
        let primary_bit_mask = read_varint(&mut data)? as u16;
        let data_size = read_varint(&mut data)? as usize;

        if data.remaining() < data_size {
            return Err(ProtocolError::Incomplete);
        }
        let mut section_stream = data.split_to(data_size);

        let mut column = ChunkColumn::new(chunk_x, chunk_z, 0, 16);
        for section_y in 0..16 {
            if (primary_bit_mask & (1 << section_y)) != 0 {
                let sec = Self::decode_section_1_12_2(&mut section_stream, true)?;
                column.sections[section_y] = Some(sec);
            }
        }
        Ok(column)
    }
}

impl ProtocolAdapter for Proto1_12_2 {
    fn version(&self) -> GameVersion {
        GameVersion::V1_12_2
    }

    fn current_state(&self) -> ConnectionState {
        self.state
    }

    fn compression_threshold(&self) -> Option<i32> {
        self.compression_threshold
    }

    fn encode_action(&mut self, action: ClientAction, out_buf: &mut BytesMut) -> Result<(), ProtocolError> {
        match action {
            ClientAction::InitiateLogin { server_host, server_port, username } => {
                let mut hs = BytesMut::new();
                write_varint(&mut hs, 340);
                write_string(&mut hs, &server_host);
                hs.put_u16(server_port);
                write_varint(&mut hs, 2);
                write_framed_packet(out_buf, 0x00, &hs, None)?;

                self.state = ConnectionState::Login;

                let mut login = BytesMut::new();
                write_string(&mut login, &username);
                write_framed_packet(out_buf, 0x00, &login, self.compression_threshold)
            }
            ClientAction::KeepAliveResponse { keep_alive_id } => {
                let mut payload = BytesMut::with_capacity(8);
                payload.put_i64(keep_alive_id);
                write_framed_packet(out_buf, 0x0B, &payload, self.compression_threshold)
            }
            ClientAction::ConfirmTeleport { teleport_id } => {
                let mut payload = BytesMut::new();
                write_varint(&mut payload, teleport_id);
                write_framed_packet(out_buf, 0x00, &payload, self.compression_threshold)
            }
            ClientAction::SendPositionAndRotation { x, y, z, yaw, pitch, on_ground } => {
                let mut payload = BytesMut::with_capacity(33);
                payload.put_f64(x);
                payload.put_f64(y);
                payload.put_f64(z);
                payload.put_f32(yaw);
                payload.put_f32(pitch);
                payload.put_u8(u8::from(on_ground));
                write_framed_packet(out_buf, 0x0E, &payload, self.compression_threshold)
            }
            ClientAction::SendChatMessage { message } => {
                let mut payload = BytesMut::new();
                write_string(&mut payload, &message);
                write_framed_packet(out_buf, 0x02, &payload, self.compression_threshold)
            }
            ClientAction::ClickContainerSlot { window_id, slot, button, mode, action_id, .. } => {
                // Click Window (0x07 trên 1.12.2)
                let mut payload = BytesMut::new();
                payload.put_u8(window_id);
                payload.put_i16(slot);
                payload.put_u8(button);
                payload.put_i16(action_id);
                write_varint(&mut payload, mode);
                payload.put_i16(-1); // Empty clicked item placeholder
                write_framed_packet(out_buf, 0x07, &payload, self.compression_threshold)
            }
            ClientAction::CloseContainerWindow { window_id } => {
                let mut payload = BytesMut::with_capacity(1);
                payload.put_u8(window_id);
                write_framed_packet(out_buf, 0x08, &payload, self.compression_threshold)
            }
        }
    }

    fn decode_packets(
        &mut self,
        in_buf: &mut BytesMut,
        _out_auto_reply: &mut BytesMut,
    ) -> Result<Vec<ServerEvent>, ProtocolError> {
        let mut events = Vec::new();

        while let Some((packet_id, mut data)) = try_read_framed_packet(in_buf, self.compression_threshold)? {
            match self.state {
                ConnectionState::Login => match packet_id {
                    0x00 => {
                        let reason = read_string(&mut data)?;
                        self.state = ConnectionState::Disconnected;
                        events.push(ServerEvent::Disconnected { reason });
                    }
                    0x02 => {
                        let uuid = read_string(&mut data)?;
                        let username = read_string(&mut data)?;
                        self.state = ConnectionState::Play;
                        events.push(ServerEvent::LoginSuccess { uuid, username });
                    }
                    0x03 => {
                        let threshold = read_varint(&mut data)?;
                        self.compression_threshold = if threshold >= 0 { Some(threshold) } else { None };
                        events.push(ServerEvent::CompressionEnabled { threshold });
                    }
                    _ => {}
                },
                ConnectionState::Play => match packet_id {
                    0x05 => {
                        // Spawn Player (0x05)
                        let entity_id = read_varint(&mut data)?;
                        let _uuid = data.get_u128();
                        let x = data.get_f64();
                        let y = data.get_f64();
                        let z = data.get_f64();
                        let yaw = (data.get_i8() as f32) * 360.0 / 256.0;
                        let pitch = (data.get_i8() as f32) * 360.0 / 256.0;
                        events.push(ServerEvent::SpawnPlayer {
                            entity_id,
                            name: format!("Player_{entity_id}"),
                            x,
                            y,
                            z,
                            yaw,
                            pitch,
                        });
                    }
                    0x0B => {
                        let (x, y, z) = decode_block_pos_1_12_2(data.get_u64());
                        let state_id = read_varint(&mut data)? as u32;
                        events.push(ServerEvent::BlockChange {
                            x, y, z, state: CanonicalBlockState(state_id),
                        });
                    }
                    0x0F => {
                        let json_message = read_string(&mut data)?;
                        let pos = data.get_u8();
                        events.push(ServerEvent::SystemChat {
                            json_message,
                            is_action_bar: pos == 2,
                        });
                    }
                    0x13 => {
                        // Open Window (0x13)
                        let window_id = data.get_u8();
                        let _window_type = read_string(&mut data)?;
                        let title = read_string(&mut data)?;
                        let slot_count = data.get_u8() as usize;
                        events.push(ServerEvent::OpenContainerWindow { window_id, title, slot_count });
                    }
                    0x14 => {
                        // Window Items (0x14)
                        let window_id = data.get_u8();
                        let count = data.get_i16() as usize;
                        let mut items = Vec::with_capacity(count);
                        for _ in 0..count {
                            items.push(Self::read_slot_1_12_2(&mut data)?);
                        }
                        events.push(ServerEvent::ContainerSetItems { window_id, state_id: 0, items });
                    }
                    0x1A => {
                        let reason = read_string(&mut data)?;
                        self.state = ConnectionState::Disconnected;
                        events.push(ServerEvent::Disconnected { reason });
                    }
                    0x1F => {
                        let keep_alive_id = data.get_i64();
                        events.push(ServerEvent::KeepAliveRequest { keep_alive_id });
                    }
                    0x20 => {
                        if let Ok(column) = self.decode_chunk_column_packet(data) {
                            events.push(ServerEvent::ChunkColumnLoad { column });
                        }
                    }
                    0x23 => {
                        let entity_id = data.get_i32();
                        let _gamemode = data.get_u8();
                        let dimension = data.get_i32();
                        let _difficulty = data.get_u8();
                        let max_players = data.get_u8();
                        events.push(ServerEvent::JoinServerWorld { entity_id, dimension, max_players });
                    }
                    0x2F => {
                        let x = data.get_f64();
                        let y = data.get_f64();
                        let z = data.get_f64();
                        let yaw = data.get_f32();
                        let pitch = data.get_f32();
                        let _flags = data.get_u8();
                        let teleport_id = read_varint(&mut data)?;
                        events.push(ServerEvent::SynchronizePlayerPosition { x, y, z, yaw, pitch, teleport_id });
                    }
                    0x32 => {
                        // Destroy Entities (0x32)
                        let count = read_varint(&mut data)? as usize;
                        let mut entity_ids = Vec::with_capacity(count);
                        for _ in 0..count {
                            entity_ids.push(read_varint(&mut data)?);
                        }
                        events.push(ServerEvent::DestroyEntities { entity_ids });
                    }
                    0x4C => {
                        // Entity Teleport (0x4C)
                        let entity_id = read_varint(&mut data)?;
                        let x = data.get_f64();
                        let y = data.get_f64();
                        let z = data.get_f64();
                        let yaw = (data.get_i8() as f32) * 360.0 / 256.0;
                        let pitch = (data.get_i8() as f32) * 360.0 / 256.0;
                        events.push(ServerEvent::EntityMove {
                            entity_id, x, y, z, yaw, pitch, is_relative: false,
                        });
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        Ok(events)
    }
}
```

---

### 7.4. Crate 3: `crates/proto_26_2` (Giao thức 26.2 Đầy Đủ: Configuration, Chunk Packet `0x27` & Nameless NBT)

#### `crates/proto_26_2/Cargo.toml`

```toml
[package]
name = "proto_26_2"
version = "0.1.0"
edition = "2021"

[dependencies]
casx_core = { path = "../casx_core" }
bytes.workspace = true
```

#### `crates/proto_26_2/src/lib.rs`

```rust
use bytes::{Buf, BufMut, BytesMut};
use casx_core::{
    codec::{
        decode_block_pos_26_2, read_string, read_varint, try_read_framed_packet,
        write_framed_packet, write_string, write_varint,
    },
    generate_offline_uuid, CanonicalBlockState, ChunkColumn, ChunkSection, ClientAction,
    ConnectionState, GameVersion, NbtTag, ProtocolAdapter, ProtocolError, ServerEvent,
};

pub struct Proto26_2 {
    state: ConnectionState,
    compression_threshold: Option<i32>,
    world_section_count: usize, // Mặc định 24 sections (Y = -64..320)
    world_min_y: i32,
}

impl Default for Proto26_2 {
    fn default() -> Self {
        Self::new()
    }
}

impl Proto26_2 {
    pub fn new() -> Self {
        Self {
            state: ConnectionState::Handshaking,
            compression_threshold: None,
            world_section_count: 24,
            world_min_y: -64,
        }
    }

    pub fn decode_section_26_2(buf: &mut impl Buf) -> Result<ChunkSection, ProtocolError> {
        if buf.remaining() < 3 {
            return Err(ProtocolError::Incomplete);
        }
        let non_air_count = buf.get_u16();
        let bits_per_entry = buf.get_u8() as usize;

        let mut section = ChunkSection::new_empty();
        section.non_air_count = non_air_count;

        if bits_per_entry == 0 {
            let single_state = CanonicalBlockState(read_varint(buf)? as u32);
            let _data_array_len = read_varint(buf)?;
            if !single_state.is_air() {
                section.blocks.fill(single_state);
            }
            return Ok(section);
        }

        let palette = if bits_per_entry <= 8 {
            let len = read_varint(buf)? as usize;
            let mut pal = Vec::with_capacity(len);
            for _ in 0..len {
                pal.push(CanonicalBlockState(read_varint(buf)? as u32));
            }
            Some(pal)
        } else {
            None
        };

        let effective_bpe = if bits_per_entry <= 4 { 4 } else { bits_per_entry };
        let long_count = read_varint(buf)? as usize;
        if buf.remaining() < long_count * 8 {
            return Err(ProtocolError::Incomplete);
        }

        let entries_per_long = 64 / effective_bpe;
        let mask = (1u64 << effective_bpe) - 1;
        let mut block_idx = 0usize;

        for _ in 0..long_count {
            let word = buf.get_u64();
            for sub in 0..entries_per_long {
                if block_idx >= 4096 {
                    break;
                }
                let raw_val = ((word >> (sub * effective_bpe)) & mask) as usize;
                let state = match &palette {
                    Some(pal) => pal.get(raw_val).copied().unwrap_or(CanonicalBlockState::AIR),
                    None => CanonicalBlockState(raw_val as u32),
                };
                section.blocks[block_idx] = state;
                block_idx += 1;
            }
        }

        Ok(section)
    }

    /// Bỏ qua PalettedContainer của Biome (4x4x4 = 64 phần tử) trong mỗi Section 26.2
    fn skip_biome_container_26_2(buf: &mut impl Buf) -> Result<(), ProtocolError> {
        if !buf.has_remaining() {
            return Err(ProtocolError::Incomplete);
        }
        let bpe = buf.get_u8() as usize;
        if bpe == 0 {
            let _single = read_varint(buf)?;
            let _len = read_varint(buf)?;
            return Ok(());
        }
        if bpe <= 3 {
            let len = read_varint(buf)? as usize;
            for _ in 0..len {
                let _ = read_varint(buf)?;
            }
        }
        let long_count = read_varint(buf)? as usize;
        if buf.remaining() < long_count * 8 {
            return Err(ProtocolError::Incomplete);
        }
        buf.advance(long_count * 8);
        Ok(())
    }

    /// Giải mã hoàn chỉnh gói tin Level Chunk With Light (0x27) trên 26.2
    fn decode_chunk_packet_26_2(&self, mut data: BytesMut) -> Result<ChunkColumn, ProtocolError> {
        let chunk_x = data.get_i32();
        let chunk_z = data.get_i32();

        // 1. Giải mã Heightmaps (Network Nameless NBT)
        let (_, _heightmaps) = NbtTag::decode_root(&mut data, true)?;

        // 2. Đọc mảng dữ liệu Section
        let data_size = read_varint(&mut data)? as usize;
        if data.remaining() < data_size {
            return Err(ProtocolError::Incomplete);
        }
        let mut section_stream = data.split_to(data_size);

        let mut column = ChunkColumn::new(chunk_x, chunk_z, self.world_min_y, self.world_section_count);
        for sec_idx in 0..self.world_section_count {
            if !section_stream.has_remaining() {
                break;
            }
            let section = Self::decode_section_26_2(&mut section_stream)?;
            Self::skip_biome_container_26_2(&mut section_stream)?;
            if section.non_air_count > 0 {
                column.sections[sec_idx] = Some(section);
            }
        }

        Ok(column)
    }
}

impl ProtocolAdapter for Proto26_2 {
    fn version(&self) -> GameVersion {
        GameVersion::V26_2
    }

    fn current_state(&self) -> ConnectionState {
        self.state
    }

    fn compression_threshold(&self) -> Option<i32> {
        self.compression_threshold
    }

    fn encode_action(&mut self, action: ClientAction, out_buf: &mut BytesMut) -> Result<(), ProtocolError> {
        match action {
            ClientAction::InitiateLogin { server_host, server_port, username } => {
                let mut hs = BytesMut::new();
                write_varint(&mut hs, GameVersion::V26_2.protocol_id());
                write_string(&mut hs, &server_host);
                hs.put_u16(server_port);
                write_varint(&mut hs, 2);
                write_framed_packet(out_buf, 0x00, &hs, None)?;

                self.state = ConnectionState::Login;

                let mut login = BytesMut::new();
                write_string(&mut login, &username);
                login.put_u128(generate_offline_uuid(&username));
                write_framed_packet(out_buf, 0x00, &login, self.compression_threshold)
            }
            ClientAction::KeepAliveResponse { keep_alive_id } => {
                let packet_id = if self.state == ConnectionState::Configuration { 0x04 } else { 0x18 };
                let mut payload = BytesMut::with_capacity(8);
                payload.put_i64(keep_alive_id);
                write_framed_packet(out_buf, packet_id, &payload, self.compression_threshold)
            }
            ClientAction::ConfirmTeleport { teleport_id } => {
                let mut payload = BytesMut::new();
                write_varint(&mut payload, teleport_id);
                write_framed_packet(out_buf, 0x00, &payload, self.compression_threshold)
            }
            ClientAction::SendPositionAndRotation { x, y, z, yaw, pitch, on_ground } => {
                let mut payload = BytesMut::with_capacity(33);
                payload.put_f64(x);
                payload.put_f64(y);
                payload.put_f64(z);
                payload.put_f32(yaw);
                payload.put_f32(pitch);
                payload.put_u8(u8::from(on_ground));
                write_framed_packet(out_buf, 0x1B, &payload, self.compression_threshold)
            }
            ClientAction::SendChatMessage { message } => {
                let mut payload = BytesMut::new();
                write_string(&mut payload, &message);
                payload.put_i64(0);
                payload.put_i64(0);
                payload.put_u8(0);
                write_varint(&mut payload, 0);
                payload.put_slice(&[0u8; 3]);
                write_framed_packet(out_buf, 0x06, &payload, self.compression_threshold)
            }
            ClientAction::ClickContainerSlot { window_id, state_id, slot, button, mode, .. } => {
                // Container Click (0x0E trên 26.2)
                let mut payload = BytesMut::new();
                payload.put_u8(window_id);
                write_varint(&mut payload, state_id);
                payload.put_i16(slot);
                payload.put_u8(button);
                write_varint(&mut payload, mode);
                write_varint(&mut payload, 0); // Changed slots count = 0
                payload.put_u8(0); // Carried item count = 0
                write_framed_packet(out_buf, 0x0E, &payload, self.compression_threshold)
            }
            ClientAction::CloseContainerWindow { window_id } => {
                let mut payload = BytesMut::with_capacity(1);
                payload.put_u8(window_id);
                write_framed_packet(out_buf, 0x0F, &payload, self.compression_threshold)
            }
        }
    }

    fn decode_packets(
        &mut self,
        in_buf: &mut BytesMut,
        out_auto_reply: &mut BytesMut,
    ) -> Result<Vec<ServerEvent>, ProtocolError> {
        let mut events = Vec::new();

        while let Some((packet_id, mut data)) = try_read_framed_packet(in_buf, self.compression_threshold)? {
            match self.state {
                ConnectionState::Login => match packet_id {
                    0x00 => {
                        let reason = read_string(&mut data)?;
                        self.state = ConnectionState::Disconnected;
                        events.push(ServerEvent::Disconnected { reason });
                    }
                    0x02 => {
                        let uuid_raw = data.get_u128();
                        let username = read_string(&mut data)?;
                        write_framed_packet(out_auto_reply, 0x03, &[], self.compression_threshold)?;
                        self.state = ConnectionState::Configuration;
                        events.push(ServerEvent::LoginSuccess {
                            uuid: format!("{uuid_raw:032x}"),
                            username,
                        });
                    }
                    0x03 => {
                        let threshold = read_varint(&mut data)?;
                        self.compression_threshold = if threshold >= 0 { Some(threshold) } else { None };
                        events.push(ServerEvent::CompressionEnabled { threshold });
                    }
                    _ => {}
                },
                ConnectionState::Configuration => match packet_id {
                    0x03 => {
                        write_framed_packet(out_auto_reply, 0x03, &[], self.compression_threshold)?;
                        self.state = ConnectionState::Play;
                        events.push(ServerEvent::ConfigurationComplete);
                    }
                    0x04 => {
                        let keep_alive_id = data.get_i64();
                        events.push(ServerEvent::KeepAliveRequest { keep_alive_id });
                    }
                    0x0E => {
                        let mut reply = BytesMut::new();
                        write_varint(&mut reply, 0);
                        write_framed_packet(out_auto_reply, 0x07, &reply, self.compression_threshold)?;
                    }
                    _ => {}
                },
                ConnectionState::Play => match packet_id {
                    0x01 => {
                        // Add Entity (0x01)
                        let entity_id = read_varint(&mut data)?;
                        let _uuid = data.get_u128();
                        let _entity_type = read_varint(&mut data)?;
                        let x = data.get_f64();
                        let y = data.get_f64();
                        let z = data.get_f64();
                        let pitch = (data.get_i8() as f32) * 360.0 / 256.0;
                        let yaw = (data.get_i8() as f32) * 360.0 / 256.0;
                        events.push(ServerEvent::SpawnPlayer {
                            entity_id,
                            name: format!("Entity_{entity_id}"),
                            x, y, z, yaw, pitch,
                        });
                    }
                    0x09 => {
                        // Block Update (0x09)
                        let (x, y, z) = decode_block_pos_26_2(data.get_u64());
                        let state_id = read_varint(&mut data)? as u32;
                        events.push(ServerEvent::BlockChange {
                            x, y, z, state: CanonicalBlockState(state_id),
                        });
                    }
                    0x26 => {
                        let keep_alive_id = data.get_i64();
                        events.push(ServerEvent::KeepAliveRequest { keep_alive_id });
                    }
                    0x27 => {
                        if let Ok(column) = self.decode_chunk_packet_26_2(data) {
                            events.push(ServerEvent::ChunkColumnLoad { column });
                        }
                    }
                    0x2B => {
                        let entity_id = data.get_i32();
                        let _hardcore = data.get_u8();
                        events.push(ServerEvent::JoinServerWorld {
                            entity_id,
                            dimension: 0,
                            max_players: 100,
                        });
                    }
                    0x40 => {
                        let teleport_id = read_varint(&mut data)?;
                        let x = data.get_f64();
                        let y = data.get_f64();
                        let z = data.get_f64();
                        let _vx = data.get_f64();
                        let _vy = data.get_f64();
                        let _vz = data.get_f64();
                        let yaw = data.get_f32();
                        let pitch = data.get_f32();
                        events.push(ServerEvent::SynchronizePlayerPosition {
                            x, y, z, yaw, pitch, teleport_id,
                        });
                    }
                    0x6C => {
                        let json_message = read_string(&mut data)?;
                        let is_action_bar = data.has_remaining() && data.get_u8() != 0;
                        events.push(ServerEvent::SystemChat { json_message, is_action_bar });
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        Ok(events)
    }
}
```

---

### 7.5. Crate 4: `crates/casx_render` (6-Direction Greedy Mesher, AO, Frustum Culling & Dual Kawase Blur)

#### `crates/casx_render/Cargo.toml`

```toml
[package]
name = "casx_render"
version = "0.1.0"
edition = "2021"

[dependencies]
casx_core = { path = "../casx_core" }
rayon.workspace = true
glam.workspace = true
bytemuck.workspace = true
wgpu.workspace = true
```

#### `crates/casx_render/src/lib.rs`

```rust
use bytemuck::{Pod, Zeroable};
use casx_core::{CanonicalBlockState, ChunkSection};
use glam::{Mat4, Vec3, Vec4};

// ============================================================================
// 1. ĐỈNH VOXEL NÉN 64-BIT & FRUSTUM CULLING 6 MẶT PHẲNG
// ============================================================================

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Pod, Zeroable)]
pub struct PackedVoxelVertex(pub u64);

impl PackedVoxelVertex {
    #[inline]
    pub const fn pack(
        x: u8,          // 0..=16 (6 bits)
        y: u8,          // 0..=16 (6 bits)
        z: u8,          // 0..=16 (6 bits)
        quad_w: u8,     // 1..=16 (5 bits)
        quad_h: u8,     // 1..=16 (5 bits)
        normal_dir: u8, // 0..=5  (3 bits)
        ao_level: u8,   // 0..=3  (2 bits)
        tex_layer: u16, // 0..=65535 (16 bits)
        light: u8,      // 4-bit block | 4-bit sky (8 bits)
    ) -> Self {
        let v = (x as u64 & 0x3F)
            | ((y as u64 & 0x3F) << 6)
            | ((z as u64 & 0x3F) << 12)
            | ((quad_w as u64 & 0x1F) << 18)
            | ((quad_h as u64 & 0x1F) << 23)
            | ((normal_dir as u64 & 0x07) << 28)
            | ((ao_level as u64 & 0x03) << 31)
            | ((tex_layer as u64 & 0xFFFF) << 33)
            | ((light as u64) << 49);
        Self(v)
    }
}

/// Bộ kiểm tra cắt tầm nhìn 6 mặt phẳng (View-Frustum Culling)
#[derive(Debug, Clone, Copy)]
pub struct ViewFrustum {
    pub planes: [Vec4; 6], // (nx, ny, nz, d)
}

impl ViewFrustum {
    pub fn from_view_proj(vp: Mat4) -> Self {
        let r0 = vp.row(0);
        let r1 = vp.row(1);
        let r2 = vp.row(2);
        let r3 = vp.row(3);

        let mut planes = [
            r3 + r0, // Left
            r3 - r0, // Right
            r3 + r1, // Bottom
            r3 - r1, // Top
            r2,      // Near (wgpu NDC z in [0, 1])
            r3 - r2, // Far
        ];

        for p in &mut planes {
            let len = Vec3::new(p.x, p.y, p.z).length();
            if len > 1.0e-6 {
                *p /= len;
            }
        }
        Self { planes }
    }

    /// Trả về `true` nếu hộp giới hạn [min, max] nằm hoàn toàn hoặc một phần trong tầm nhìn
    #[inline]
    pub fn is_aabb_visible(&self, min: Vec3, max: Vec3) -> bool {
        for p in &self.planes {
            let px = if p.x >= 0.0 { max.x } else { min.x };
            let py = if p.y >= 0.0 { max.y } else { min.y };
            let pz = if p.z >= 0.0 { max.z } else { min.z };
            if p.x * px + p.y * py + p.z * pz + p.w < 0.0 {
                return false;
            }
        }
        true
    }
}

// ============================================================================
// 2. THUẬT TOÁN GREEDY MESHING ĐẦY ĐỦ CẢ 6 HƯỚNG MẶT KHỐI + VOXEL AO
// ============================================================================

#[derive(Default, Clone)]
pub struct SectionMesh {
    pub vertices: Vec<PackedVoxelVertex>,
    pub indices: Vec<u32>,
    pub merged_quad_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FaceCell {
    state: CanonicalBlockState,
    ao_corners: [u8; 4], // AO tại 4 đỉnh của mặt khối
}

pub struct GreedyVoxelMesher;

impl GreedyVoxelMesher {
    /// Công thức chuẩn tính Voxel Ambient Occlusion cho một đỉnh
    #[inline]
    const fn compute_vertex_ao(side1: bool, side2: bool, corner: bool) -> u8 {
        if side1 && side2 {
            0
        } else {
            3 - (side1 as u8 + side2 as u8 + corner as u8)
        }
    }

    #[inline]
    fn is_solid_safe(section: &ChunkSection, x: i32, y: i32, z: i32) -> bool {
        if (0..16).contains(&x) && (0..16).contains(&y) && (0..16).contains(&z) {
            section.get(x as usize, y as usize, z as usize).is_opaque_solid()
        } else {
            false
        }
    }

    /// Xây dựng lưới tối ưu cho toàn bộ 6 hướng mặt khối (+X, -X, +Y, -Y, +Z, -Z)
    pub fn build_full_section_mesh(section: &ChunkSection) -> SectionMesh {
        if section.non_air_count == 0 {
            return SectionMesh::default();
        }

        let mut mesh = SectionMesh::default();

        // Duyệt qua cả 6 hướng mặt phẳng:
        // dir = 0 (+X), 1 (-X), 2 (+Y), 3 (-Y), 4 (+Z), 5 (-Z)
        for dir in 0u8..6 {
            let (axis, u_axis, v_axis, step): (usize, usize, usize, i32) = match dir {
                0 => (0, 2, 1, 1),  // +X: mặt phẳng (Z, Y)
                1 => (0, 2, 1, -1), // -X: mặt phẳng (Z, Y)
                2 => (1, 0, 2, 1),  // +Y: mặt phẳng (X, Z)
                3 => (1, 0, 2, -1), // -Y: mặt phẳng (X, Z)
                4 => (2, 0, 1, 1),  // +Z: mặt phẳng (X, Y)
                _ => (2, 0, 1, -1), // -Z: mặt phẳng (X, Y)
            };

            for slice in 0..16i32 {
                let mut mask: [[Option<FaceCell>; 16]; 16] = [[None; 16]; 16];

                // Bước 1: Quét lát cắt 16x16 và lọc các mặt bị che khuất (Face Culling)
                for v in 0..16i32 {
                    for u in 0..16i32 {
                        let mut pos = [0i32; 3];
                        pos[axis] = slice;
                        pos[u_axis] = u;
                        pos[v_axis] = v;

                        let current = section.get(pos[0] as usize, pos[1] as usize, pos[2] as usize);
                        if !current.is_opaque_solid() {
                            continue;
                        }

                        let mut neighbor = pos;
                        neighbor[axis] += step;
                        if Self::is_solid_safe(section, neighbor[0], neighbor[1], neighbor[2]) {
                            continue;
                        }

                        // Tính toán AO 4 góc trên mặt phẳng tiếp xúc (neighbor)
                        let mut du = [0i32; 3];
                        let mut dv = [0i32; 3];
                        du[u_axis] = 1;
                        dv[v_axis] = 1;

                        let s_neg_u = Self::is_solid_safe(section, neighbor[0] - du[0], neighbor[1] - du[1], neighbor[2] - du[2]);
                        let s_pos_u = Self::is_solid_safe(section, neighbor[0] + du[0], neighbor[1] + du[1], neighbor[2] + du[2]);
                        let s_neg_v = Self::is_solid_safe(section, neighbor[0] - dv[0], neighbor[1] - dv[1], neighbor[2] - dv[2]);
                        let s_pos_v = Self::is_solid_safe(section, neighbor[0] + dv[0], neighbor[1] + dv[1], neighbor[2] + dv[2]);

                        let c00 = Self::is_solid_safe(section, neighbor[0] - du[0] - dv[0], neighbor[1] - du[1] - dv[1], neighbor[2] - du[2] - dv[2]);
                        let c01 = Self::is_solid_safe(section, neighbor[0] - du[0] + dv[0], neighbor[1] - du[1] + dv[1], neighbor[2] - du[2] + dv[2]);
                        let c11 = Self::is_solid_safe(section, neighbor[0] + du[0] + dv[0], neighbor[1] + du[1] + dv[1], neighbor[2] + du[2] + dv[2]);
                        let c10 = Self::is_solid_safe(section, neighbor[0] + du[0] - dv[0], neighbor[1] + du[1] - dv[1], neighbor[2] + du[2] - dv[2]);

                        let ao_corners = [
                            Self::compute_vertex_ao(s_neg_u, s_neg_v, c00),
                            Self::compute_vertex_ao(s_neg_u, s_pos_v, c01),
                            Self::compute_vertex_ao(s_pos_u, s_pos_v, c11),
                            Self::compute_vertex_ao(s_pos_u, s_neg_v, c10),
                        ];

                        mask[v as usize][u as usize] = Some(FaceCell { state: current, ao_corners });
                    }
                }

                // Bước 2: Gộp Greedy Quad theo chiều rộng W (trục u) và chiều cao H (trục v)
                for v in 0..16usize {
                    let mut u = 0usize;
                    while u < 16 {
                        let Some(cell) = mask[v][u] else {
                            u += 1;
                            continue;
                        };

                        let mut quad_w = 1usize;
                        while u + quad_w < 16 && mask[v][u + quad_w] == Some(cell) {
                            quad_w += 1;
                        }

                        let mut quad_h = 1usize;
                        'extend_h: while v + quad_h < 16 {
                            for du in 0..quad_w {
                                if mask[v + quad_h][u + du] != Some(cell) {
                                    break 'extend_h;
                                }
                            }
                            quad_h += 1;
                        }

                        for dv in 0..quad_h {
                            for du in 0..quad_w {
                                mask[v + dv][u + du] = None;
                            }
                        }

                        // Bước 3: Xuất 4 đỉnh nén 64-bit
                        let face_plane_coord = if step > 0 { (slice + 1) as u8 } else { slice as u8 };
                        let mut p0 = [0u8; 3];
                        let mut p1 = [0u8; 3];
                        let mut p2 = [0u8; 3];
                        let mut p3 = [0u8; 3];

                        for p in [&mut p0, &mut p1, &mut p2, &mut p3] {
                            p[axis] = face_plane_coord;
                        }

                        p0[u_axis] = u as u8;
                        p0[v_axis] = v as u8;

                        p1[u_axis] = u as u8;
                        p1[v_axis] = (v + quad_h) as u8;

                        p2[u_axis] = (u + quad_w) as u8;
                        p2[v_axis] = (v + quad_h) as u8;

                        p3[u_axis] = (u + quad_w) as u8;
                        p3[v_axis] = v as u8;

                        let base_idx = mesh.vertices.len() as u32;
                        let tex = cell.state.texture_layer_id();
                        let w = quad_w as u8;
                        let h = quad_h as u8;

                        mesh.vertices.push(PackedVoxelVertex::pack(p0[0], p0[1], p0[2], w, h, dir, cell.ao_corners[0], tex, 0xFF));
                        mesh.vertices.push(PackedVoxelVertex::pack(p1[0], p1[1], p1[2], w, h, dir, cell.ao_corners[1], tex, 0xFF));
                        mesh.vertices.push(PackedVoxelVertex::pack(p2[0], p2[1], p2[2], w, h, dir, cell.ao_corners[2], tex, 0xFF));
                        mesh.vertices.push(PackedVoxelVertex::pack(p3[0], p3[1], p3[2], w, h, dir, cell.ao_corners[3], tex, 0xFF));

                        if step > 0 {
                            mesh.indices.extend_from_slice(&[base_idx, base_idx + 1, base_idx + 2, base_idx, base_idx + 2, base_idx + 3]);
                        } else {
                            mesh.indices.extend_from_slice(&[base_idx, base_idx + 2, base_idx + 1, base_idx, base_idx + 3, base_idx + 2]);
                        }
                        mesh.merged_quad_count += 1;
                        u += quad_w;
                    }
                }
            }
        }

        mesh
    }
}

// ============================================================================
// 3. SHADER WGSL: VOXEL 64-BIT UNPACKING & DUAL-FILTER KAWASE BLUR
// ============================================================================

pub const CASX_VOXEL_SHADER_WGSL: &str = r#"
struct CameraUniform {
    view_proj: mat4x4<f32>,
    chunk_offset: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) tex_layer: u32,
    @location(2) shade: f32,
};

@vertex
fn vs_main(
    @location(0) packed_lo: u32,
    @location(1) packed_hi: u32,
    @builtin(vertex_index) v_idx: u32,
) -> VertexOutput {
    let x = f32(packed_lo & 0x3Fu);
    let y = f32((packed_lo >> 6u) & 0x3Fu);
    let z = f32((packed_lo >> 12u) & 0x3Fu);
    let w = f32((packed_lo >> 18u) & 0x1Fu);
    let h = f32((packed_lo >> 23u) & 0x1Fu);
    let normal_dir = (packed_lo >> 28u) & 0x07u;
    let ao_level = f32((packed_lo >> 31u) | ((packed_hi & 0x01u) << 1u));
    let tex_layer = (packed_hi >> 1u) & 0xFFFFu;

    let world_pos = vec3<f32>(x, y, z) + camera.chunk_offset.xyz;
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(world_pos, 1.0);

    let corner = v_idx & 3u;
    if (corner == 0u) { out.uv = vec2<f32>(0.0, 0.0); }
    else if (corner == 1u) { out.uv = vec2<f32>(0.0, h); }
    else if (corner == 2u) { out.uv = vec2<f32>(w, h); }
    else { out.uv = vec2<f32>(w, 0.0); }

    // Đổ bóng định hướng theo pháp tuyến chuẩn Minecraft
    var dir_light = 1.0;
    if (normal_dir == 3u) { dir_light = 0.5; }
    else if (normal_dir == 0u || normal_dir == 1u) { dir_light = 0.6; }
    else if (normal_dir == 4u || normal_dir == 5u) { dir_light = 0.8; }

    let ao_factor = 0.45 + (ao_level / 3.0) * 0.55;
    out.tex_layer = tex_layer;
    out.shade = dir_light * ao_factor;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tiled_uv = fract(in.uv);
    // Tạo vân gạch khối rõ nét ngay cả khi chưa nạp Resource Pack ngoài
    let border = step(0.03, tiled_uv.x) * step(0.03, tiled_uv.y) * step(tiled_uv.x, 0.97) * step(tiled_uv.y, 0.97);
    let base_tint = vec3<f32>(0.38, 0.68, 0.42) * mix(0.78, 1.0, border);
    return vec4<f32>(base_tint * in.shade, 1.0);
}
"#;

/// Shader hậu kỳ Dual-Filter Kawase Blur cho Bảng Mod Lunar Client (Right Shift)
pub const KAWASE_BLUR_SHADER_WGSL: &str = r#"
struct BlurParams {
    half_pixel: vec2<f32>,
    offset: f32,
    _pad: f32,
};

@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;
@group(0) @binding(2) var<uniform> params: BlurParams;

struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_fullscreen(@builtin(vertex_index) idx: u32) -> FullscreenOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>( 3.0, -1.0),
        vec2<f32>(-1.0,  3.0)
    );
    var out: FullscreenOut;
    let p = positions[idx];
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv = vec2<f32>((p.x + 1.0) * 0.5, (1.0 - p.y) * 0.5);
    return out;
}

@fragment
fn fs_kawase_downsample(in: FullscreenOut) -> @location(0) vec4<f32> {
    let d = params.half_pixel * params.offset;
    var sum = textureSample(src_tex, src_sampler, in.uv) * 4.0;
    sum += textureSample(src_tex, src_sampler, in.uv - d);
    sum += textureSample(src_tex, src_sampler, in.uv + d);
    sum += textureSample(src_tex, src_sampler, in.uv + vec2<f32>(d.x, -d.y));
    sum += textureSample(src_tex, src_sampler, in.uv - vec2<f32>(d.x, -d.y));
    return sum / 8.0;
}
"#;
```

---

### 7.6. Crate 5: `crates/casx_ui` (Hệ thống Giao diện Thực tế $\text{HoYoPlay} \times \text{Legacy} \times \text{Lunar}$ với `egui`)

#### `crates/casx_ui/Cargo.toml`

```toml
[package]
name = "casx_ui"
version = "0.1.0"
edition = "2021"

[dependencies]
casx_core = { path = "../casx_core" }
egui.workspace = true
```

#### `crates/casx_ui/src/lib.rs`

```rust
use casx_core::{ContainerWindow, GameVersion};
use egui::{Align2, Color32, Context, CornerRadius, Frame, Margin, RichText, Sense, Stroke, Vec2};
use std::collections::VecDeque;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoYoSidebarTab {
    ServerHub,
    LunarMods,
    ResourcePacks,
    EngineSettings,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthProvider {
    Offline { username: String },
    ElyBy { username: String, access_token: String },
    MicrosoftOAuth { gamertag: String, uuid: String },
}

impl AuthProvider {
    pub fn username(&self) -> &str {
        match self {
            AuthProvider::Offline { username } => username,
            AuthProvider::ElyBy { username, .. } => username,
            AuthProvider::MicrosoftOAuth { gamertag, .. } => gamertag,
        }
    }
}

#[derive(Debug, Clone)]
pub struct QuickServerEntry {
    pub display_name: String,
    pub host: String,
    pub port: u16,
    pub preferred_version: GameVersion,
}

#[derive(Default)]
pub struct CpsCounter {
    left_clicks: VecDeque<Instant>,
    right_clicks: VecDeque<Instant>,
}

impl CpsCounter {
    pub fn record_left_click(&mut self, now: Instant) {
        self.left_clicks.push_back(now);
    }

    pub fn record_right_click(&mut self, now: Instant) {
        self.right_clicks.push_back(now);
    }

    pub fn current_cps(&mut self, now: Instant) -> (usize, usize) {
        let one_sec = Duration::from_secs(1);
        while self.left_clicks.front().is_some_and(|&t| now.duration_since(t) > one_sec) {
            self.left_clicks.pop_front();
        }
        while self.right_clicks.front().is_some_and(|&t| now.duration_since(t) > one_sec) {
            self.right_clicks.pop_front();
        }
        (self.left_clicks.len(), self.right_clicks.len())
    }
}

#[derive(Debug, Clone)]
pub struct LunarHudModule {
    pub id: &'static str,
    pub title: &'static str,
    pub enabled: bool,
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub scale: f32,
}

impl LunarHudModule {
    pub fn move_with_magnetic_snap(&mut self, new_x: f32, new_y: f32, screen_w: f32, screen_h: f32) {
        const SNAP_THRESHOLD: f32 = 6.0;
        let center_x = (screen_w - self.width * self.scale) * 0.5;
        let center_y = (screen_h - self.height * self.scale) * 0.5;

        self.x = if (new_x - center_x).abs() <= SNAP_THRESHOLD {
            center_x
        } else {
            new_x.clamp(0.0, (screen_w - self.width * self.scale).max(0.0))
        };

        self.y = if (new_y - center_y).abs() <= SNAP_THRESHOLD {
            center_y
        } else {
            new_y.clamp(0.0, (screen_h - self.height * self.scale).max(0.0))
        };
    }
}

pub enum UiCommand {
    None,
    ConnectToSelectedServer,
    DisconnectFromServer,
    ClickChestSlot { window_id: u8, slot: i16, button: u8 },
}

pub struct CasxCompositeUi {
    // HoYoPlay State
    pub active_tab: HoYoSidebarTab,
    pub dynamic_bg_shader: bool,

    // Legacy Quick-Switch State
    pub username_input: String,
    pub active_auth: AuthProvider,
    pub selected_version: GameVersion,
    pub favorite_servers: Vec<QuickServerEntry>,
    pub selected_server_index: usize,

    // Lunar Client Overlay State
    pub in_server_world: bool,
    pub lunar_mod_overlay_open: bool,
    pub freelook_active: bool,
    pub toggle_sprint_active: bool,
    pub hide_scoreboard_red_numbers: bool,
    pub cps_tracker: CpsCounter,
    pub hud_modules: Vec<LunarHudModule>,

    // Runtime Metrics & Server State
    pub current_fps: f32,
    pub current_ping_ms: u32,
    pub active_container: Option<ContainerWindow>,
    pub recent_chat_lines: VecDeque<String>,
}

impl Default for CasxCompositeUi {
    fn default() -> Self {
        Self::new()
    }
}

impl CasxCompositeUi {
    pub fn new() -> Self {
        Self {
            active_tab: HoYoSidebarTab::ServerHub,
            dynamic_bg_shader: true,
            username_input: "CasxDuois".to_string(),
            active_auth: AuthProvider::Offline { username: "CasxDuois".to_string() },
            selected_version: GameVersion::V26_2,
            favorite_servers: vec![
                QuickServerEntry {
                    display_name: "CasxDuois Official Network (26.2)".into(),
                    host: "127.0.0.1".into(),
                    port: 25565,
                    preferred_version: GameVersion::V26_2,
                },
                QuickServerEntry {
                    display_name: "Legacy Tu Tiên & PvP Server (1.12.2)".into(),
                    host: "127.0.0.1".into(),
                    port: 25566,
                    preferred_version: GameVersion::V1_12_2,
                },
            ],
            selected_server_index: 0,
            in_server_world: false,
            lunar_mod_overlay_open: false,
            freelook_active: false,
            toggle_sprint_active: true,
            hide_scoreboard_red_numbers: true,
            cps_tracker: CpsCounter::default(),
            hud_modules: vec![
                LunarHudModule { id: "fps_ping", title: "FPS & Server Ping", enabled: true, x: 16.0, y: 16.0, width: 165.0, height: 34.0, scale: 1.0 },
                LunarHudModule { id: "keystrokes_cps", title: "Keystrokes + CPS", enabled: true, x: 16.0, y: 60.0, width: 165.0, height: 120.0, scale: 1.0 },
                LunarHudModule { id: "custom_scoreboard", title: "Lunar Scoreboard", enabled: true, x: 1080.0, y: 240.0, width: 185.0, height: 190.0, scale: 1.0 },
            ],
            current_fps: 540.0,
            current_ping_ms: 14,
            active_container: None,
            recent_chat_lines: VecDeque::from([
                "[CasxDuois] Sẵn sàng kết nối Multiplayer 1.12.2 & 26.2!".to_string(),
            ]),
        }
    }

    pub fn select_server(&mut self, index: usize) {
        if let Some(entry) = self.favorite_servers.get(index) {
            self.selected_server_index = index;
            self.selected_version = entry.preferred_version;
        }
    }

    pub fn toggle_lunar_overlay(&mut self) {
        if self.in_server_world {
            self.lunar_mod_overlay_open = !self.lunar_mod_overlay_open;
        }
    }

    /// Dựng toàn bộ giao diện 2D hợp nhất (HoYoPlay Hub / Legacy Bar / Lunar HUD)
    pub fn build_ui(&mut self, ctx: &Context) -> UiCommand {
        let mut command = UiCommand::None;
        let now = Instant::now();
        let (lmb_cps, rmb_cps) = self.cps_tracker.current_cps(now);
        let screen_rect = ctx.screen_rect();

        if !self.in_server_world {
            // ================================================================
            // CHẾ ĐỘ 1: SẢNH CHÍNH HOYOPLAY x LEGACY LAUNCHER
            // ================================================================
            egui::SidePanel::left("hoyo_sidebar")
                .exact_width(78.0)
                .frame(Frame::none().fill(Color32::from_rgba_unmultiplied(15, 18, 26, 235)).inner_margin(Margin::same(10.0)))
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add_space(12.0);
                        ui.label(RichText::new("CXD").size(22.0).strong().color(Color32::from_rgb(90, 200, 255)));
                        ui.add_space(28.0);

                        for (tab, label) in [
                            (HoYoSidebarTab::ServerHub, "HUB"),
                            (HoYoSidebarTab::LunarMods, "MODS"),
                            (HoYoSidebarTab::ResourcePacks, "PACK"),
                            (HoYoSidebarTab::EngineSettings, "CFG"),
                        ] {
                            let selected = self.active_tab == tab;
                            let btn = egui::Button::new(
                                RichText::new(label)
                                    .size(13.0)
                                    .strong()
                                    .color(if selected { Color32::WHITE } else { Color32::GRAY }),
                            )
                            .min_size(Vec2::new(56.0, 48.0))
                            .rounding(CornerRadius::same(12))
                            .fill(if selected { Color32::from_rgb(45, 115, 230) } else { Color32::from_rgb(28, 33, 46) });

                            if ui.add(btn).clicked() {
                                self.active_tab = tab;
                            }
                            ui.add_space(10.0);
                        }
                    });
                });

            // Thanh điều khiển nhanh Legacy Launcher + Nút CTA lớn kiểu HoYoPlay ở dưới cùng
            egui::TopBottomPanel::bottom("legacy_hoyo_bottom_bar")
                .exact_height(96.0)
                .frame(Frame::none().fill(Color32::from_rgba_unmultiplied(18, 22, 32, 242)).inner_margin(Margin::symmetric(24.0, 14.0)))
                .show(ctx, |ui| {
                    ui.horizontal_centered(|ui| {
                        // Cụm Legacy Launcher Quick-Switch bên trái
                        ui.vertical(|ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Tài khoản:").color(Color32::LIGHT_GRAY));
                                if ui.add(egui::TextEdit::singleline(&mut self.username_input).desired_width(140.0)).changed() {
                                    self.active_auth = AuthProvider::Offline { username: self.username_input.clone() };
                                }
                                ui.add_space(16.0);
                                ui.label(RichText::new("Phiên bản Rust Engine:").color(Color32::LIGHT_GRAY));
                                ui.selectable_value(&mut self.selected_version, GameVersion::V26_2, "26.2 Next-Gen");
                                ui.selectable_value(&mut self.selected_version, GameVersion::V1_12_2, "1.12.2 Legacy");
                            });
                            ui.add_space(8.0);
                            ui.horizontal(|ui| {
                                ui.label(RichText::new("Server Nhanh:").color(Color32::LIGHT_GRAY));
                                for idx in 0..self.favorite_servers.len() {
                                    let name = self.favorite_servers[idx].display_name.clone();
                                    if ui.selectable_label(self.selected_server_index == idx, name).clicked() {
                                        self.select_server(idx);
                                    }
                                }
                            });
                        });

                        // Nút CTA Khởi chạy lớn kiểu HoYoPlay bên phải
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let launch_btn = egui::Button::new(
                                RichText::new("KẾT NỐI SERVER")
                                    .size(18.0)
                                    .strong()
                                    .color(Color32::from_rgb(20, 24, 34)),
                            )
                            .min_size(Vec2::new(230.0, 56.0))
                            .rounding(CornerRadius::same(28))
                            .fill(Color32::from_rgb(255, 205, 65));

                            if ui.add(launch_btn).clicked() {
                                command = UiCommand::ConnectToSelectedServer;
                            }
                        });
                    });
                });

            // Khu vực trung tâm HoYoPlay Hub
            egui::CentralPanel::default()
                .frame(Frame::none().fill(Color32::from_rgba_unmultiplied(12, 15, 22, 180)).inner_margin(Margin::same(32.0)))
                .show(ctx, |ui| {
                    ui.label(RichText::new("CASXDUOIS CLIENT").size(34.0).strong().color(Color32::WHITE));
                    ui.label(
                        RichText::new("Kiến trúc Rust Multiplayer-Only thuần túy | Không Integrated Server | Khởi chạy tức thì")
                            .size(15.0)
                            .color(Color32::from_rgb(150, 205, 255)),
                    );
                    ui.add_space(24.0);

                    Frame::none()
                        .fill(Color32::from_rgba_unmultiplied(24, 30, 44, 210))
                        .rounding(CornerRadius::same(16))
                        .stroke(Stroke::new(1.0, Color32::from_rgb(60, 85, 130)))
                        .inner_margin(Margin::same(20.0))
                        .show(ui, |ui| {
                            ui.label(RichText::new("TRẠNG THÁI NHÂN MẠNG & ĐỒ HỌA").strong().color(Color32::WHITE));
                            ui.add_space(8.0);
                            ui.label(format!("• Nhân giao thức đang chọn: {}", self.selected_version.display_name()));
                            ui.label("• Bộ dựng hình: wgpu 64-Bit Packed Voxel + 6-Direction Greedy Meshing");
                            ui.label("• Giao diện trong game: Nhấn [Right Shift] để mở Lunar Client HUD & Kawase Blur");
                        });
                });
        } else {
            // ================================================================
            // CHẾ ĐỘ 2: TRONG SERVER - LUNAR CLIENT HUD WIDGETS & MOD MENU
            // ================================================================
            let can_drag = self.lunar_mod_overlay_open;
            let sw = screen_rect.width();
            let sh = screen_rect.height();

            for module in &mut self.hud_modules {
                if !module.enabled {
                    continue;
                }
                let area = egui::Area::new(egui::Id::new(module.id))
                    .fixed_pos(egui::pos2(module.x, module.y))
                    .movable(can_drag);

                let resp = area.show(ctx, |ui| {
                    Frame::none()
                        .fill(Color32::from_rgba_unmultiplied(15, 18, 25, 165))
                        .rounding(CornerRadius::same(8))
                        .stroke(if can_drag {
                            Stroke::new(1.5, Color32::from_rgb(90, 200, 255))
                        } else {
                            Stroke::NONE
                        })
                        .inner_margin(Margin::same(8.0))
                        .show(ui, |ui| match module.id {
                            "fps_ping" => {
                                ui.label(
                                    RichText::new(format!("{:.0} FPS  |  {} ms", self.current_fps, self.current_ping_ms))
                                        .strong()
                                        .color(Color32::WHITE),
                                );
                            }
                            "keystrokes_cps" => {
                                ui.vertical_centered(|ui| {
                                    ui.label(RichText::new("[ W ]").strong().color(Color32::WHITE));
                                    ui.label(RichText::new("[ A ]  [ S ]  [ D ]").strong().color(Color32::WHITE));
                                    ui.label(RichText::new("[   SPACE   ]").color(Color32::LIGHT_GRAY));
                                    ui.label(
                                        RichText::new(format!("LMB: {} CPS | RMB: {} CPS", lmb_cps, rmb_cps))
                                            .size(12.0)
                                            .color(Color32::from_rgb(120, 220, 255)),
                                    );
                                });
                            }
                            "custom_scoreboard" => {
                                ui.label(RichText::new("CASXDUOIS SERVER").strong().color(Color32::GOLD));
                                ui.separator();
                                ui.label(format!("Phiên bản: {:?}", self.selected_version));
                                ui.label(format!("Người chơi: {}", self.active_auth.username()));
                                ui.label("Khu vực: Sảnh Chính");
                            }
                            _ => {}
                        });
                });

                if can_drag && resp.response.dragged() {
                    let delta = resp.response.drag_delta();
                    module.move_with_magnetic_snap(module.x + delta.x, module.y + delta.y, sw, sh);
                }
            }

            // Bảng điều khiển Lunar Client Mod Menu (Mở bằng Right Shift)
            if self.lunar_mod_overlay_open {
                egui::Window::new("CASXDUOIS × LUNAR MOD SETTINGS (Kéo thả HUD trên màn hình)")
                    .anchor(Align2::CENTER_CENTER, Vec2::ZERO)
                    .collapsible(false)
                    .resizable(false)
                    .show(ctx, |ui| {
                        for module in &mut self.hud_modules {
                            ui.checkbox(&mut module.enabled, module.title);
                        }
                        ui.separator();
                        ui.checkbox(&mut self.toggle_sprint_active, "Toggle Sprint");
                        ui.checkbox(&mut self.hide_scoreboard_red_numbers, "Ẩn số đỏ Scoreboard");
                        ui.add_space(10.0);
                        if ui.button(RichText::new("Ngắt kết nối về Sảnh HoYoPlay").color(Color32::LIGHT_RED)).clicked() {
                            self.lunar_mod_overlay_open = false;
                            command = UiCommand::DisconnectFromServer;
                        }
                    });
            }

            // Hiển thị Rương / Menu Server nếu đang mở ContainerWindow
            if let Some(container) = &self.active_container {
                egui::Window::new(&container.title)
                    .anchor(Align2::CENTER_CENTER, Vec2::new(0.0, -40.0))
                    .collapsible(false)
                    .show(ctx, |ui| {
                        egui::Grid::new("server_chest_grid").spacing(Vec2::new(4.0, 4.0)).show(ui, |ui| {
                            for (slot_idx, item) in container.slots.iter().enumerate() {
                                let label = match item {
                                    Some(stack) => format!("#{}x{}", stack.item_id, stack.count),
                                    None => " [ ] ".to_string(),
                                };
                                if ui.button(label).clicked() {
                                    command = UiCommand::ClickChestSlot {
                                        window_id: container.window_id,
                                        slot: slot_idx as i16,
                                        button: 0,
                                    };
                                }
                                if (slot_idx + 1) % 9 == 0 {
                                    ui.end_row();
                                }
                            }
                        });
                    });
            }
        }

        command
    }
}
```

---

### 7.7. Crate 6: `crates/casx_client` (Ứng dụng Thực thi Chính: `winit` + `wgpu` + `egui` + `tokio`)

#### `crates/casx_client/Cargo.toml`

```toml
[package]
name = "casx_client"
version = "0.1.0"
edition = "2021"

[dependencies]
casx_core = { path = "../casx_core" }
proto_1_12_2 = { path = "../proto_1_12_2" }
proto_26_2 = { path = "../proto_26_2" }
casx_render = { path = "../casx_render" }
casx_ui = { path = "../casx_ui" }
bytes.workspace = true
tokio.workspace = true
rayon.workspace = true
crossbeam-channel.workspace = true
glam.workspace = true
winit.workspace = true
wgpu.workspace = true
egui.workspace = true
egui-winit.workspace = true
egui-wgpu.workspace = true
pollster.workspace = true
```

#### `crates/casx_client/src/main.rs`

```rust
use bytes::BytesMut;
use casx_core::{
    CanonicalBlockState, ChunkColumn, ChunkSection, ClientAction, ClientWorldCache,
    ConnectionState, ContainerWindow, EntityTracker, GameVersion, LocalPlayerPhysics,
    ProtocolAdapter, RemoteEntity, ServerEvent,
};
use casx_render::{GreedyVoxelMesher, SectionMesh, ViewFrustum};
use casx_ui::{CasxCompositeUi, UiCommand};
use crossbeam_channel::{unbounded, Receiver, Sender};
use glam::{DVec3, Mat4, Vec3};
use proto_1_12_2::Proto1_12_2;
use proto_26_2::Proto26_2;
use rayon::prelude::*;
use std::collections::HashSet;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use winit::application::ApplicationHandler;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

fn create_adapter(version: GameVersion) -> Box<dyn ProtocolAdapter> {
    match version {
        GameVersion::V1_12_2 => Box::new(Proto1_12_2::new()),
        GameVersion::V26_2 => Box::new(Proto26_2::new()),
    }
}

/// Luồng mạng bất đồng bộ chạy trên nền Tokio Runtime
async fn network_worker_loop(
    host: String,
    port: u16,
    username: String,
    version: GameVersion,
    tx_events: Sender<ServerEvent>,
    mut rx_actions: tokio::sync::mpsc::UnboundedReceiver<ClientAction>,
) {
    let mut adapter = create_adapter(version);
    let addr = format!("{host}:{port}");
    let Ok(mut stream) = TcpStream::connect(&addr).await else {
        let _ = tx_events.send(ServerEvent::Disconnected {
            reason: format!("Không thể kết nối tới máy chủ {addr}"),
        });
        return;
    };
    let _ = stream.set_nodelay(true);

    let mut read_buf = BytesMut::with_capacity(131_072);
    let mut write_buf = BytesMut::with_capacity(16_384);

    if adapter
        .encode_action(
            ClientAction::InitiateLogin { server_host: host, server_port: port, username },
            &mut write_buf,
        )
        .is_ok()
    {
        let _ = stream.write_all(&write_buf).await;
        write_buf.clear();
    }

    loop {
        tokio::select! {
            read_res = stream.read_buf(&mut read_buf) => {
                match read_res {
                    Ok(0) | Err(_) => {
                        let _ = tx_events.send(ServerEvent::Disconnected { reason: "Mất kết nối từ Server".into() });
                        break;
                    }
                    Ok(_) => {
                        if let Ok(events) = adapter.decode_packets(&mut read_buf, &mut write_buf) {
                            for ev in events {
                                if let ServerEvent::KeepAliveRequest { keep_alive_id } = ev {
                                    let _ = adapter.encode_action(ClientAction::KeepAliveResponse { keep_alive_id }, &mut write_buf);
                                }
                                let _ = tx_events.send(ev);
                            }
                        }
                        if !write_buf.is_empty() {
                            let _ = stream.write_all(&write_buf).await;
                            write_buf.clear();
                        }
                    }
                }
            }
            Some(action) = rx_actions.recv() => {
                if adapter.current_state() == ConnectionState::Play {
                    let _ = adapter.encode_action(action, &mut write_buf);
                    if !write_buf.is_empty() {
                        let _ = stream.write_all(&write_buf).await;
                        write_buf.clear();
                    }
                }
            }
        }
    }
}

struct GpuState {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    egui_renderer: egui_wgpu::Renderer,
}

struct CasxApp {
    window: Option<Arc<Window>>,
    gpu: Option<GpuState>,
    egui_ctx: egui::Context,
    egui_winit: Option<egui_winit::State>,

    // Các phân hệ lõi của CasxDuois Client
    ui_state: CasxCompositeUi,
    world: ClientWorldCache,
    physics: LocalPlayerPhysics,
    entities: EntityTracker,
    pressed_keys: HashSet<KeyCode>,

    // Đa luồng mạng & Vật lý
    tokio_rt: tokio::runtime::Runtime,
    tx_net_events: Sender<ServerEvent>,
    rx_net_events: Receiver<ServerEvent>,
    tx_net_actions: Option<tokio::sync::mpsc::UnboundedSender<ClientAction>>,
    last_physics_tick: Instant,
    last_frame_time: Instant,
    cached_meshes: Vec<SectionMesh>,
}

impl CasxApp {
    fn new() -> Self {
        let (tx_net_events, rx_net_events) = unbounded();
        let tokio_rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("Khởi tạo Tokio Runtime");

        // Khởi tạo sẵn một sàn Chunk mẫu 3x3 để kiểm chứng Greedy Mesher ngay khi mở Client
        let mut world = ClientWorldCache::default();
        let mut sample_col = ChunkColumn::new(0, 0, 0, 16);
        let mut sec = ChunkSection::new_empty();
        for z in 0..16 {
            for x in 0..16 {
                sec.set(x, 0, z, CanonicalBlockState(16));
            }
        }
        sample_col.sections[0] = Some(sec);
        world.insert_chunk(sample_col);

        Self {
            window: None,
            gpu: None,
            egui_ctx: egui::Context::default(),
            egui_winit: None,
            ui_state: CasxCompositeUi::new(),
            world,
            physics: LocalPlayerPhysics::default(),
            entities: EntityTracker::default(),
            pressed_keys: HashSet::new(),
            tokio_rt,
            tx_net_events,
            rx_net_events,
            tx_net_actions: None,
            last_physics_tick: Instant::now(),
            last_frame_time: Instant::now(),
            cached_meshes: Vec::new(),
        }
    }

    fn rebuild_dirty_chunks_parallel(&mut self) {
        let dirty_sections: Vec<&ChunkSection> = self
            .world
            .chunks
            .values_mut()
            .filter(|col| col.dirty_mesh)
            .flat_map(|col| {
                col.dirty_mesh = false;
                col.sections.iter().flatten()
            })
            .collect();

        if !dirty_sections.is_empty() {
            let new_meshes: Vec<SectionMesh> = dirty_sections
                .par_iter()
                .map(|sec| GreedyVoxelMesher::build_full_section_mesh(sec))
                .collect();
            self.cached_meshes = new_meshes;
        }
    }

    fn process_network_events(&mut self) {
        while let Ok(ev) = self.rx_net_events.try_recv() {
            match ev {
                ServerEvent::JoinServerWorld { .. } => {
                    self.ui_state.in_server_world = true;
                }
                ServerEvent::SynchronizePlayerPosition { x, y, z, yaw, pitch, teleport_id } => {
                    self.physics.curr_pos = DVec3::new(x, y, z);
                    self.physics.prev_pos = self.physics.curr_pos;
                    self.physics.yaw = yaw;
                    self.physics.pitch = pitch;
                    if let Some(tx) = &self.tx_net_actions {
                        let _ = tx.send(ClientAction::ConfirmTeleport { teleport_id });
                    }
                }
                ServerEvent::ChunkColumnLoad { column } => {
                    self.world.insert_chunk(column);
                }
                ServerEvent::BlockChange { x, y, z, state } => {
                    self.world.set_block(x, y, z, state);
                }
                ServerEvent::SpawnPlayer { entity_id, name, x, y, z, yaw, pitch } => {
                    self.entities.entities.insert(
                        entity_id,
                        RemoteEntity::new(entity_id, name, DVec3::new(x, y, z), yaw, pitch),
                    );
                }
                ServerEvent::EntityMove { entity_id, x, y, z, yaw, pitch, is_relative } => {
                    if let Some(ent) = self.entities.entities.get_mut(&entity_id) {
                        let target = if is_relative {
                            ent.target_pos + DVec3::new(x, y, z)
                        } else {
                            DVec3::new(x, y, z)
                        };
                        ent.push_network_target(target, yaw, pitch);
                    }
                }
                ServerEvent::DestroyEntities { entity_ids } => {
                    for id in entity_ids {
                        self.entities.entities.remove(&id);
                    }
                }
                ServerEvent::OpenContainerWindow { window_id, title, slot_count } => {
                    self.ui_state.active_container = Some(ContainerWindow::new(window_id, title, slot_count));
                }
                ServerEvent::ContainerSetItems { window_id, state_id, items } => {
                    if let Some(win) = &mut self.ui_state.active_container {
                        if win.window_id == window_id {
                            win.state_id = state_id;
                            win.slots = items;
                        }
                    }
                }
                ServerEvent::SystemChat { json_message, .. } => {
                    if self.ui_state.recent_chat_lines.len() >= 12 {
                        self.ui_state.recent_chat_lines.pop_front();
                    }
                    self.ui_state.recent_chat_lines.push_back(json_message);
                }
                ServerEvent::Disconnected { reason } => {
                    self.ui_state.in_server_world = false;
                    self.ui_state.recent_chat_lines.push_back(format!("[Ngắt kết nối] {reason}"));
                }
                _ => {}
            }
        }
    }

    fn step_20_tps_physics_if_due(&mut self) {
        let now = Instant::now();
        let tick_duration = Duration::from_millis(50);

        while now.duration_since(self.last_physics_tick) >= tick_duration {
            self.last_physics_tick += tick_duration;
            if self.ui_state.in_server_world {
                let mut forward = 0.0;
                let mut strafe = 0.0;
                if self.pressed_keys.contains(&KeyCode::KeyW) { forward += 1.0; }
                if self.pressed_keys.contains(&KeyCode::KeyS) { forward -= 1.0; }
                if self.pressed_keys.contains(&KeyCode::KeyA) { strafe += 1.0; }
                if self.pressed_keys.contains(&KeyCode::KeyD) { strafe -= 1.0; }
                let jump = self.pressed_keys.contains(&KeyCode::Space);

                self.physics.sprinting = self.ui_state.toggle_sprint_active && forward > 0.0;
                self.physics.step_physics_tick(&self.world, forward, strafe, jump);
                self.entities.tick_all();

                if let Some(tx) = &self.tx_net_actions {
                    let _ = tx.send(ClientAction::SendPositionAndRotation {
                        x: self.physics.curr_pos.x,
                        y: self.physics.curr_pos.y,
                        z: self.physics.curr_pos.z,
                        yaw: self.physics.yaw,
                        pitch: self.physics.pitch,
                        on_ground: self.physics.on_ground,
                    });
                }
            }
        }
    }
}

impl ApplicationHandler for CasxApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let attrs = Window::default_attributes()
            .with_title("CasxDuois Client | Rust Multiplayer Engine (1.12.2 & 26.2)")
            .with_inner_size(winit::dpi::LogicalSize::new(1280.0, 720.0));
        let window = Arc::new(event_loop.create_window(attrs).expect("Tạo cửa sổ winit"));

        let instance = wgpu::Instance::default();
        let surface = instance.create_surface(window.clone()).expect("Tạo wgpu surface");
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))
        .expect("Tìm GPU hỗ trợ wgpu");

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
            .expect("Tạo wgpu device");

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps.formats[0];
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let egui_winit = egui_winit::State::new(
            self.egui_ctx.clone(),
            egui::ViewportId::ROOT,
            &window,
            Some(window.scale_factor() as f32),
            None,
            None,
        );
        let egui_renderer = egui_wgpu::Renderer::new(&device, format, None, 1, false);

        self.window = Some(window);
        self.egui_winit = Some(egui_winit);
        self.gpu = Some(GpuState { surface, device, queue, config, egui_renderer });
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if let (Some(window), Some(egui_winit)) = (&self.window, &mut self.egui_winit) {
            let _ = egui_winit.on_window_event(window, &event);
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.config.width = new_size.width.max(1);
                    gpu.config.height = new_size.height.max(1);
                    gpu.surface.configure(&gpu.device, &gpu.config);
                }
            }
            WindowEvent::MouseInput { state: ElementState::Pressed, button, .. } => {
                let now = Instant::now();
                match button {
                    MouseButton::Left => self.ui_state.cps_tracker.record_left_click(now),
                    MouseButton::Right => self.ui_state.cps_tracker.record_right_click(now),
                    _ => {}
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    match event.state {
                        ElementState::Pressed => {
                            self.pressed_keys.insert(code);
                            if code == KeyCode::ShiftRight {
                                self.ui_state.toggle_lunar_overlay();
                            }
                        }
                        ElementState::Released => {
                            self.pressed_keys.remove(&code);
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(self.last_frame_time).as_secs_f32().max(1.0e-5);
                self.last_frame_time = now;
                self.ui_state.current_fps = self.ui_state.current_fps * 0.9 + (1.0 / dt) * 0.1;

                self.process_network_events();
                self.step_20_tps_physics_if_due();
                self.rebuild_dirty_chunks_parallel();

                // Tính toán nội suy vị trí Camera và kiểm tra Frustum Culling
                let alpha = (now.duration_since(self.last_physics_tick).as_secs_f64() / 0.05).clamp(0.0, 1.0);
                let eye = self.physics.interpolated_position(alpha) + DVec3::new(0.0, 1.62, 0.0);
                let view = Mat4::look_to_rh(eye.as_vec3(), Vec3::Z, Vec3::Y);
                let proj = Mat4::perspective_rh(70.0f32.to_radians(), 16.0 / 9.0, 0.1, 512.0);
                let frustum = ViewFrustum::from_view_proj(proj * view);
                let _visible_test = frustum.is_aabb_visible(Vec3::ZERO, Vec3::splat(16.0));

                if let (Some(window), Some(gpu), Some(egui_winit)) = (&self.window, &mut self.gpu, &mut self.egui_winit) {
                    let raw_input = egui_winit.take_egui_input(window);
                    let mut pending_cmd = UiCommand::None;
                    let full_output = self.egui_ctx.run(raw_input, |ctx| {
                        pending_cmd = self.ui_state.build_ui(ctx);
                    });

                    match pending_cmd {
                        UiCommand::ConnectToSelectedServer => {
                            let srv = self.ui_state.favorite_servers[self.ui_state.selected_server_index].clone();
                            let username = self.ui_state.active_auth.username().to_string();
                            let version = self.ui_state.selected_version;
                            let (tx_act, rx_act) = tokio::sync::mpsc::unbounded_channel();
                            self.tx_net_actions = Some(tx_act);
                            let tx_ev = self.tx_net_events.clone();

                            self.tokio_rt.spawn(async move {
                                network_worker_loop(srv.host, srv.port, username, version, tx_ev, rx_act).await;
                            });
                            self.ui_state.in_server_world = true;
                        }
                        UiCommand::DisconnectFromServer => {
                            self.tx_net_actions = None;
                            self.ui_state.in_server_world = false;
                        }
                        UiCommand::ClickChestSlot { window_id, slot, button } => {
                            if let Some(tx) = &self.tx_net_actions {
                                let state_id = self.ui_state.active_container.as_ref().map(|c| c.state_id).unwrap_or(0);
                                let _ = tx.send(ClientAction::ClickContainerSlot {
                                    window_id, state_id, slot, button, mode: 0, action_id: 1,
                                });
                            }
                        }
                        UiCommand::None => {}
                    }

                    egui_winit.handle_platform_output(window, full_output.platform_output);
                    let tris = self.egui_ctx.tessellate(full_output.shapes, full_output.pixels_per_point);
                    for (id, image_delta) in &full_output.textures_delta.set {
                        gpu.egui_renderer.update_texture(&gpu.device, &gpu.queue, *id, image_delta);
                    }

                    let output_frame = gpu.surface.get_current_texture().expect("Lấy khung hình Swapchain");
                    let view_tex = output_frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
                    let mut encoder = gpu.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

                    let screen_desc = egui_wgpu::ScreenDescriptor {
                        size_in_pixels: [gpu.config.width, gpu.config.height],
                        pixels_per_point: window.scale_factor() as f32,
                    };
                    gpu.egui_renderer.update_buffers(&gpu.device, &gpu.queue, &mut encoder, &tris, &screen_desc);

                    {
                        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("CasxDuois Main Render Pass"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: &view_tex,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.05, g: 0.07, b: 0.11, a: 1.0 }),
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                        });
                        gpu.egui_renderer.render(&mut rpass, &tris, &screen_desc);
                    }

                    for id in &full_output.textures_delta.free {
                        gpu.egui_renderer.free_texture(id);
                    }

                    gpu.queue.submit(std::iter::once(encoder.finish()));
                    output_frame.present();
                    window.request_redraw();
                }
            }
            _ => {}
        }
    }
}

fn main() {
    let event_loop = EventLoop::new().expect("Tạo winit EventLoop");
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = CasxApp::new();
    event_loop.run_app(&mut app).expect("Chạy vòng lặp CasxDuois Client");
}
```