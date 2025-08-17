use std::{collections::HashMap, sync::{Arc, Mutex, RwLock}};
use midir::{MidiInput, MidiInputConnection, MidiInputPorts};
use serde::{Serialize, Deserialize};
use eframe::egui;
use crossbeam::channel::{Receiver, Sender};
use strum_macros::EnumDiscriminants;

use crate::socket::ClientSocketThreadHandle;

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
    receiver: Receiver<ClientMidiFunction>,
    sender: Sender<ClientMidiFunction>,
    egui_ctx: egui::Context
}

impl MidiState {
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
                name: format!("CC: {} Channel: {}", cc, channel),
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

    pub fn connect_to_port(&mut self, id: &str, client_socket: ClientSocketThreadHandle) {
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
    //                                    CC, channel
    pub port_devices: HashMap<String, HashMap<(u8, u8), MidiDevice>>
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
