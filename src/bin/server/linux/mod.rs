mod audio_devices;
mod jack_server;

pub fn setup(&mut self) -> (Host, Device, Device) {
    let (in_device, out_device) = audio_devices::io_device_selector();

    jack_server::start_jack_server(JACK_FRAMES_PER_PERIOD, JACK_PERIODS_PER_BUFFER, in_device, out_device).expect("Failed to start JACK server");
    jack_server::jack_server_wait();

    jack_server::get_jack_host()
}

pub fn after_setup(&mut self) {
    jack_server::stereo_output();
}
