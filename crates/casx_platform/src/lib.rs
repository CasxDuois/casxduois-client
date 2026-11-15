//! Crate `casx_platform` (Layer 1 in DAG).
//!
//! Responsibilities:
//! - Hardware telemetry and system monitoring (`PlatformTelemetry`).
//! - High-resolution nanosecond timer and frame budget accumulator (`HighResolutionTimer`).
//! - Standard OS Minecraft directory detection (`detect_default_minecraft_dir`).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use sysinfo::{CpuRefreshKind, MemoryRefreshKind, ProcessRefreshKind, RefreshKind, System};
use thiserror::Error;

/// Platform and hardware errors.
#[derive(Debug, Error)]
pub enum PlatformError {
    #[error("Directory not found: {0:?}")]
    PathNotFound(PathBuf),

    #[error("Environment variable missing: {0}")]
    EnvVarMissing(String),

    #[error("System I/O error: {0}")]
    Io(#[from] std::io::Error),
}

/// Collects and monitors system hardware metrics.
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
    /// Initializes system telemetry with a minimum refresh interval.
    pub fn new(refresh_interval: Duration) -> Self {
        let refresh_kind = RefreshKind::new()
            .with_cpu(CpuRefreshKind::everything())
            .with_memory(MemoryRefreshKind::everything())
            .with_processes(ProcessRefreshKind::everything());
        let system = System::new_with_specifics(refresh_kind);
        Self {
            system,
            last_refresh: Instant::now(),
            refresh_interval,
        }
    }

    /// Polls system updates if the refresh interval has elapsed.
    pub fn poll_updates(&mut self) {
        if self.last_refresh.elapsed() >= self.refresh_interval {
            self.system.refresh_cpu_all();
            self.system.refresh_memory();
            self.last_refresh = Instant::now();
        }
    }

    /// Global CPU utilization across all cores in percent [0.0, 100.0].
    pub fn global_cpu_usage(&self) -> f32 {
        self.system.global_cpu_usage()
    }

    /// Total system physical memory in Megabytes.
    pub fn total_memory_mb(&self) -> u64 {
        self.system.total_memory() / (1024 * 1024)
    }

    /// System used memory in Megabytes.
    pub fn used_memory_mb(&self) -> u64 {
        self.system.used_memory() / (1024 * 1024)
    }

    /// System free memory in Megabytes.
    pub fn free_memory_mb(&self) -> u64 {
        self.system.free_memory() / (1024 * 1024)
    }

    /// Process resident set size (RSS) memory in Kilobytes.
    pub fn process_memory_kb(&mut self) -> u64 {
        use sysinfo::Pid;
        let pid = Pid::from_u32(std::process::id());
        self.system
            .refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]));
        if let Some(process) = self.system.process(pid) {
            process.memory() / 1024
        } else {
            0
        }
    }
}

/// High-resolution nanosecond timer for rendering and physics loops.
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
    /// Initializes timer with a sliding window for moving average FPS calculation.
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

    /// Updates timestamp and returns delta seconds.
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

    /// Elapsed time of the latest frame in seconds.
    pub fn delta_seconds(&self) -> f64 {
        self.delta_seconds
    }

    /// Elapsed time of the latest frame in milliseconds.
    pub fn delta_millis(&self) -> f64 {
        self.delta_seconds * 1000.0
    }

    /// Total time elapsed since timer creation.
    pub fn total_elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }

    /// Calculates moving average FPS.
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

/// Locates default Minecraft installation directory for the current platform.
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

/// Verifies directory existence and canonicalizes path.
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

        for _ in 0..3 {
            sleep(Duration::from_millis(5));
            let delta = timer.tick_frame();
            assert!(delta >= 0.004, "Delta time must be >= 4ms");
            assert!(timer.delta_millis() >= 4.0);
        }

        let fps = timer.average_fps();
        assert!(
            fps > 0.0 && fps <= 300.0,
            "Average FPS within reasonable range: {}",
            fps
        );
    }

    #[test]
    fn test_telemetry_reads_memory() {
        let mut telemetry = PlatformTelemetry::default();
        telemetry.poll_updates();
        let total = telemetry.total_memory_mb();
        assert!(total > 0, "System memory must be greater than 0 MB");
    }

    #[test]
    fn test_verify_directory_exists_on_current_dir() {
        let current_dir = Path::new(".");
        let verified = verify_directory_exists(current_dir);
        assert!(
            verified.is_ok(),
            "Current directory should verify successfully"
        );
    }

    #[test]
    fn test_verify_directory_not_found() {
        let non_existent = Path::new("path_does_not_exist_12345");
        let result = verify_directory_exists(non_existent);
        assert!(result.is_err(), "Non-existent path should return an error");
    }
}
