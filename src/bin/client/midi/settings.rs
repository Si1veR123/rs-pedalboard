#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MidiSettings {
    // Port ID, Settings
    #[serde(default)]
    pub port_settings: HashMap<String, MidiPortSettings>,
    #[serde(skip)]
    pub in_looper_mode: bool
}

impl MidiSettings {
    pub(super) fn device_settings_mut<'a>(
        &'a mut self,
        port_id: &str,
        cc: u8,
        channel: u8,
        ctx: &egui::Context,
    ) -> Option<&'a mut MidiDevice> {
        if let Some(settings) = self.port_settings.get_mut(port_id) {
            Some(settings.devices.entry((cc, channel)).or_insert_with(|| {
                MidiState::invalidate_device_name_cache(ctx);
                MidiDevice::default()
            }))
        } else {
            None
        }
    }

    /// The index of the device at `(port_id, cc, channel)` among the devices which have no
    /// functions of their own and therefore fall back to a sensible parameter mapping.
    ///
    /// Devices are numbered per [`MidiDeviceKind`] and ordered by `(port_id, cc, channel)` so
    /// that a device keeps its index across runs (the underlying maps are unordered) and when
    /// its configured settings change. The first device of a kind has index 0.
    pub(super) fn get_parameter_device_index(
        &self,
        port_id: &str,
        cc: u8,
        channel: u8,
        active_pedalboard_id: u32,
    ) -> Option<usize> {
        let target_kind = {
            let target = self
                .port_settings
                .get(port_id)?
                .devices
                .get(&(cc, channel))?;

            if target.function_mode(active_pedalboard_id) != DeviceFunctionMode::Sensible {
                return None;
            }

            target.device_type.kind()
        };

        let mut device_keys: Vec<(u32, &str, (u8, u8))> = self
            .port_settings
            .iter()
            .flat_map(|(port_id, port_settings)| {
                port_settings
                    .devices
                    .iter()
                    .map(move |(cc_channel, device)| (device.get_sensible_parameter_priority(), port_id.as_str(), *cc_channel))
            })
            .collect();
        device_keys.sort_unstable();

        let mut index = 0;
        for (_priority, device_port_id, cc_channel) in device_keys {
            let device = &self.port_settings[device_port_id].devices[&cc_channel];

            if device_port_id == port_id && cc_channel == (cc, channel) {
                // `index` counts the sensibly mapped devices of this kind before the target,
                // which is the target's own index
                return Some(index);
            }

            if device.device_type.kind() == target_kind
                && device.function_mode(active_pedalboard_id) == DeviceFunctionMode::Sensible
            {
                index += 1;
            }
        }

        None
    }

        pub fn save(&self) -> Result<(), std::io::Error> {
        let stringified = serde_json::to_string(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let dir_path = rs_pedalboard::get_save_dir()?;

        if !dir_path.exists() {
            std::fs::create_dir_all(&dir_path)?;
        }
        let file_path = dir_path.join(MIDI_SETTINGS_SAVE_NAME);

        std::fs::write(file_path, stringified)
    }

    pub fn load_or_default() -> Self {
        let file_path = match homedir::my_home() {
            Ok(Some(home)) => home.join(SAVE_DIR).join(MIDI_SETTINGS_SAVE_NAME),
            Ok(None) => {
                tracing::error!("Could not determine home directory, using default MIDI settings");
                return Default::default();
            }
            Err(e) => {
                tracing::error!("Failed to get home directory: {e}, using default MIDI settings");
                return Default::default();
            }
        };

        if !file_path.exists() {
            tracing::info!(
                "MIDI Settings save file not found at {:?}, using default",
                file_path
            );
            return Default::default();
        }

        let stringified = match std::fs::read_to_string(&file_path) {
            Ok(s) => s,
            Err(e) => {
                tracing::error!(
                    "Failed to read MIDI settings file {:?}: {e}, using default",
                    file_path
                );
                return Default::default();
            }
        };

        match serde_json::from_str(&stringified) {
            Ok(state) => state,
            Err(e) => {
                tracing::error!(
                    "Failed to deserialize MIDI settings from {:?}: {e}, using default",
                    file_path
                );
                Default::default()
            }
        }
    }
}

#[derive(Debug, Clone)]
pub struct MidiPortSettings {
    // (cc, channel)
    pub devices: HashMap<(u8, u8), MidiDevice>,
    pub auto_connect: bool,
    /// User-facing name of the port, recorded when the port is connected. Defaults to
    /// [`UNKNOWN_PORT_NAME`] for entries saved before names were recorded.
    pub name: String,
}

impl Default for MidiPortSettings {
    fn default() -> Self {
        MidiPortSettings {
            devices: HashMap::new(),
            auto_connect: false,
            name: UNKNOWN_PORT_NAME.to_string(),
        }
    }
}

impl Serialize for MidiPortSettings {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Convert the HashMap<(cc, ch), MidiDevice> into HashMap<String, &MidiDevice>
        let converted: HashMap<String, &MidiDevice> = self
            .devices
            .iter()
            .map(|(port, inner)| {
                let key = format!("{}:{}", port.0, port.1);
                (key, inner)
            })
            .collect();

        let mut struct_serializer = serializer.serialize_struct("Port", 3)?;
        struct_serializer.serialize_field("devices", &converted)?;
        struct_serializer.serialize_field("auto_connect", &self.auto_connect)?;
        struct_serializer.serialize_field("name", &self.name)?;

        struct_serializer.end()
    }
}

impl<'de> Deserialize<'de> for MidiPortSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(default)]
        struct Port {
            devices: HashMap<String, MidiDevice>,
            auto_connect: bool,
            name: String,
        }

        // Missing fields fall back to the same values as MidiPortSettings::default(), so a port
        // saved by an older version of the client can still be loaded.
        impl Default for Port {
            fn default() -> Self {
                let defaults = MidiPortSettings::default();
                Port {
                    devices: HashMap::new(),
                    auto_connect: defaults.auto_connect,
                    name: defaults.name,
                }
            }
        }

        // first deserialize into HashMap<String, HashMap<String, MidiDevice>>
        let raw: Port = Deserialize::deserialize(deserializer)?;

        let mut actual_map = HashMap::new();
        for (key, dev) in raw.devices {
            let mut parts = key.split(':');
            let cc = parts
                .next()
                .ok_or_else(|| serde::de::Error::custom("missing cc"))?
                .parse::<u8>()
                .map_err(serde::de::Error::custom)?;
            let ch = parts
                .next()
                .ok_or_else(|| serde::de::Error::custom("missing channel"))?
                .parse::<u8>()
                .map_err(serde::de::Error::custom)?;
            actual_map.insert((cc, ch), dev);
        }

        Ok(MidiPortSettings {
            devices: actual_map,
            auto_connect: raw.auto_connect,
            name: raw.name,
        })
    }
}
use eframe::egui;
use serde::{ser::SerializeStruct, Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;

use super::{
    device::{DeviceFunctionMode, MidiDevice},
    MidiState, MIDI_SETTINGS_SAVE_NAME, UNKNOWN_PORT_NAME,
};
use crate::SAVE_DIR;
