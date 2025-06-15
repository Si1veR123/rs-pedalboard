pub mod pedalboard;
pub mod pedalboard_set;
pub mod pedals;
pub mod dsp_algorithms;
pub mod plugin;

pub(crate) fn unique_time_id() -> usize {
    let now = std::time::SystemTime::now();
    let duration = now.duration_since(std::time::UNIX_EPOCH).unwrap();
    let nanoseconds = duration.subsec_nanos() as usize;
    nanoseconds
}