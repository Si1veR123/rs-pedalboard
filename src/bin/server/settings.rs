use std::str::FromStr;

use clap::Parser;
use rs_pedalboard::server_settings::{ServerSettingsSave, SupportedHost};

#[cfg(target_os = "linux")]
mod constants {
    pub const DEFAULT_FRAMES_PER_PERIOD: usize = 256;
    pub const DEFAULT_RING_BUFFER_LATENCY_MS: f32 = 5.0;
    pub const DEFAULT_HOST: &'static str = "Jack";
    pub const HOST_HELP_STR: &'static str = "Audio host to use (JACK (default) or ALSA)";
}
#[cfg(target_os = "windows")]
mod constants {
    pub const DEFAULT_FRAMES_PER_PERIOD: usize = 512;
    pub const DEFAULT_RING_BUFFER_LATENCY_MS: f32 = 7.5;
    pub const HOST_HELP_STR: &'static str = "Audio host to use (WASAPI (default) or ASIO)";
}

#[derive(Parser, Clone, Debug)]
#[command(name = "Pedalboard Server")]
pub struct ServerArguments {
    #[arg(short, long, help=constants::HOST_HELP_STR)]
    pub host: Option<String>,
    #[arg(short, long, help="Number of frames (samples) processed at a time")]
    pub frames_per_period: Option<usize>,
    #[arg(short, long, help="Latency in milliseconds for the internal buffer")]
    pub buffer_latency: Option<f32>,
    #[arg(long, help="Number of periods per buffer (JACK) (default: 3)")]
    pub periods_per_buffer: Option<usize>,
    #[arg(long, help="Minimum frequency for the tuner (default: 40)")]
    pub tuner_min_freq: Option<usize>,
    #[arg(long, help="Maximum frequency for the tuner (default: 1300)")]
    pub tuner_max_freq: Option<usize>,
    #[arg(long, help="Number of periods of the minimum frequency to process for pitch (default: 5)")]
    pub tuner_periods: Option<usize>,
    #[arg(short, long)]
    pub input_device: Option<String>,
    #[arg(short, long)]
    pub output_device: Option<String>,
    #[arg(long, default_value_t=false, help="Ignore saved settings - use command line arguments/default")]
    pub ignore_save: bool
}

/// All server settings, compiled from args, save file and default values.
#[derive(Clone, Debug)]
pub struct ServerSettings {
    pub host: SupportedHost,
    pub frames_per_period: usize,
    pub buffer_latency: f32,
    #[allow(dead_code)]
    pub periods_per_buffer: usize,
    pub tuner_min_freq: usize,
    pub tuner_max_freq: usize,
    pub tuner_periods: usize,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
}

impl ServerSettings {
    pub fn new(args: ServerArguments, mut saved: Option<ServerSettingsSave>) -> Self {
        if args.ignore_save {
            saved = None;
        }

        let host = match args.host {
            Some(host_str) => SupportedHost::from_str(&host_str).unwrap_or_else(|e| {
                panic!("{}", e);
            }),
            None => saved
                .as_ref()
                .map_or_else(
                    || SupportedHost::default(),
                    |s| s.host.clone()
                )
        };

        let frames_per_period = args.frames_per_period.unwrap_or_else(|| {
            saved.as_ref().map_or_else(
                || constants::DEFAULT_FRAMES_PER_PERIOD,
                |s| s.buffer_size_samples()
            )
        });

        let buffer_latency = args.buffer_latency.unwrap_or_else(|| {
            saved.as_ref().map_or_else(
                || constants::DEFAULT_RING_BUFFER_LATENCY_MS,
                |s| s.latency
            )
        });

        let periods_per_buffer = args.periods_per_buffer.unwrap_or_else(|| {
            saved.as_ref().map_or_else(
                || 3,
                |s| s.periods_per_buffer
            )
        });

        let tuner_min_freq = args.tuner_min_freq.unwrap_or(40);
        let tuner_max_freq = args.tuner_max_freq.unwrap_or(1300);
        let tuner_periods = args.tuner_periods.unwrap_or_else(|| {
            saved.as_ref().map_or_else(
                || 5,
                |s| s.tuner_periods
            )
        });
        let input_device = args.input_device.or_else(|| {
            saved.as_ref().and_then(|s| s.input_device.clone())
        });
        let output_device = args.output_device.or_else(|| {
            saved.as_ref().and_then(|s| s.output_device.clone())
        });

        ServerSettings {
            host,
            frames_per_period,
            buffer_latency,
            periods_per_buffer,
            tuner_min_freq,
            tuner_max_freq,
            tuner_periods,
            input_device,
            output_device,
        }
    }
}

impl From<ServerSettings> for ServerSettingsSave {
    fn from(value: ServerSettings) -> Self {
        Self {
            host: value.host,
            buffer_size: value.frames_per_period,
            latency: value.buffer_latency,
            periods_per_buffer: value.periods_per_buffer,
            tuner_periods: value.tuner_periods,
            input_device: value.input_device,
            output_device: value.output_device
        }
    }
}
