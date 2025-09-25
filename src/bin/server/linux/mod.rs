mod jack_server;

use cpal::{Host, Device};
use rs_pedalboard::audio_devices::{get_input_devices, get_output_devices};
use super::device_select::device_select_menu;
use crate::settings::ServerSettings;

pub fn setup(input: Option<&str>, output: Option<&str>, args: &ServerSettings) -> (Host, Device, Device) {
    let input_devices = get_input_devices(None).expect("Failed to get input devices");
    let output_devices = get_output_devices(None).expect("Failed to get output devices");

    let in_device = match input {
        Some(name) => input_devices.iter().find(|&d| d == &name)
            .expect("Input device not found").clone(),
        None => {
            println!("Input Devices:");
            device_select_menu(&input_devices)
        }
    };

    let out_device = match output {
        Some(name) => output_devices.iter().find(|&d| d == &name)
            .expect("Output device not found").clone(),
        None => {
            println!("Output Devices:");
            device_select_menu(&output_devices)
        }
    };

    tracing::info!("Selected ALSA Devices: Input {in_device}, Output {out_device}");

    jack_server::start_jack_server(args.frames_per_period, args.periods_per_buffer, args.preferred_sample_rate.unwrap_or(48000), in_device, out_device).expect("Failed to start JACK server");
    jack_server::jack_server_wait(true);

    jack_server::get_jack_host()
}

pub fn after_setup(out_channels: cpal::ChannelCount) {
    if channels == 1 {
        jack_server::stereo_output();
    }
}
