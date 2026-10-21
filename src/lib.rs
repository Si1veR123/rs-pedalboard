use std::time::{Duration, SystemTime, UNIX_EPOCH};

use std::path::{Path, PathBuf};

pub mod audio_devices;
pub mod dsp;
pub mod pedalboard;
pub mod pedalboard_set;
pub mod pedals;
pub mod plugin;
pub mod processor_api;
pub mod processor_settings;
pub mod socket_helper;

pub const SAVE_DIR: &str = "rs_pedalboard";
// Required by both processor and client so define it here
pub const DEFAULT_VOLUME_MONITOR_UPDATE_RATE: Duration = Duration::from_millis(100);

// For pedals such as EQ/Compressor, or when volume monitors are active, how often to update the UI
pub const DEFAULT_REFRESH_DURATION: Duration = Duration::from_millis(33); // 30 FPS

pub fn set_use_vst2_global_host(use_global_host: bool) {
    plugin::vst2::set_use_vst2_global_host(use_global_host);
}

pub fn unique_time_id() -> u32 {
    let now = SystemTime::now();
    let duration = now.duration_since(UNIX_EPOCH).unwrap();
    let nanoseconds = duration.subsec_nanos();
    nanoseconds
}

pub fn forward_slash_path<P: AsRef<Path>>(path: P) -> PathBuf {
    let s = path.as_ref().to_str().expect("Path contains invalid UTF-8");
    PathBuf::from(s.replace('\\', "/"))
}

pub fn get_save_dir() -> Result<PathBuf, std::io::Error> {
    let dir_path = homedir::my_home()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
        .unwrap()
        .join(SAVE_DIR);

    if !dir_path.exists() {
        std::fs::create_dir_all(&dir_path)?;
    }

    Ok(dunce::canonicalize(&dir_path).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?)
}

use std::fs::File;
use std::io;
use tracing_subscriber::{
    filter::EnvFilter,
    fmt::{self, format::Writer, time::FormatTime},
    prelude::*,
};

pub struct TimeOnlyFormat;
impl FormatTime for TimeOnlyFormat {
    fn format_time(&self, w: &mut Writer) -> std::fmt::Result {
        let now = chrono::Local::now();
        write!(w, "{}", now.format("%H:%M:%S"))
    }
}

pub fn init_tracing(file_path: &str) {
    // Console layer
    let console_filter_layer =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let stdout_layer = fmt::layer()
        .with_writer(io::stdout)
        .with_target(false)
        .with_timer(TimeOnlyFormat)
        .with_filter(console_filter_layer);

    // File layer
    let file_filter_layer =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("debug"));

    let file = File::create(file_path).expect("Failed to create log file");
    let file_layer = fmt::layer()
        .with_writer(file)
        .with_thread_names(true)
        .with_ansi(false)
        .with_target(true)
        .with_filter(file_filter_layer);

    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(file_layer)
        .init();
}

use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// By storing both whether the user has muted and whether we are muted due to tuner
/// the previous mute state can be restored when the tuner is deactivated
pub struct MuteStatus {
    pub is_muted: bool,
    pub is_muted_tuner: bool, // muted because tuner is active
}

impl MuteStatus {
    pub fn new() -> Self {
        Self {
            is_muted: false,
            is_muted_tuner: false,
        }
    }

    pub fn is_muted(&self) -> bool {
        self.is_muted || self.is_muted_tuner
    }

    pub fn toggled(&self) -> Self {
        Self {
            is_muted: !self.is_muted,
            is_muted_tuner: self.is_muted_tuner,
        }
    }
}
