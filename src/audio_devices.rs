/// Get a list of audio input and output devices available on the system.

#[cfg(target_os = "linux")]
use std::fs::File;
#[cfg(target_os = "linux")]
use std::io::Read;
#[cfg(target_os = "linux")]
use regex::Regex;

use std::error::Error;
use std::fmt::Display;
use cpal::{DeviceNameError, DevicesError, Host};
use cpal::traits::{HostTrait, DeviceTrait};

#[derive(Debug)]
pub enum AudioDeviceError {
    HostRequired,
    DevicesError(DevicesError),
    DeviceNameError(DeviceNameError),
    IOError(std::io::Error),
}

impl Display for AudioDeviceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AudioDeviceError::HostRequired => write!(f, "Host is required to get audio devices on this platform"),
            AudioDeviceError::DevicesError(e) => write!(f, "Failed to get devices: {}", e),
            AudioDeviceError::DeviceNameError(e) => write!(f, "Failed to get device name: {}", e),
            AudioDeviceError::IOError(e) => write!(f, "Audio Device IO error: {}", e),
        }
    }
}

impl Error for AudioDeviceError {}

#[cfg(target_os = "windows")]
pub fn get_input_devices(host: Option<&Host>) -> Result<Vec<String>, AudioDeviceError> {
    if let Some(host) = host {
        let input_devices: Result<Vec<String>, AudioDeviceError> = host.input_devices().map_err(AudioDeviceError::DevicesError)?
            .map(|d| d.name().map_err(AudioDeviceError::DeviceNameError))
            .collect();
        Ok(input_devices?)
    } else {
        Err(AudioDeviceError::HostRequired)
    }
}

#[cfg(target_os = "windows")]
pub fn get_output_devices(host: Option<&Host>) -> Result<Vec<String>, AudioDeviceError> {
    if let Some(host) = host {
        let output_devices: Result<Vec<String>, AudioDeviceError> = host.output_devices().map_err(AudioDeviceError::DevicesError)?
            .map(|d| d.name().map_err(AudioDeviceError::DeviceNameError))
            .collect();
        Ok(output_devices?)
    } else {
        Err(AudioDeviceError::HostRequired)
    }
}

#[cfg(target_os = "linux")]
fn get_hw_devices() -> Result<Vec<String>, AudioDeviceError> {
    let mut sound_cards = String::new();
    File::open("/proc/asound/cards")
        .map_err(AudioDeviceError::IOError)?
        .read_to_string(&mut sound_cards)
        .map_err(AudioDeviceError::IOError)?;

    let re = Regex::new(r"\[(.*)\]").unwrap();

    Ok(re.captures_iter(&sound_cards).map(|c| {
        let (_full, [name]) = c.extract();
        format!("hw:{}", name.trim())
    }).collect())
}

// On linux, we list all input and output devices for both input and output
#[cfg(target_os = "linux")]
pub fn get_input_devices(_host: Option<Host>) -> Result<Vec<String>> {
    get_hw_devices()
}

#[cfg(target_os = "linux")]
pub fn get_output_devices(_host: Option<Host>) -> Result<Vec<String>> {
    get_hw_devices()
}
