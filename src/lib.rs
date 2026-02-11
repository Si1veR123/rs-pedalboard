use std::time::{Duration, UNIX_EPOCH, SystemTime};

use std::path::{Path, PathBuf};

pub mod pedalboard;
pub mod pedalboard_set;
pub mod pedals;
pub mod dsp_algorithms;
pub mod plugin;
pub mod socket_helper;
pub mod audio_devices;
pub mod processor_settings;
pub mod processor_api;

pub const SAVE_DIR: &str = "rs_pedalboard";
// Required by both processor and client so define it here
pub const DEFAULT_VOLUME_MONITOR_UPDATE_RATE: Duration = Duration::from_millis(100);

// For pedals such as EQ/Compressor, or when volume monitors are active, how often to update the UI
pub const DEFAULT_REFRESH_DURATION: Duration = Duration::from_millis(33); // 30 FPS

pub fn unique_time_id() -> u32 {
    let now = SystemTime::now();
    let duration = now.duration_since(UNIX_EPOCH).unwrap();
    let nanoseconds = duration.subsec_nanos();
    nanoseconds
}

pub fn forward_slash_path<P: AsRef<Path>>(path: P) -> PathBuf {
    let s = path
        .as_ref()
        .to_str()
        .expect("Path contains invalid UTF-8");
    PathBuf::from(s.replace('\\', "/"))
}

use tracing_subscriber::{fmt::{format::Writer, time::FormatTime, self}, filter::EnvFilter, prelude::*};
use std::io;
use std::fs::File;

pub struct TimeOnlyFormat;
impl FormatTime for TimeOnlyFormat {
    fn format_time(&self, w: &mut Writer) -> std::fmt::Result {
        let now = chrono::Local::now();
        write!(w, "{}", now.format("%H:%M:%S"))
    }
}

pub fn init_tracing(file_path: &str) {
    // Console layer
    let console_filter_layer = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let stdout_layer = fmt::layer()
        .with_writer(io::stdout)
        .with_target(false)
        .with_timer(TimeOnlyFormat)
        .with_filter(console_filter_layer);

    // File layer
    let file_filter_layer = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("debug"));

    let file = File::create(file_path)
        .expect("Failed to create log file");
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
