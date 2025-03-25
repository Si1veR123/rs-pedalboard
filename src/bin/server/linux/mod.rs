mod audio_devices;
mod jack_server;

use cpal::{Host, Device};
use crate::{FRAMES_PER_PERIOD, PERIODS_PER_BUFFER};

pub fn setup() -> (Host, Device, Device) {
    let (in_device, out_device) = audio_devices::io_device_selector();

    jack_server::start_jack_server(FRAMES_PER_PERIOD, PERIODS_PER_BUFFER, in_device, out_device).expect("Failed to start JACK server");
    jack_server::jack_server_wait(true);

    jack_server::get_jack_host()
}

pub fn after_setup() {
    jack_server::stereo_output();
}
