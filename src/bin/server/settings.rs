use std::{path::PathBuf, str::FromStr};

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
    pub tuner_min_freq: Option<u32>,
    #[arg(long, help="Maximum frequency for the tuner (default: 1300)")]
    pub tuner_max_freq: Option<u32>,
    #[arg(long, help="Number of periods of the minimum frequency to process for pitch (default: 5)")]
    pub tuner_periods: Option<usize>,
    #[arg(short, long)]
    pub input_device: Option<String>,
    #[arg(short, long)]
    pub output_device: Option<String>,
    #[arg(long, help="Preferred sample rate for the audio host. Uses highest if not available. (default: 48000)")]
    pub preferred_sample_rate: Option<u32>,
    #[arg(long, help="Number of 2x upsample passes to apply before processing (default: 0)")]
    pub upsample_passes: Option<u32>,
    #[arg(long, default_value_t=false, help="Ignore saved settings - use command line arguments/default")]
    pub ignore_save: bool,
    #[arg(long, help="Directory to save recordings to (default: ~/rs_pedalboard/Recordings)")]
    pub recording_dir: Option<PathBuf>
}

/// All server settings, compiled from args, save file and default values.
#[derive(Clone, Debug)]
pub struct ServerSettings {
    pub host: SupportedHost,
    pub frames_per_period: usize,
    pub buffer_latency: f32,
    #[allow(dead_code)]
    pub periods_per_buffer: usize,
    pub tuner_min_freq: u32,
    pub tuner_max_freq: u32,
    pub tuner_periods: usize,
    pub input_device: Option<String>,
    pub output_device: Option<String>,
    pub preferred_sample_rate: Option<u32>,
    pub upsample_passes: u32,
    pub recording_dir: PathBuf
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

        let preferred_sample_rate = args.preferred_sample_rate.or_else(|| {
            saved.as_ref().and_then(|s| s.preferred_sample_rate)
        });

        let upsample_passes = args.upsample_passes.unwrap_or_else(|| {
            saved.as_ref().map_or_else(
                || 0,
                |s| s.upsample_passes
            )
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
            preferred_sample_rate,
            upsample_passes,
            recording_dir: Self::recording_dir(
                args.recording_dir,
                saved.as_ref()
                    .and_then(
                        |s|
                        s.recording_dir.as_ref().map(|p| p.as_path())
                    )
            )
        }
    }

    pub fn frames_per_period_after_upsample(&self) -> usize {
        self.frames_per_period * 2_usize.pow(self.upsample_passes)
    }

    pub fn default_recording_dir() -> Option<PathBuf> {
        let dir = homedir::my_home()
            .ok()
            .and_then(
                |p| p.and_then(|p| Some(p.join(rs_pedalboard::SAVE_DIR).join("Recordings")))
            )?;
            
        if !dir.exists() {
            std::fs::create_dir_all(&dir).ok()?;
        }
        Some(dir)
    }

    pub fn recording_dir(arg: Option<PathBuf>, saved: Option<&std::path::Path>) -> PathBuf {
        arg.or_else(|| saved.map(|s| s.to_path_buf()))
            .or_else(|| Self::default_recording_dir())
            .expect("Failed to get recordings directory")
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
            output_device: value.output_device,
            preferred_sample_rate: value.preferred_sample_rate,
            upsample_passes: value.upsample_passes,
            recording_dir: Some(value.recording_dir)
        }
    }
}
