use std::collections::HashMap;
use midir::{MidiInput, MidiInputConnection, MidiInputPorts};
use serde::{Serialize, Deserialize};
use eframe::egui;
use crossbeam::channel::{Receiver, Sender};

pub struct MidiState {
    settings: MidiSettings,
    input_connections: Vec<(String, MidiInputConnection<String>)>,
    available_input_ports: MidiInputPorts,
    receiver: Receiver<(u64, [u8; 16], String)>,
    sender: Sender<(u64, [u8; 16], String)>
}

impl MidiState {
    fn create_midi_input() -> MidiInput {
        MidiInput::new("Pedalboard MIDI Input").expect("Failed to create MIDI input")
    }

    pub fn new(settings: MidiSettings) -> Self {
        let available_input_ports = Self::create_midi_input().ports();
        let (sender, receiver) = crossbeam::channel::unbounded();

        Self {
            settings,
            available_input_ports,
            input_connections: Vec::new(),
            receiver,
            sender,
        }
    }

    pub fn connect_to_port(&mut self, id: &str) {
        if let Some(port) = self.available_input_ports.iter().find(|p| p.id() == id) {
            if !self.input_connections.iter().any(|(name, _c) | name == id) {
                let sender = self.sender.clone();
                let midi_input = Self::create_midi_input();
                match midi_input.connect(
                    port,
                    "Pedalboard MIDI Input Port",
                    move |time, message, data| {
                        let mut message_buf = [0; 16];
                        message_buf[..message.len()].copy_from_slice(message);
                        sender.send((time, message_buf, data.clone())).unwrap();
                    },
                    id.to_string()
                ) {
                    Ok(connection) => {
                        self.input_connections.push((id.to_string(), connection));
                        log::info!("Connected to MIDI port: {}", id);
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

    pub fn refresh_available_ports(&mut self) {
        self.available_input_ports = Self::create_midi_input().ports();
    }


}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiSettings {
    pub devices: HashMap<MidiPortSettings, Vec<MidiDevice>>
}

#[derive(Debug, Clone, Serialize, Deserialize, Hash, PartialEq, Eq)]
pub struct MidiPortSettings {
    pub name: String,
    pub auto_connect: bool
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiDevice {
    pub name: String,
    pub device_type: MidiDeviceType,
    pub midi_channel: u8,
    pub midi_cc: u8,
    pub current_value: f32
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

    pub fn midi_value_to_float(&self, mut current_value: f32, midi_value: u8) -> f32 {
        match self {
            MidiDeviceType::RelativeEncoder { sensitivity, increment_value, decrement_value } => {
                if midi_value == *increment_value {
                    current_value += *sensitivity;
                } else if midi_value == *decrement_value {
                    current_value -= *sensitivity;
                }
                current_value.clamp(0.0, 1.0)
            }
            MidiDeviceType::AbsoluteEncoder { min_value, max_value } => {
                let range = *max_value as f32 - *min_value as f32;
                (midi_value as f32 - *min_value as f32) / range
            }
            MidiDeviceType::LatchingFootswitch { on_value } => {
                if midi_value == *on_value {
                    1.0
                } else {
                    0.0
                }
            },
            MidiDeviceType::MomentaryFootswitch {
                on_value,
                use_as_latching
            } => {
                if *use_as_latching {
                    if midi_value == *on_value {
                        if current_value == 0.0 {
                            1.0
                        } else {
                            0.0
                        }
                    } else {
                        current_value
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
