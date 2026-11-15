# GHI CHÚ KỸ THUẬT HORIZON (PHẦN 2): CHIẾN LƯỢC ĐA PHIÊN BẢN (`version-ports.md`) & KIỂM THỬ ĐỐI CHIẾU (`parity-workflow.md`)

* **Tham chiếu gốc:** `horizon-rust/horizon` (`docs/version-ports.md`, `docs/parity-workflow.md`, `tools/parity.py`)
* **Áp dụng cho:** **CasxDuois Client** (`proto_1_12_2` & `proto_26_2`)

---

## 1. Giải quyết Giới hạn "Single-Version Build" của Horizon

Trong tài liệu `docs/version-ports.md` của **Horizon**, nhóm phát triển ghi chú rõ:
> *"Supporting two Minecraft versions in one build is out of scope."* (Hỗ trợ hai phiên bản Minecraft trong cùng một bản dựng nằm ngoài phạm vi của Horizon).

Lý do là Horizon tái tạo toàn bộ hệ thống sinh địa hình (WorldGen), AI quái vật và lưu trữ thế giới (Anvil DataFixers) gắn chặt với một tập hằng số tĩnh duy nhất (`crates/horizon-core/src/version.rs` và `crates/horizon-protocol/src/ids.rs`).

**CasxDuois Client** khắc phục hoàn toàn giới hạn này nhờ kiến trúc **Multiplayer-Only**:
1. Vì không chứa WorldGen hay Mob AI của từng phiên bản, phần lõi đồ họa (`casx_render`) và vật lý (`casx_core`) hoàn toàn độc lập với mã gói tin.
2. Thay vì sinh ra một tệp `ids.rs` duy nhất, bộ công cụ sinh mã của CasxDuois xuất ra hai bảng hằng số biệt lập từ báo cáo `packets.json` và `blocks.json` của từng phiên bản:
   * `crates/proto_1_12_2/src/generated_ids.rs` (sinh từ báo cáo dữ liệu `1.12.2`)
   * `crates/proto_26_2/src/generated_ids.rs` (sinh từ `java -DbundlerMainClass=net.minecraft.data.Main -jar server.jar --reports` của `26.2`)
3. Người chơi có thể chuyển đổi nóng (Hot-Switch) giữa nhân `1.12.2` và `26.2` ngay trên thanh điều khiển **Legacy Quick-Switch** chỉ bằng cách tráo đổi con trỏ `Box<dyn ProtocolAdapter>` trong $0\text{ ms}$.

---

## 2. Phương pháp Kiểm thử Đối chiếu Vi sai (Differential Parity Harnesses)

Một bài học đắt giá từ Horizon là: *đọc đặc tả giao thức trên wiki thường bỏ sót các chi tiết biên (edge cases) của server thực tế*. Horizon đạt độ tương thích $100\%$ nhờ hệ thống **Differential Parity Harnesses** (`tools/test_*_parity.py` và `game/net_harness.rs`):

$$\text{Cùng một chuỗi đầu vào } I \longrightarrow \begin{cases} \text{Oracle tham chiếu (Server / Vanilla Logic)} \longrightarrow \text{Kết quả } O_{\text{ref}} \\ \text{Rust Probe (CasxDuois Client)} \longrightarrow \text{Kết quả } O_{\text{rust}} \end{cases} \implies \text{Khẳng định } O_{\text{ref}} \equiv O_{\text{rust}}$$

### 4 Bộ Kiểm Thử Đối Chiếu Bắt Buộc Cho CasxDuois Client

1. **`net_handshake_parity`:**
   * Kiểm tra toàn bộ chuỗi gói tin từ `Handshake` $\rightarrow$ `Login` $\rightarrow$ `SetCompression` $\rightarrow$ (`Configuration` trên `26.2`) $\rightarrow$ `Play` đối với các phần mềm máy chủ phổ biến: **Paper / Purpur / Folia** (cho `26.2`), **Spigot / Paper 1.12.2**, và **Velocity / BungeeCord Proxy** (xử lý gói `LoginPluginRequest` và chuyển Server liên cụm).
