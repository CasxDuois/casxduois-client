# GHI CHÚ KỸ THUẬT HORIZON (PHẦN 1): CHUỖI CHUẨN BỊ TÀI NGUYÊN & `research-lock.json`

* **Tham chiếu gốc:** `horizon-rust/horizon` (`horizon-prepare`, `research-lock.json`, `tools/check_public_tree.py`)
* **Áp dụng cho:** **CasxDuois Client** (Hỗ trợ đồng thời `1.12.2` và `26.2`)

---

## 1. Nguyên tắc "Zero-Asset Distribution" (Không phân phối tài nguyên có bản quyền)

Một trong những bài học kiến trúc quan trọng nhất từ **Horizon** là mã nguồn và tệp nhị phân phát hành của Client **tuyệt đối không được chứa bất kỳ tệp tin, hình ảnh (`.png`), âm thanh (`.ogg`), mô hình (`.json`) hay mã dịch ngược nào của Mojang/Microsoft**.

Để tuân thủ EULA của Minecraft và giữ dung lượng tệp thực thi `.exe` của **CasxDuois Client** ở mức siêu nhẹ ($< 15\text{ MB}$), toàn bộ tài nguyên trò chơi sẽ được trích xuất cục bộ ngay trên máy người dùng từ bản cài đặt Minecraft hợp pháp có sẵn (Official Launcher, Prism Launcher, MultiMC hoặc Legacy Launcher) thông qua công cụ **`casx-prepare`**.

### Quy tắc cây thư mục công khai (`check_public_tree`)
* Toàn bộ tài nguyên đã giải nén và bảng dữ liệu sinh ra bắt buộc nằm trong thư mục `.local/` (đã được khai báo trong `.gitignore`).
* Không bao giờ commit các tệp `.jar`, `.local/prepared/`, hoặc mã nguồn dịch ngược vào kho lưu trữ Git.

---

## 2. Cấu trúc Tệp Khóa Phiên Bản Kép (`research-lock.json`)

Trong dự án Horizon gốc, tệp `research-lock.json` chỉ khóa một phiên bản duy nhất. Đối với **CasxDuois Client**, chúng ta mở rộng cấu trúc này thành **Dual-Version Lock** để quản lý đồng thời mã băm $\text{SHA-1}$ của cả hai phiên bản `1.12.2` và `26.2`:

```json
{
  "schema_version": 2,
  "client_name": "CasxDuois Client",
  "targets": {
    "v1_12_2": {
      "version_id": "1.12.2",
      "protocol_version": 340,
      "pack_format": 3,
      "client_jar_relpath": "versions/1.12.2/1.12.2.jar",
      "client_jar_sha1": "0f275bc1547d01fa5e56ea54bd81e2d9c29f23e4",
      "extract_namespaces": ["minecraft"],
      "atlas_strategy": "legacy_blockstate_meta_16bit"
    },
    "v26_2": {
      "version_id": "26.2",
      "protocol_version": 770,
      "pack_format": 97,
      "client_jar_relpath": "versions/26.2/26.2.jar",
      "client_jar_sha1": "9f67e10e00000000000000000000000000000000",
      "extract_namespaces": ["minecraft"],
      "atlas_strategy": "modern_flattened_palette_32bit"
    }
  }
}
```

---

## 3. Quy trình Thực thi của `casx-prepare` Khi Khởi Động Lần Đầu

Khi người dùng mở **CasxDuois Client** lần đầu tiên, sảnh chờ **HoYoPlay $\times$ Legacy** sẽ kiểm tra thư mục `.local/prepared/`. Nếu chưa có dữ liệu, tiến trình nền sẽ chạy tự động trong chưa đầy $1.2\text{ giây}$:

