use std::time::{Duration, UNIX_EPOCH, SystemTime};

pub mod pedalboard;
pub mod pedalboard_set;
pub mod pedals;
pub mod dsp_algorithms;
pub mod plugin;
pub mod socket_helper;
pub mod audio_devices;
pub mod server_settings;

pub const SAVE_DIR: &str = "rs_pedalboard";
// Required by both server and client so define it here
pub const DEFAULT_VOLUME_MONITOR_UPDATE_RATE: Duration = Duration::from_millis(100);

pub(crate) fn unique_time_id() -> usize {
    let now = SystemTime::now();
    let duration = now.duration_since(UNIX_EPOCH).unwrap();
    let nanoseconds = duration.subsec_nanos() as usize;
    nanoseconds
}