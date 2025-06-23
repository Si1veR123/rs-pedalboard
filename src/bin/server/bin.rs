#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::{setup, after_setup};

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows::{setup, after_setup};

#[cfg(target_os = "linux")]
mod constants {
    pub const DEFAULT_FRAMES_PER_PERIOD: &'static str = "256";
    pub const DEFAULT_RING_BUFFER_LATENCY_MS: &'static str = "5.0";
}
#[cfg(target_os = "windows")]
mod constants {
    pub const DEFAULT_FRAMES_PER_PERIOD: &'static str = "512";
    pub const DEFAULT_RING_BUFFER_LATENCY_MS: &'static str = "7.5";
}

mod audio_io;
mod socket;
mod device_select;
mod tuner;

use cpal::traits::StreamTrait;
use crossbeam::channel::bounded;
use simplelog::*;
use clap::Parser;

#[derive(Parser, Clone, Debug)]
#[command(name = "Pedalboard Server", version = "1.0")]
struct ServerArguments {
    #[arg(short, long, default_value=constants::DEFAULT_FRAMES_PER_PERIOD)]
    frames_per_period: usize,
    #[arg(short, long, default_value=constants::DEFAULT_RING_BUFFER_LATENCY_MS, help="Latency in milliseconds for the internal ring buffer")]
    buffer_latency: f32,
    #[arg(long, default_value="3", help="Number of periods per buffer (JACK)")]
    periods_per_buffer: usize,
    #[arg(long, default_value="40")]
    tuner_min_freq: usize,
    #[arg(long, default_value="1300")]
    tuner_max_freq: usize,
    #[arg(long, default_value="5", help="Number of periods of the minimum frequency to process for pitch")]
    tuner_periods: usize,
    #[arg(short, long)]
    input_device: Option<String>,
    #[arg(short, long)]
    output_device: Option<String>,
}

fn main() {
    CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Debug, Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
            WriteLogger::new(LevelFilter::Info, Config::default(), std::fs::File::create("pedalboard-server.log").expect("Failed to create log file")),
        ]
    ).expect("Failed to start logging");
    log::info!("Started logging...");
    log::info!("Parsing command line arguments...");
    let args = ServerArguments::parse();

    let (_host, input, output) = setup(
        args.input_device.as_ref().map(|s| s.as_str()),
        args.output_device.as_ref().map(|s| s.as_str()),
        &args
    );

    let (socket_command_sender, audio_command_receiver) = bounded(12);
    let (audio_command_sender, socket_command_receiver) = bounded(12);

    let (in_stream, out_stream) = audio_io::create_linked_streams(
        input,
        output,
        args.buffer_latency,
        args.frames_per_period,
        audio_command_receiver,
        audio_command_sender,
        args
    );

    in_stream.play().expect("Failed to play input stream");
    out_stream.play().expect("Failed to play output stream");

    after_setup();

    // Will loop infinitely (unless panic)
    socket::ServerSocket::new(29475, socket_command_sender, socket_command_receiver).start().expect("Failed to start server");
}
