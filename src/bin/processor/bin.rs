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
mod file_processor;
use file_processor::process_audio_file;
use settings::{ProcessorSettings, ProcessorArguments};

use cpal::traits::StreamTrait;
use smol::channel::bounded;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, Layer, filter::EnvFilter};
use clap::Parser;
use std::{fs::File, io};

const LOG_FILE: &str = "pedalboard-processor.log";

pub fn init_tracing() {
    // Console layer
    let console_filter_layer = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let stdout_layer = fmt::layer()
        .with_writer(io::stdout)
        .with_target(false)
        .with_timer(rs_pedalboard::TimeOnlyFormat)
        .with_filter(console_filter_layer);

    // File layer
    let file_filter_layer = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("debug"));

    let file = File::create(LOG_FILE)
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
    
    // Check if we are processing a file (1st arg == 'process')
    if let Some(first_arg) = std::env::args().nth(1) {
        if first_arg == "process" {
            let args: Vec<String> = std::env::args().collect();
            if args.len() != 6 {
                tracing::info!("Received {} arguments, expected 5.", args.len() - 1);
                tracing::info!("Usage: {} process <src_wav_path> <to_wav_path> <sample_rate> <pedalboard_json>", args[0]);
                std::process::exit(1);
            }

            let src_path = std::path::Path::new(&args[2]);
            let to_path = std::path::Path::new(&args[3]);
            let sample_rate: f32 = args[4].parse().expect("Sample rate is not a valid float");

            let pedalboard_str = &args[5].trim();
            let mut pedalboard: rs_pedalboard::pedalboard::Pedalboard;
            if pedalboard_str.ends_with(".json") {
                let file = File::open(pedalboard_str).expect("Failed to open pedalboard JSON file");
                pedalboard = serde_json::from_reader(file).expect("Failed to deserialize pedalboard JSON from file");
            } else {
                pedalboard = serde_json::from_str(&args[5]).expect("Failed to deserialize pedalboard JSON");
            }

            match process_audio_file(src_path, to_path, &mut pedalboard, sample_rate) {
                Ok(_) => {
                    tracing::info!("Successfully processed audio file and saved to {}", to_path.display());
                    std::process::exit(0);
                },
                Err(e) => {
                    tracing::error!("Failed to process audio file: {}", e);
                    std::process::exit(1);
                }
            }
        }
    }

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
