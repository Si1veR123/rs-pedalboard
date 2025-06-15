#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::{setup, after_setup};

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows::{setup, after_setup};

#[cfg(target_os = "linux")]
pub mod constants {
    pub const FRAMES_PER_PERIOD: usize = 256;
    pub const PERIODS_PER_BUFFER: usize = 3;
    pub const RING_BUFFER_LATENCY_MS: f32 = 5.0;
}
#[cfg(target_os = "windows")]
pub mod constants {
    pub const FRAMES_PER_PERIOD: usize = 512;
    pub const RING_BUFFER_LATENCY_MS: f32 = 10.0;
}

mod audio_io;
mod socket;
mod device_select;

use cpal::traits::StreamTrait;
use rs_pedalboard::pedalboard_set::PedalboardSet;
use crossbeam::channel::bounded;

use simplelog::*;

fn main() {
    CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Debug, Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
            WriteLogger::new(LevelFilter::Info, Config::default(), std::fs::File::create("pedalboard-server.log").expect("Failed to create log file")),
        ]
    ).expect("Failed to start logging");
    log::info!("Started logging...");

    let (_host, input, output) = setup();

    let (socket_command_sender, audio_command_receiver) = bounded(12);
    let (audio_command_sender, socket_command_receiver) = bounded(12);

    let (in_stream, out_stream) = audio_io::create_linked_streams(
        input,
        output,
        constants::RING_BUFFER_LATENCY_MS,
        constants::FRAMES_PER_PERIOD,
        audio_command_receiver,
        audio_command_sender
    );

    in_stream.play().expect("Failed to play input stream");
    out_stream.play().expect("Failed to play output stream");

    after_setup();

    // Will loop infinitely (unless panic)
    socket::ServerSocket::new(29475, socket_command_sender, socket_command_receiver).start().expect("Failed to start server");
}
