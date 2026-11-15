//! Crate `casx_platform` (Tang 1 trong do thi phu thuoc DAG).
//!
//! Trach nhiem:
//! - Do luong thong so phan cung va he dieu hanh (`PlatformTelemetry`).
//! - Dong ho do do phan giai nano giay va tinh toan ngan sach thoi gian khung hinh (`HighResolutionTimer`).
//! - Dinh vi thu muc cai dat goc Minecraft tren moi he dieu hanh (`detect_default_minecraft_dir`).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, RefreshKind, System};
use thiserror::Error;

/// Loi phat sinh trong phan he nen tang va phan cung
#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("[PlatformError::PathNotFound] Thu muc khong ton tai: {0:?}")]
    PathNotFound(PathBuf),

    #[error("[PlatformError::EnvVarMissing] Khong tim thay bien moi truong he thong: {0}")]
    EnvVarMissing(String),

    #[error("[PlatformError::Io] Loi I/O he thong: {0}")]
    Io(#[from] std::io::Error),
}

/// Thu thap va giam sat thong so tai nguyen phan cung theo thoi gian thuc
pub struct PlatformTelemetry {
    system: System,
    last_refresh: Instant,
    refresh_interval: Duration,
}

impl Default for PlatformTelemetry {
    fn default() -> Self {
        Self::new(Duration::from_millis(500))
    }
}

impl PlatformTelemetry {
    /// Khoi tao bo giam sat voi chu ky cap nhat toi thieu
    ///
    /// Khong cap nhat CPU moi khung hinh de tranh chi phi goi kernel OS vuot qua $0.05\text{ ms}$.
    pub fn new(refresh_interval: Duration) -> Self {
        let refresh_kind = RefreshKind::new()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything());
        let system = System::new_with_specifics(refresh_kind);
        Self {
            system,
            last_refresh: Instant::now(),
            refresh_interval,
        }
    }

    /// Cap nhat so lieu he thong neu vuot qua chu ky refresh_interval
    pub fn poll_updates(&mut self) {
        if self.last_refresh.elapsed() >= self.refresh_interval {
            self.system.refresh_cpu_all();
            self.system.refresh_memory();
            self.last_refresh = Instant::now();
        }
    }

    /// Muc su dung CPU trung binh cua tat ca cac loi (theo ty le phan tram $[0.0, 100.0]$)
    pub fn global_cpu_usage(&self) -> f32 {
        self.system.global_cpu_usage()
    }

    /// Tong dung luong RAM vat ly cua he thong tinh bang Megabytes
    pub fn total_memory_mb(&self) -> u64 {
        self.system.total_memory() / (1024 * 1024)
    }

    /// Dung luong RAM da su dung boi toan he thong tinh bang Megabytes
    pub fn used_memory_mb(&self) -> u64 {
        self.system.used_memory() / (1024 * 1024)
    }

    /// Dung luong RAM con trong cua he thong tinh bang Megabytes
    pub fn free_memory_mb(&self) -> u64 {
        self.system.free_memory() / (1024 * 1024)
    }
}

/// Dong ho do thoi gian do phan giai nano giay phuc vu cho vong lap Render va Physics Tick
///
/// Voi chu ky tick co dinh cua Minecraft $\Delta t_{\text{tick}} = 50\text{ ms}$ ($20\text{ TPS}$)
/// va ngan sach khung hinh $\Delta t_{\text{frame}} \le 1.66\text{ ms}$ ($\ge 600\text{ FPS}$).
pub struct HighResolutionTimer {
    start_time: Instant,
    last_frame_time: Instant,
    delta_seconds: f64,
    frame_durations: VecDeque<f64>,
    window_size: usize,
}

impl Default for HighResolutionTimer {
    fn default() -> Self {
        Self::new(60)
    }
}

impl HighResolutionTimer {
    /// Khoi tao bo dem thoi gian voi cua so truot de tinh FPS trung binh
    pub fn new(window_size: usize) -> Self {
        let now = Instant::now();
        Self {
            start_time: now,
            last_frame_time: now,
            delta_seconds: 0.0,
            frame_durations: VecDeque::with_capacity(window_size),
            window_size: window_size.max(1),
        }
    }

