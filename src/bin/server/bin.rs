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
    pub const FRAMES_PER_PERIOD: usize = 1024;
    pub const RING_BUFFER_LATENCY_MS: f32 = 20.0;
}

mod audio_io;
mod socket;
mod device_select;

use cpal::traits::StreamTrait;
use rs_pedalboard::{pedalboard::Pedalboard, pedalboard_set::PedalboardSet, pedals::{self, Pedal}};
use crossbeam::channel::bounded;

use simplelog::*;

fn main() {
    CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
            WriteLogger::new(LevelFilter::Info, Config::default(), std::fs::File::create("pedalboard-server.log").expect("Failed to create log file")),
        ]
    ).expect("Failed to start logging");
    log::info!("Started logging...");

    let (_host, input, output) = setup();

    let delay = pedals::Delay::new();
    let fuzz = pedals::Fuzz::new();

    let pedalboard = Pedalboard::from_pedals(vec![Pedal::Delay(delay), Pedal::Fuzz(fuzz)]);
    let pedalboard_set = PedalboardSet::from_pedalboards(vec![pedalboard]);

    let (command_sender, command_receiver) = bounded(12);

    let (in_stream, out_stream) = audio_io::create_linked_streams(
        input,
        output,
        pedalboard_set,
        constants::RING_BUFFER_LATENCY_MS,
        constants::FRAMES_PER_PERIOD,
        command_receiver
    );

    in_stream.play().expect("Failed to play input stream");
    out_stream.play().expect("Failed to play output stream");

    after_setup();

    // Will loop infinitely (unless panic)
    socket::ServerSocket::new(29475, command_sender).start().expect("Failed to start server");
}
