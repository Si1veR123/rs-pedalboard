#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::{setup, after_setup};

#[cfg(target_os = "windows")]
mod windows;
use rs_pedalboard::processor_settings::ProcessorSettingsSave;
#[cfg(target_os = "windows")]
use windows::{setup, after_setup};

mod audio_processor;
mod sample_conversion;
mod stream_config;
mod audio_callback;
mod socket;
mod device_select;
mod tuner;
mod metronome_player;
mod volume_monitor;
mod volume_normalization;
mod settings;
mod recording;
use settings::{ProcessorSettings, ProcessorArguments};

use cpal::traits::StreamTrait;
use smol::channel::bounded;
use clap::Parser;
use rs_pedalboard::init_tracing;

const LOG_FILE: &str = "pedalboard-processor.log";

pub fn init_panic_logging() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("panic: {info:?}");
        default_hook(info);
    }));
}

fn main() {
    init_tracing(LOG_FILE);
    tracing::info!("Started logging...");
    init_panic_logging();

    let settings = ProcessorSettings::new(ProcessorArguments::parse(), Some(ProcessorSettingsSave::load_or_default()));
    tracing::info!("Processor settings: {:?}", settings);

    let (_host, input, output) = setup(
        settings.input_device.as_ref().map(|s| s.as_str()),
        settings.output_device.as_ref().map(|s| s.as_str()),
        &settings
    );

    let (socket_command_sender, audio_command_receiver) = bounded(12);
    let (audio_command_sender, socket_command_receiver) = bounded(12);

    let (in_stream, (out_stream, out_channels)) = audio_callback::create_linked_streams(
        input,
        output,
        audio_command_receiver,
        audio_command_sender,
        settings
    );

    in_stream.play().expect("Failed to play input stream");
    out_stream.play().expect("Failed to play output stream");

    after_setup(out_channels);

    // Will loop infinitely (unless panic)
    socket::ProcessorSocket::new(29475, socket_command_sender, socket_command_receiver).start().expect("Failed to start processor");
}
