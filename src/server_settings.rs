use crate::SAVE_DIR;
use serde::{Serialize, Deserialize};
use std::path::PathBuf;

const SAVE_NAME: &str = "server_settings.json";

/// Server settings that will be saved to a file.
#[derive(Serialize, Deserialize, Clone)]
pub struct ServerSettingsSave {
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

    pub fn load() -> Result<Self, String> {
        match std::fs::read_to_string(Self::get_save_path().expect("Failed to get server settings save path")) {
            Ok(data) => serde_json::from_str(&data).map_err(|e| format!("Failed to deserialize server settings, error: {}", e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err("Settings file not found".to_string())
            },
            Err(e) => Err(format!("Failed to read server settings file, error: {}", e)),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let data = serde_json::to_string(self).map_err(|e| format!("Failed to serialize server settings, error: {}", e))?;
        std::fs::write(Self::get_save_path().expect("Failed to get server settings save path"), data).map_err(|e| format!("Failed to write server settings file, error: {}", e))?;
        Ok(())
    }

    pub fn buffer_size_samples(&self) -> usize {
        2_usize.pow(self.buffer_size as u32)
    }
}