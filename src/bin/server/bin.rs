#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::{setup, after_setup};

#[cfg(target_os = "windows")]
mod windows;
use rs_pedalboard::server_settings::ServerSettingsSave;
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
use settings::{ServerSettings, ServerArguments};

use cpal::traits::StreamTrait;
use smol::channel::bounded;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, Layer, filter::EnvFilter};
use clap::Parser;
use std::{fs::File, io};

const LOG_FILE: &str = "pedalboard-server.log";

pub fn init_tracing() {
    // Console layer
    let console_filter_layer = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let stdout_layer = fmt::layer()
        .with_writer(io::stdout)
        .with_target(false)
        .with_filter(console_filter_layer);

    // File layer
    let file_filter_layer = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("debug"));

    let file = File::create(LOG_FILE)
        .expect("Failed to create log file");
    let file_layer = fmt::layer()
        .with_writer(file)
        .with_ansi(false)
        .with_target(true)
        .with_filter(file_filter_layer);

    tracing_subscriber::registry()
        .with(stdout_layer)
        .with(file_layer)
        .init();
}

pub fn init_panic_logging() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("panic: {info:?}");
        default_hook(info);
    }));
}

fn main() {
    init_tracing();
    tracing::info!("Started logging...");
    init_panic_logging();
    
    let settings = ServerSettings::new(ServerArguments::parse(), Some(ServerSettingsSave::load_or_default()));
    tracing::info!("Server settings: {:?}", settings);

    let (_host, input, output) = setup(
        settings.input_device.as_ref().map(|s| s.as_str()),
        settings.output_device.as_ref().map(|s| s.as_str()),
        &settings
    );

    let (socket_command_sender, audio_command_receiver) = bounded(12);
    let (audio_command_sender, socket_command_receiver) = bounded(12);

    let (in_stream, out_stream) = audio_callback::create_linked_streams(
        input,
        output,
        audio_command_receiver,
        audio_command_sender,
        settings
    );

    in_stream.play().expect("Failed to play input stream");
    out_stream.play().expect("Failed to play output stream");

    after_setup();

    // Will loop infinitely (unless panic)
    socket::ServerSocket::new(29475, socket_command_sender, socket_command_receiver).start().expect("Failed to start server");
}