    /// Cap nhat moc thoi gian cho khung hinh moi va tra ve $\Delta t = t_k - t_{k-1}$ tinh bang giay
    pub fn tick_frame(&mut self) -> f64 {
        let now = Instant::now();
        let duration = now.duration_since(self.last_frame_time);
        self.last_frame_time = now;
        self.delta_seconds = duration.as_secs_f64();

        if self.frame_durations.len() >= self.window_size {
            self.frame_durations.pop_front();
        }
        self.frame_durations.push_back(self.delta_seconds);

        self.delta_seconds
    }

    /// Khoang thoi gian cua khung hinh gan nhat tinh bang giay
    pub fn delta_seconds(&self) -> f64 {
        self.delta_seconds
    }

    /// Khoang thoi gian cua khung hinh gan nhat tinh bang mili giay
    pub fn delta_millis(&self) -> f64 {
        self.delta_seconds * 1000.0
    }

    /// Tong thoi gian da troi qua tu khi khoi dong bo dem
    pub fn total_elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Tinh toan chi so FPS trung binh dua tren cua so truot:
    /// $$\text{FPS}_{\text{avg}} = \frac{N}{\sum_{i=1}^N \Delta t_i}$$
    pub fn average_fps(&self) -> f64 {
        if self.frame_durations.is_empty() {
            return 0.0;
        }
        let total: f64 = self.frame_durations.iter().sum();
        if total <= 0.0 {
            0.0
        } else {
            self.frame_durations.len() as f64 / total
        }
    }
}

/// Dinh vi thu muc cai dat Minecraft goc tren he dieu hanh dang chay
///
/// Uu tien bien moi truong `CASX_MINECRAFT_DIR` neu duoc nguoi dung thiet lap thu cong.
pub fn detect_default_minecraft_dir() -> Option<PathBuf> {
    if let Ok(custom) = std::env::var("CASX_MINECRAFT_DIR") {
        let p = PathBuf::from(custom);
        if p.exists() {
            return Some(p);
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let p = PathBuf::from(appdata).join(".minecraft");
            if p.exists() {
                return Some(p);
            }
        }
    }

    #[cfg(target_os = "linux")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let p = PathBuf::from(home).join(".minecraft");
            if p.exists() {
                return Some(p);
            }
        }
    }

    #[cfg(target_os = "macos")]
    {
        if let Ok(home) = std::env::var("HOME") {
            let p = PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("minecraft");
            if p.exists() {
                return Some(p);
            }
        }
    }

    None
}

/// Xac thuc thu muc ton tai tren o dia va tra ve duong dan tuyet doi
pub fn verify_directory_exists(path: &Path) -> Result<PathBuf, PlatformError> {
    if path.is_dir() {
        path.canonicalize().map_err(PlatformError::Io)
    } else {
        Err(PlatformError::PathNotFound(path.to_path_buf()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread::sleep;

    #[test]
    fn test_high_resolution_timer_delta_and_fps() {
        let mut timer = HighResolutionTimer::new(10);
        assert_eq!(timer.average_fps(), 0.0);

        // Gia lap 3 khung hinh voi khoang nghi 5ms
        for _ in 0..3 {
            sleep(Duration::from_millis(5));
            let delta = timer.tick_frame();
            assert!(delta >= 0.004, "Delta time phai >= 4ms");
            assert!(timer.delta_millis() >= 4.0);
        }

        let fps = timer.average_fps();
        assert!(fps > 0.0 && fps <= 300.0, "FPS tinh toan phai nam trong dai hop ly: {}", fps);
    }

    #[test]
    fn test_telemetry_reads_memory() {
        let mut telemetry = PlatformTelemetry::default();
        telemetry.poll_updates();
        let total = telemetry.total_memory_mb();
        assert!(total > 0, "Tong dung luong RAM he thong phai lon hon 0 MB");
    }

    #[test]
    fn test_verify_directory_exists_on_current_dir() {
        let current_dir = Path::new(".");
        let verified = verify_directory_exists(current_dir);
        assert!(verified.is_ok(), "Thu muc hien tai phai xac thuc thanh cong");
    }

    #[test]
    fn test_verify_directory_not_found() {
        let non_existent = Path::new("path_khong_he_ton_tai_12345");
        let result = verify_directory_exists(non_existent);
        assert!(result.is_err(), "Duong dan khong ton tai phai tra ve loi");
    }
}
