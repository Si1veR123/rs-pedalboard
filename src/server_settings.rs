use crate::{audio_devices::{get_host, get_input_devices, get_output_devices}, SAVE_DIR};
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
        //let host = get_host();
//
        //let (input_device, output_device) = match host {
        //    Some(host) => {
        //        let input_devices = get_input_devices(Some(&host));
        //        let output_devices = get_output_devices(Some(&host));
//
        //        let input_device = match input_devices {
        //            Ok(devices) => {
        //                if devices.is_empty() {
        //                    log::warn!("No input devices found");
        //                    None
        //                } else {
        //                    Some(devices[0].clone())
        //                }
        //            },
        //            Err(e) => {
        //                log::warn!("Failed to get input devices: {}", e);
        //                None
        //            }
        //        };
//
        //        let output_device = match output_devices {
        //            Ok(devices) => {
        //                if devices.is_empty() {
        //                    log::warn!("No output devices found");
        //                    None
        //                } else {
        //                    Some(devices[0].clone())
        //                }
        //            },
        //            Err(e) => {
        //                log::warn!("Failed to get output devices: {}", e);
        //                None
        //            }
        //        };
//
        //        (input_device, output_device)
        //    },
        //    None => {
        //        log::warn!("No audio host found");
        //        (None, None)
        //    }
        //};

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
        match std::fs::read_to_string(Self::get_save_path().expect("Failed to get settings save path")) {
            Ok(data) => serde_json::from_str(&data).map_err(|e| format!("Failed to deserialize settings, error: {}", e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err("Settings file not found".to_string())
            },
            Err(e) => Err(format!("Failed to read settings file, error: {}", e)),
        }
    }

    pub fn save(&self) -> Result<(), String> {
        let data = serde_json::to_string(self).map_err(|e| format!("Failed to serialize settings, error: {}", e))?;
        std::fs::write(Self::get_save_path().expect("Failed to get settings save path"), data).map_err(|e| format!("Failed to write settings file, error: {}", e))?;
        Ok(())
    }

    pub fn buffer_size_samples(&self) -> usize {
        2_usize.pow(self.buffer_size as u32)
    }
}