use crate::pedals::parameters::{ParameterUpdate, PedalParameterRange};
use crate::SAVE_DIR;
use serde::{Deserialize, Serialize};
use std::{fmt::Display, path::PathBuf, str::FromStr};
use strum_macros::EnumIter;

const SAVE_NAME: &str = "processor_settings.json";

#[cfg(target_os = "linux")]
#[derive(Serialize, Deserialize, Clone, Copy, Default, Debug, EnumIter, PartialEq)]
pub enum SupportedHost {
    #[default]
    JACK,
}

#[cfg(target_os = "windows")]
#[derive(Serialize, Deserialize, Clone, Copy, Default, Debug, EnumIter, PartialEq)]
pub enum SupportedHost {
    #[default]
    WASAPI,
    #[cfg(feature = "asio")]
    ASIO,
}

impl Display for SupportedHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Just use debug implementation
        write!(f, "{:?}", self)
    }
}

impl FromStr for SupportedHost {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        #[cfg(target_os = "linux")]
        match s.to_lowercase().as_str() {
            "jack" => Ok(SupportedHost::JACK),
            _ => Err(format!("Unsupported host: {}", s)),
        }

        #[cfg(target_os = "windows")]
        match s.to_lowercase().as_str() {
            "wasapi" => Ok(SupportedHost::WASAPI),
            #[cfg(feature = "asio")]
            "asio" => Ok(SupportedHost::ASIO),
            _ => Err(format!("Unsupported host: {}", s)),
        }
    }
}

impl From<SupportedHost> for cpal::HostId {
    fn from(value: SupportedHost) -> Self {
        #[cfg(target_os = "linux")]
        match value {
            SupportedHost::JACK => cpal::HostId::Jack,
        }

        #[cfg(target_os = "windows")]
        match value {
            SupportedHost::WASAPI => cpal::HostId::Wasapi,
            #[cfg(feature = "asio")]
            SupportedHost::ASIO => cpal::HostId::Asio,
        }
    }
}

/// Processor settings that will be saved to a file.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ProcessorSettingsSave {
    pub host: SupportedHost,
    // Buffer size is this value ^2
    pub buffer_size: usize,
    pub latency: f32,
    // Only used for JACK (linux)
    pub periods_per_buffer: usize,
    pub tuner_periods: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_device: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preferred_sample_rate: Option<u32>,
    pub upsample_passes: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recording_dir: Option<PathBuf>,
}

impl Default for ProcessorSettingsSave {
    fn default() -> Self {
        Self {
            host: SupportedHost::default(),
            buffer_size: f32::log2(256.0) as usize,
            latency: 5.0,
            periods_per_buffer: 3,
            tuner_periods: 5,
            input_device: None,
            output_device: None,
            preferred_sample_rate: None,
            upsample_passes: 0,
            recording_dir: None,
        }
    }
}

impl ProcessorSettingsSave {
    fn get_save_path() -> Option<PathBuf> {
        Some(homedir::my_home().ok()??.join(SAVE_DIR).join(SAVE_NAME))
    }

    pub fn load_or_default() -> Self {
        let save_path = match Self::get_save_path() {
            Some(path) => path,
            None => {
                tracing::error!("Failed to get processor settings save path, using default");
                return Default::default();
            }
        };

        if !save_path.exists() {
            tracing::info!("Processor settings save file not found, using default");
            return Default::default();
        }

        let data = match std::fs::read_to_string(&save_path) {
            Ok(d) => d,
            Err(e) => {
                tracing::error!(
                    "Failed to read processor settings from {:?}: {e}, using default",
                    save_path
                );
                return Default::default();
            }
        };

        match serde_json::from_str(&data) {
            Ok(settings) => settings,
            Err(e) => {
                tracing::error!(
                    "Failed to deserialize processor settings from {:?}: {e}, using default",
                    save_path
                );
                Default::default()
            }
        }
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let data = serde_json::to_string(self).expect("Failed to serialize processor settings");
        std::fs::write(
            Self::get_save_path().expect("Failed to get processor settings save path"),
            data,
        )?;
        Ok(())
    }

    pub fn buffer_size_samples(&self) -> usize {
        2_usize.pow(self.buffer_size as u32)
    }
}

/// Represents a change to a float setting such as master volume or active parameter.
/// Clamped between 0 and 1.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FloatSettingUpdate {
    Relative(f32),
    Absolute(f32),
    FlipFlop,
}

impl FloatSettingUpdate {
    pub fn apply(&self, current_value: f32, setting_range: Option<(f32, f32)>) -> f32 {
        let (min, max) = setting_range.unwrap_or((0.0, 1.0));
        let range = max - min;
        let current_value_fraction = if range == 0.0 {
            0.0
        } else {
            (current_value - min) / range
        };
        let new_value = match self {
            FloatSettingUpdate::Relative(delta) => {
                (current_value_fraction + delta).clamp(0.0, 1.0) * range + min
            }
            FloatSettingUpdate::Absolute(value) => value.clamp(0.0, 1.0) * range + min,
            FloatSettingUpdate::FlipFlop => {
                if current_value_fraction < 0.5 {
                    max
                } else {
                    min
                }
            }
        };
        new_value.clamp(min, max)
    }

    /// Converts this fraction based update into a typed parameter update for the given range.
    pub fn to_parameter_update(&self, range: &PedalParameterRange) -> ParameterUpdate {
        match self {
            FloatSettingUpdate::Relative(delta) => {
                ParameterUpdate::Relative(*delta, Some(range.clone()))
            }
            FloatSettingUpdate::Absolute(value) => {
                ParameterUpdate::Absolute(range.parameter_from_interp(*value))
            }
            FloatSettingUpdate::FlipFlop => ParameterUpdate::FlipFlop(Some(range.clone())),
        }
    }
}