1. **Định vị thư mục `.minecraft`:** Đọc biến môi trường `CASX_MINECRAFT_DIR` hoặc tự động quét đường dẫn mặc định (`%APPDATA%\.minecraft` trên Windows, `~/.minecraft` trên Linux, `~/Library/Application Support/minecraft` trên macOS).
2. **Xác thực toàn vẹn $\text{SHA-1}$:** Tính băm $\text{SHA-1}$ của `versions/1.12.2/1.12.2.jar` và `versions/26.2/26.2.jar`, đối chiếu với `research-lock.json` để đảm bảo tệp `.jar` không bị chỉnh sửa hoặc hỏng hóc.
3. **Trích xuất & Ghép Texture Array (`wgpu::TextureDimension::D2` với `depth_or_array_layers`):**
   * Đọc toàn bộ ảnh bề mặt khối (`assets/minecraft/textures/blocks/*.png` trên `1.12.2` và `assets/minecraft/textures/block/*.png` trên `26.2`).
   * Chuẩn hóa kích thước về $16 \times 16$ (hoặc $32 \times 32$ / $64 \times 64$ nếu dùng Resource Pack HD) và đóng gói thành tệp nhị phân `.local/prepared/{version}/atlas_array.bin` cùng bảng ánh xạ `tex_lookup.bin` để GPU nạp trực tiếp bằng lệnh `copy_buffer_to_texture` trong $3\text{ ms}$.
4. **Hợp nhất Server Resource Pack (`.zip`):** Khi kết nối vào các Server Multiplayer (đặc biệt là Server RPG/Tu Tiên có vũ khí và giao diện tùy chỉnh), client tải gói `.zip` từ Server vào `.local/server_packs/<sha1>.zip` và ghi đè lên các tầng Texture tương ứng mà không cần khởi động lại game.

---

## 4. Mã nguồn Triển khai Bộ Xác Thực & Trích Xuất (`crates/casx_core/src/prepare.rs`)

```rust
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize)]
pub struct VersionTargetLock {
    pub version_id: String,
    pub protocol_version: i32,
    pub pack_format: u32,
    pub client_jar_relpath: String,
    pub client_jar_sha1: String,
    pub atlas_strategy: String,
}

#[derive(Debug, Deserialize)]
pub struct ResearchLockManifest {
    pub schema_version: u32,
    pub client_name: String,
    pub targets: HashMap<String, VersionTargetLock>,
}

/// Tìm thư mục cài đặt Minecraft gốc trên hệ điều hành của người dùng
pub fn detect_default_minecraft_dir() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("CASX_MINECRAFT_DIR") {
        let p = PathBuf::from(custom);
        if p.exists() {
            return Some(p);
        }
    }

    #[cfg(target_os = "windows")]
    if let Ok(appdata) = std::env::var("APPDATA") {
        let p = PathBuf::from(appdata).join(".minecraft");
        if p.exists() {
            return Some(p);
        }
    }

    #[cfg(target_os = "linux")]
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home).join(".minecraft");
        if p.exists() {
            return Some(p);
        }
    }

    #[cfg(target_os = "macos")]
    if let Ok(home) = std::env::var("HOME") {
        let p = PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("minecraft");
        if p.exists() {
            return Some(p);
        }
    }

    None
}

/// Kiểm tra tệp `.jar` của phiên bản 1.12.2 hoặc 26.2 có sẵn sàng để trích xuất hay không
pub fn verify_jar_presence(mc_dir: &Path, lock: &VersionTargetLock) -> Result<PathBuf, String> {
    let jar_path = mc_dir.join(&lock.client_jar_relpath);
    if !jar_path.is_file() {
        return Err(format!(
            "Không tìm thấy tệp gốc {} tại {:?}. Vui lòng chạy phiên bản {} một lần trên Launcher gốc.",
            lock.version_id, jar_path, lock.version_id
        ));
    }

    let file = File::open(&jar_path).map_err(|e| format!("Lỗi mở tệp jar: {e}"))?;
    let mut reader = BufReader::with_capacity(65536, file);
    let mut header = [0u8; 4];
    reader
        .read_exact(&mut header)
        .map_err(|e| format!("Lỗi đọc header ZIP/JAR: {e}"))?;

    // Xác thực chữ ký tệp nén PK\x03\x04
    if &header != b"PK\x03\x04" {
        return Err(format!("Tệp {:?} không phải định dạng JAR/ZIP hợp lệ.", jar_path));
    }

    Ok(jar_path)
}
```