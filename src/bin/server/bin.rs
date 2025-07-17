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
mod settings;
use settings::{ServerSettings, ServerArguments};

use cpal::traits::StreamTrait;
use crossbeam::channel::bounded;
use simplelog::*;
use clap::Parser;
use std::{fs::{File, OpenOptions}, io::Write};

const LOG_FILE: &str = "pedalboard-server.log";

fn insert_log_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let panic_str = if let Some(&s) = info.payload().downcast_ref::<&str>() {
            Some(s)
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            Some(s.as_str())
        } else {
            None
        };

        let panic_message = match panic_str {
            Some(s) => format!("Panic occurred. Message: {}. PanicHookInfo: {:?}\n", s, info),
            None => format!("Panic occurred: {:?}\n", info),
        };

        OpenOptions::new()
            .create(true)
            .append(true)
            .open(LOG_FILE)
            .expect("Failed to open log file")
            .write_all(panic_message.as_bytes())
            .expect("Failed to write to log file");

        default_hook(info);
    }));
}

fn main() {
    CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
            WriteLogger::new(LevelFilter::Debug, Config::default(), File::create(LOG_FILE).expect("Failed to create log file")),
        ]
    ).expect("Failed to start logging");
    log::info!("Started logging...");

    insert_log_panic_hook();
    
    let settings = ServerSettings::new(ServerArguments::parse(), ServerSettingsSave::load().ok());
    log::info!("Server settings: {:?}", settings);

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
