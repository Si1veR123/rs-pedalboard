use crate::SAVE_DIR;
use serde::{Serialize, Deserialize};
use std::{fmt::Display, path::PathBuf, str::FromStr};
use strum_macros::EnumIter;

const SAVE_NAME: &str = "server_settings.json";

#[cfg(target_os = "linux")]
#[derive(Serialize, Deserialize, Clone, Copy, Default, Debug, EnumIter, PartialEq)]
pub enum SupportedHost {
    #[default]
    JACK
}

#[cfg(target_os = "windows")]
#[derive(Serialize, Deserialize, Clone, Copy, Default, Debug, EnumIter, PartialEq)]
pub enum SupportedHost {
    #[default]
    WASAPI,
    ASIO
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
            SupportedHost::ASIO => cpal::HostId::Asio,
        }
    }
}

/// Server settings that will be saved to a file.
#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct ServerSettingsSave {
    pub host: SupportedHost,
    // Buffer size is this value ^2
    pub buffer_size: usize,
    pub latency: f32,
    // Only used for JACK (linux)
    pub periods_per_buffer: usize,
    pub tuner_periods: usize,
    pub input_device: Option<String>,
    pub output_device: Option<String>
}

impl Default for ServerSettingsSave {
    fn default() -> Self {
        Self {
            host: SupportedHost::default(),
            buffer_size: f32::log2(256.0) as usize,
            latency: 5.0,
            periods_per_buffer: 3,
            tuner_periods: 5,
            input_device: None,
            output_device: None
        }
    }
}

impl ServerSettingsSave {
    fn get_save_path() -> Option<PathBuf> {
        Some(homedir::my_home().ok()??.join(SAVE_DIR).join(SAVE_NAME))
    }

    pub fn load_or_default() -> Result<Self, std::io::Error> {
        let save_path = Self::get_save_path().expect("Failed to get client settings save path");

        if !save_path.exists() {
            log::info!("Server settings save file not found, using default");
            return Ok(Self::default());
        }

        let data = std::fs::read_to_string(save_path)?;

        Ok(serde_json::from_str(&data).expect("Failed to deserialize server settings"))
    }

    pub fn save(&self) -> Result<(), std::io::Error> {
        let data = serde_json::to_string(self).expect("Failed to serialize server settings");
        std::fs::write(Self::get_save_path().expect("Failed to get server settings save path"), data)?;
        Ok(())
    }

    pub fn buffer_size_samples(&self) -> usize {
        2_usize.pow(self.buffer_size as u32)
    }
}