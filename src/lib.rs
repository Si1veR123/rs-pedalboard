pub mod pedalboard;
pub mod pedalboard_set;
pub mod pedals;
pub mod dsp_algorithms;
pub mod plugin;
pub mod socket_helper;
pub mod audio_devices;
pub mod server_settings;

pub const SAVE_DIR: &str = "rs_pedalboard";

pub(crate) fn unique_time_id() -> usize {
    let now = std::time::SystemTime::now();
    let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap();
    let nanoseconds = duration.subsec_nanos() as usize;
    nanoseconds
}