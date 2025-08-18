use std::{collections::HashMap, sync::{Arc, Mutex, RwLock}};
use midir::{MidiInput, MidiInputConnection, MidiInputPorts};
use serde::{Serialize, Deserialize, Serializer, Deserializer, ser::SerializeStruct};
use eframe::egui;
use crossbeam::channel::{Receiver, Sender};
use strum_macros::EnumDiscriminants;

use crate::socket::ClientSocketThreadHandle;
use crate::SAVE_DIR;

pub const MIDI_SETTINGS_SAVE_NAME: &'static str = "midi_settings.json";

// Simple functions that MIDI devices can control
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ClientMidiFunction {
    Mute,
    NextPedalboard,
    PrevPedalboard,
    OpenUtilities,
    OpenStage
}

pub struct MidiState {
    settings: Arc<Mutex<MidiSettings>>,
    input_connections: Vec<(String, MidiInputConnection<String>)>,
    available_input_ports: MidiInputPorts,
    // Receive midi functions from the midi callbacks
    receiver: Receiver<ClientMidiFunction>,
    // Used to clone new senders
    sender: Sender<ClientMidiFunction>,
    egui_ctx: egui::Context
}

impl MidiState {
    pub fn save_settings(&self) -> Result<(), std::io::Error> {
        self.settings.lock().map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, "MIDI settings mutex poisoned"))?.save()
    }

    fn create_midi_input() -> MidiInput {
        MidiInput::new("Pedalboard MIDI Input").expect("Failed to create MIDI input")
    }

    fn parse_cc_message(message: &[u8]) -> Option<(u8, u8, u8)> {
        if message.len() < 3 || message[0] & 0xF0 != 0xB0 {
            return None; // Not a Control Change message
        }
        Some((message[0] & 0x0F, message[1], message[2]))
    }

    fn device_settings_mut<'a>(settings: &'a mut MidiSettings, port_name: &str, cc: u8, channel: u8) -> Option<&'a mut MidiDevice> {
        if let Some(devices) = settings.port_devices.get_mut(port_name) {
            Some(devices.entry((cc, channel)).or_insert_with(|| MidiDevice {
                name: "New Device".to_string(),
                device_type: MidiDeviceType::AbsoluteEncoder { min_value: 0, max_value: 127 },
                current_value: 0.5,
                functions: Vec::new(),
            }))
        } else {
            None
        }
    }

    fn handle_midi_message(settings: &Arc<Mutex<MidiSettings>>, port_name: &str, message: &[u8], sender: &Sender<ClientMidiFunction>, egui_ctx: &egui::Context) {
        let (channel, cc, value) = match Self::parse_cc_message(message) {
            Some((channel, cc, value)) => (channel, cc, value),
            None => return
        };

        let mut settings_lock = settings.lock().expect("MidiState: Mutex poisoned.");

        if let Some(device) = Self::device_settings_mut(&mut settings_lock, port_name, cc, channel) {
            let old_value = device.current_value;
            device.update_with_midi_value(value);
            if device.current_value != old_value {
                // Activate any MIDI functions for this device
                if device.current_value == 1.0 {
                    for function in &device.functions {
                        // If this fails the channel is dead -> the client is dead
                        let _ = sender.send(function.clone());
                    }
                    if device.functions.len() > 0 {
                        egui_ctx.request_repaint();
                    }
                }
            }
        }
    }

    pub fn new(settings: MidiSettings, egui_ctx: egui::Context) -> Self {
        let available_input_ports = Self::create_midi_input().ports();
        let (sender, receiver) = crossbeam::channel::unbounded();

        Self {
            settings: Arc::new(Mutex::new(settings)),
            available_input_ports,
            input_connections: Vec::new(),
            receiver,
            sender,
            egui_ctx
        }
    }

    pub fn connect_to_port(&mut self, id: &str) {
        if let Some(port) = self.available_input_ports.iter().find(|p| p.id() == id) {
            if !self.input_connections.iter().any(|(name, _c) | name == id) {
                let midi_input = Self::create_midi_input();
                let settings_clone = self.settings.clone();
                let sender_clone = self.sender.clone();
                let egui_ctx_clone = self.egui_ctx.clone();
                match midi_input.connect(
                    port,
                    "Pedalboard MIDI Input Port",
                    move |_time, message, data| {
                        Self::handle_midi_message(&settings_clone, data.as_str(), message, &sender_clone, &egui_ctx_clone);
                    },
                    id.to_string()
                ) {
                    Ok(connection) => {
                        self.input_connections.push((id.to_string(), connection));
                        log::info!("Connected to MIDI port: {}", id);
                        self.available_input_ports.retain(|p| p.id() != id);
                        self.settings.lock().expect("MidiState: Mutex poisoned.").port_devices.entry(id.to_string()).or_default();
                    }
                    Err(e) => {
                        log::error!("Failed to connect to MIDI port {}: {}", id, e);
                        return;
                    }
                }
            }
        } else {
            log::error!("MIDI port {} not found", id);
        }
    }

    pub fn disconnect_from_port(&mut self, id: &str) {
        self.input_connections.retain(|(name, _)| name != id);
        self.refresh_available_ports();
    }

    pub fn refresh_available_ports(&mut self) {
        self.available_input_ports = Self::create_midi_input().ports();
        self.available_input_ports.retain(|p| !self.input_connections.iter().any(|(name, _)| name == &p.id()));
    }

    /// This UI contains a list of ports that we can connect to, and a list of connected ports.
    /// Connected ports have a list of devices from MidiSettings, that can be removed, edited, etc.
    pub fn midi_port_device_settings_ui(&mut self, ui: &mut egui::Ui) {
        let row_height = 40.0;
        ui.label("Available MIDI Ports:");
        ui.separator();
        ui.button("Refresh").on_hover_text("Refresh available MIDI ports").clicked().then(|| self.refresh_available_ports());
        egui::Grid::new("midi_ports_grid")
            .striped(true)
            .min_row_height(row_height)
            .num_columns(2)
            .show(ui, |ui| {
                let mut connect = None;
                for port in &self.available_input_ports {
                    let port_name = port.id();
                    ui.label(&port_name);
                    if ui.button("Connect").clicked() {
                        connect = Some(port_name);
                    }
                    ui.end_row();
                }
                if let Some(port_name) = connect {
                    self.connect_to_port(&port_name);
                }
            });

        ui.label("Connected MIDI Ports:");
        ui.separator();
        let mut disconnect: Option<String> = None;
        egui::Grid::new("connected_midi_ports_grid")
            .num_columns(2)
            .min_row_height(row_height)
            .show(ui, |ui| {
                let mut settings_lock = self.settings.lock().expect("MidiState: Mutex poisoned.");

                for (port_name, connection) in &self.input_connections {
                    // Lighter background for port rows
                    ui.painter().rect_filled(ui.available_rect_before_wrap(), 5.0, crate::LIGHT_BACKGROUND_COLOR);
                    ui.label(port_name);
                    if ui.button("Disconnect").clicked() {
                        disconnect = Some(port_name.clone());
                    }

                    ui.end_row();
                    
                    // Show the device settings for this port
                    egui::Grid::new(format!("midi_port_{}_devices_grid", port_name))
                        .num_columns(6)
                        .striped(true)
                        .show(ui, |ui| {
                            let mut forget: Option<(u8, u8)> = None;
                            let device_settings = settings_lock.port_devices.get_mut(port_name).expect("Connected ports should always be in MidiSettings HashMap");
                            for ((cc, channel), device) in device_settings {
                                // Device row
                                ui.label(&device.name);
                                ui.label(format!("CC {}", cc));
                                ui.label(format!("Ch {}", channel));
                                ui.label(device.device_type.get_name());
                                ui.label(format!("{:.2}", device.current_value));

                                ui.collapsing("", |ui| {
                                    ui.horizontal(|ui| {
                                        if ui.button("Forget").clicked() {
                                            forget = Some((*cc, *channel));
                                        }
                                        ui.label("Rename:");
                                        ui.text_edit_singleline(&mut device.name);
                                    });

                                    // Device type selection
                                    egui::ComboBox::from_label("Device Type")
                                        .selected_text(device.device_type.get_name())
                                        .show_ui(ui, |ui| {
                                            if ui
                                                .selectable_label(
                                                    matches!(device.device_type, MidiDeviceType::RelativeEncoder { .. }),
                                                    "Relative Encoder",
                                                )
                                                .clicked()
                                            {
                                                device.device_type = MidiDeviceType::RelativeEncoder {
                                                    sensitivity: 0.1,
                                                    increment_value: 0,
                                                    decrement_value: 127,
                                                };
                                            }
                                            if ui
                                                .selectable_label(
                                                    matches!(device.device_type, MidiDeviceType::AbsoluteEncoder { .. }),
                                                    "Absolute Encoder",
                                                )
                                                .clicked()
                                            {
                                                device.device_type = MidiDeviceType::AbsoluteEncoder {
                                                    min_value: 0,
                                                    max_value: 127,
                                                };
                                            }
                                            if ui
                                                .selectable_label(
                                                    matches!(device.device_type, MidiDeviceType::LatchingFootswitch { .. }),
                                                    "Latching Footswitch",
                                                )
                                                .clicked()
                                            {
                                                device.device_type = MidiDeviceType::LatchingFootswitch {
                                                    on_value: 127,
                                                };
                                            }
                                            if ui
                                                .selectable_label(
                                                    matches!(device.device_type, MidiDeviceType::MomentaryFootswitch { .. }),
                                                    "Momentary Footswitch",
                                                )
                                                .clicked()
                                            {
                                                device.device_type = MidiDeviceType::MomentaryFootswitch {
                                                    on_value: 127,
                                                    use_as_latching: false,
                                                };
                                            }
                                        });

                                    // Variant-specific settings
                                    device.device_type.settings_ui(ui);
                                });

                                ui.end_row();
                            }

                            if let Some((cc, channel)) = forget {
                                if let Some(port_devices) = settings_lock.port_devices.get_mut(port_name) {
                                    port_devices.remove(&(cc, channel));
                                }
                            }
                        });
                }
            });
        
        if let Some(port_name) = disconnect {
            self.disconnect_from_port(&port_name);
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct MidiSettings {
    //                       port name       CC, channel
    pub port_devices: HashMap<String, HashMap<(u8, u8), MidiDevice>>
}

impl Serialize for MidiSettings {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Convert the inner HashMap<(cc, ch), MidiDevice> into HashMap<String, &MidiDevice>
        let converted: HashMap<&String, HashMap<String, &MidiDevice>> = self
            .port_devices
            .iter()
            .map(|(port, inner)| {
                let inner_map: HashMap<String, &MidiDevice> = inner
                    .iter()
                    .map(|(&(cc, ch), dev)| (format!("{cc}:{ch}"), dev))
                    .collect();
                (port, inner_map)
            })
            .collect();

        // Serialize the HashMap directly, no wrapper field
        converted.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MidiSettings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // first deserialize into HashMap<String, HashMap<String, MidiDevice>>
        let raw: HashMap<String, HashMap<String, MidiDevice>> =
            Deserialize::deserialize(deserializer)?;

        let mut port_devices = HashMap::new();
        for (port, inner) in raw {
            let mut inner_map = HashMap::new();
            for (key, dev) in inner {
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
                inner_map.insert((cc, ch), dev);
            }
            port_devices.insert(port, inner_map);
        }

        Ok(MidiSettings { port_devices })
    }
}

impl MidiSettings {
    pub fn save(&self) -> Result<(), std::io::Error> {
        let stringified = serde_json::to_string(self).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let dir_path = homedir::my_home().map_err(
            |e| std::io::Error::new(std::io::ErrorKind::Other, e)
        )?.unwrap().join(SAVE_DIR);

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
                log::error!("Could not determine home directory, using default MIDI settings");
                return Default::default();
            }
            Err(e) => {
                log::error!("Failed to get home directory: {e}, using default MIDI settings");
                return Default::default();
            }
        };

        if !file_path.exists() {
            log::info!("MIDI Settings save file not found at {:?}, using default", file_path);
            return Default::default();
        }

        let stringified = match std::fs::read_to_string(&file_path) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to read MIDI settings file {:?}: {e}, using default", file_path);
                return Default::default();
            }
        };

        match serde_json::from_str(&stringified) {
            Ok(state) => state,
            Err(e) => {
                log::error!("Failed to deserialize MIDI settings from {:?}: {e}, using default", file_path);
                Default::default()
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiDevice {
    pub name: String,
    pub device_type: MidiDeviceType,
    pub current_value: f32,
    pub functions: Vec<ClientMidiFunction>,
}

impl MidiDevice {
    pub fn update_with_midi_value(&mut self, midi_value: u8) {
        match &self.device_type {
            MidiDeviceType::RelativeEncoder { sensitivity, increment_value, decrement_value } => {
                if midi_value == *increment_value {
                    self.current_value += *sensitivity;
                } else if midi_value == *decrement_value {
                    self.current_value -= *sensitivity;
                }
                self.current_value = self.current_value.clamp(0.0, 1.0);
            }
            MidiDeviceType::AbsoluteEncoder { min_value, max_value } => {
                let range = *max_value as f32 - *min_value as f32;
                self.current_value = (midi_value as f32 - *min_value as f32) / range;
            }
            MidiDeviceType::LatchingFootswitch { on_value } => {
                self.current_value = if midi_value == *on_value {
                    1.0
                } else {
                    0.0
                };
            },
            MidiDeviceType::MomentaryFootswitch {
                on_value,
                use_as_latching
            } => {
                self.current_value = if *use_as_latching {
                    if midi_value == *on_value {
                        if self.current_value == 0.0 {
                            1.0
                        } else {
                            0.0
                        }
                    } else {
                        self.current_value
                    }
                } else {
                    if midi_value == *on_value {
                        1.0
                    } else {
                        0.0
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MidiDeviceType {
    RelativeEncoder {
        sensitivity: f32,
        increment_value: u8,
        decrement_value: u8,
    },
    AbsoluteEncoder {
        min_value: u8,
        max_value: u8
    },
    LatchingFootswitch {
        on_value: u8
    },
    MomentaryFootswitch {
        on_value: u8,
        use_as_latching: bool
    }
}

impl MidiDeviceType {
    pub fn get_name(&self) -> &'static str {
        match self {
            MidiDeviceType::RelativeEncoder { .. } => "Relative Encoder",
            MidiDeviceType::AbsoluteEncoder { .. } => "Absolute Encoder",
            MidiDeviceType::LatchingFootswitch { .. } => "Latching Footswitch",
            MidiDeviceType::MomentaryFootswitch { .. } => "Momentary Footswitch",
        }
    }

    pub fn settings_ui(&mut self, ui: &mut egui::Ui) {
        match self {
            MidiDeviceType::RelativeEncoder { sensitivity, increment_value, decrement_value } => {
                ui.add(egui::Slider::new(sensitivity, 0.01..=1.0).text("Sensitivity"));
                ui.add(egui::Slider::new(increment_value, 0..=127).text("Increment Value"));
                ui.add(egui::Slider::new(decrement_value, 0..=127).text("Decrement Value"));
            }
            MidiDeviceType::AbsoluteEncoder { min_value, max_value } => {
                ui.add(egui::Slider::new(min_value, 0..=127).text("Min Value"));
                ui.add(egui::Slider::new(max_value, 0..=127).text("Max Value"));
            }
            MidiDeviceType::LatchingFootswitch { on_value } => {
                ui.add(egui::Slider::new(on_value, 0..=127).text("On Value"));
            }
            MidiDeviceType::MomentaryFootswitch { on_value, use_as_latching } => {
                ui.add(egui::Slider::new(on_value, 0..=127).text("On Value"));
                ui.checkbox(use_as_latching, "Use as Latching");
            }
        }
    }
}