2. **`physics_anticheat_parity`:**
   * Đưa chuỗi thao tác di chuyển phức tạp (chạy nước rút nhảy cầu, trượt trên băng, bơi trong nước, va chạm góc cầu thang/phiến đá) qua bộ vật lý $20\text{ TPS}$ của `casx_core` và đối chiếu từng số thực `f64` $(x, y, z, \text{on\_ground})$ với phương trình gốc của Minecraft để đảm bảo **0 cảnh báo (0 flags)** trên các hệ thống Anti-Cheat hiện đại như **GrimAC**, **Vulcan**, và **Matrix**.
3. **`inventory_click_parity`:**
   * Mô phỏng $1.000$ chuỗi thao tác bấm chuột trái, chuột phải, Shift-Click và phím số $1..9$ trong `ContainerWindow` (Rương Menu Server), kiểm tra sự khớp nối của `ActionNumber` (`1.12.2`) và `StateID` + mảng `ChangedSlots` (`26.2`).
4. **`chunk_palette_parity`:**
   * Giải mã cùng một dữ liệu Chunk chứa đủ mọi trường hợp `bits_per_entry` ($0$, $4$, $5..8$, và Direct Palette $13/15\text{ bits}$) trên cả hai phiên bản để đảm bảo không bao giờ xảy ra lỗi dịch bit (Bit-Shift Misalignment).

---

## 3. Mã nguồn Bộ Kiểm Thử Đối Chiếu Giao Thức (`crates/casx_core/tests/parity_harness.rs`)

```rust
use casx_core::codec::{decode_block_pos_1_12_2, decode_block_pos_26_2};
use casx_core::{generate_offline_uuid, ClientWorldCache, LocalPlayerPhysics};
use glam::DVec3;

#[test]
fn test_parity_block_position_encoding_between_versions() {
    // Tọa độ âm và dương tại biên giới thế giới để kiểm tra dấu (Sign Extension)
    let test_coords = [
        (0, 64, 0),
        (-15420, 255, 8912),
        (29_999_984, -64, -29_999_984),
    ];

    for (x, y, z) in test_coords {
        // Đóng gói chuẩn 1.12.2: X (26b) | Y (12b) | Z (26b)
        let packed_1_12_2 = (((x as i64) & 0x3FFFFFF) << 38)
            | (((y as i64) & 0xFFF) << 26)
            | ((z as i64) & 0x3FFFFFF);
        assert_eq!(decode_block_pos_1_12_2(packed_1_12_2 as u64), (x, y, z));

        // Đóng gói chuẩn 26.2: X (26b) | Z (26b) | Y (12b)
        let packed_26_2 = (((x as i64) & 0x3FFFFFF) << 38)
            | (((z as i64) & 0x3FFFFFF) << 12)
            | ((y as i64) & 0xFFF);
        assert_eq!(decode_block_pos_26_2(packed_26_2 as u64), (x, y, z));
    }
}

#[test]
fn test_parity_offline_uuid_v3_matches_vanilla() {
    // Kiểm chứng UUIDv3 chuẩn "OfflinePlayer:Notch" của Mojang
    let uuid = generate_offline_uuid("Notch");
    let formatted = format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (uuid >> 96) as u32,
        ((uuid >> 80) & 0xFFFF) as u16,
        ((uuid >> 64) & 0xFFFF) as u16,
        ((uuid >> 48) & 0xFFFF) as u16,
        uuid & 0xFFFF_FFFF_FFFF
    );
    assert_eq!(formatted, "b50ad385-829d-3141-a216-7e7d7539ba7f");
}

#[test]
fn test_parity_freefall_gravity_20_tps() {
    let world = ClientWorldCache::default();
    let mut physics = LocalPlayerPhysics::default();
    physics.curr_pos = DVec3::new(0.0, 100.0, 0.0);
    physics.prev_pos = physics.curr_pos;

    // Mô phỏng 10 ticks rơi tự do không có lực cản mặt đất
    for _ in 0..10 {
        physics.step_physics_tick(&world, 0.0, 0.0, false);
    }

    // Kiểm tra vận tốc rơi chuẩn theo công thức v_y = (v_y - 0.08) * 0.98
    assert!(physics.curr_pos.y < 96.5 && physics.curr_pos.y > 95.5);
    assert!(!physics.on_ground);
}
```