#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum MidiChange {
    RelativeValueChange(f32),
    AbsoluteValueChange(f32),
    Toggled,
    Held, 
    None,
}

impl std::fmt::Display for MidiChange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MidiChange::RelativeValueChange(v) => {
                if *v >= 0.0 {
                    write!(f, "+{v:.2}")
                } else {
                    write!(f, "{v:.2}")
                }
            },
            MidiChange::AbsoluteValueChange(v) => write!(f, "{v:.2}"),
            MidiChange::Toggled => write!(f, "Toggle"),
            MidiChange::Held => write!(f, "Held"),
            MidiChange::None => write!(f, "No Change"),
        }
    }
}

impl MidiChange {
    pub fn to_float_setting_update(&self) -> Option<FloatSettingUpdate> {
        match self {
            MidiChange::RelativeValueChange(v) => Some(FloatSettingUpdate::Relative(*v)),
            MidiChange::AbsoluteValueChange(v) => Some(FloatSettingUpdate::Absolute(*v)),
            MidiChange::Toggled | MidiChange::Held => Some(FloatSettingUpdate::FlipFlop),
            MidiChange::None => None,
        }
    }

    pub fn to_parameter_update(&self, range: &PedalParameterRange) -> Option<ParameterUpdate> {
        match self {
            MidiChange::RelativeValueChange(v) => {
                Some(ParameterUpdate::Relative(*v, Some(range.clone())))
            }
            MidiChange::AbsoluteValueChange(v) => {
                let parameter_value = range.parameter_from_interp(*v);
                Some(ParameterUpdate::Absolute(parameter_value))
            }
            MidiChange::Toggled | MidiChange::Held => Some(ParameterUpdate::FlipFlop(Some(range.clone()))),
            MidiChange::None => None,
        }
    }
}

use serde_with::{serde_as, Seq};
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MidiDevice {
    pub id: u32,
    pub name: String,
    pub device_type: MidiDeviceType,
    pub current_value: f32,
    pub global_functions: Vec<GlobalMidiFunction>,
    #[serde_as(as = "Seq<(_, _)>")]
    pub parameter_functions: HashMap<ParameterPath, PedalParameterRange>,
    pub use_global: bool,
    pub sensible_parameter_priority: Option<u32>
}

impl Default for MidiDevice {
    fn default() -> Self {
        MidiDevice {
            id: unique_time_id(),
            name: "New Device".to_string(),
            device_type: MidiDeviceType::AbsoluteEncoder {
                min_value: 0,
                max_value: 127,
            },
            current_value: 0.5,
            sensible_parameter_priority: None,
            global_functions: Vec::new(),
            parameter_functions: HashMap::new(),
            use_global: true,
        }
    }
}

impl MidiDevice {
    pub(super) fn get_sensible_parameter_priority(&self) -> u32 {
        self.sensible_parameter_priority.unwrap_or(u32::MAX)
    }

    pub fn change_from_midi_value(&mut self, midi_value: u8) -> MidiChange {
        match &mut self.device_type {
            MidiDeviceType::RelativeEncoder {
                sensitivity,
                increment_value,
                decrement_value,
            } => {
                if midi_value == *increment_value {
                    self.current_value += *sensitivity;
                    self.current_value = self.current_value.clamp(0.0, 1.0);
                    MidiChange::RelativeValueChange(*sensitivity)
                } else if midi_value == *decrement_value {
                    self.current_value -= *sensitivity;
                    self.current_value = self.current_value.clamp(0.0, 1.0);
                    MidiChange::RelativeValueChange(-*sensitivity)
                } else {
                    tracing::debug!("Received MIDI value {midi_value} for relative encoder, but it is not the increment ({increment_value}) or decrement ({decrement_value}) value. Ignoring.");
                    MidiChange::None
                }
            }
            MidiDeviceType::AbsoluteEncoder {
                min_value,
                max_value,
            } => {
                let range = *max_value as f32 - *min_value as f32;
                let new_value = (midi_value as f32 - *min_value as f32) / range;
                self.current_value = new_value.clamp(0.0, 1.0);
                MidiChange::AbsoluteValueChange(self.current_value)
            }
            MidiDeviceType::Footswitch {
                on_value,
                momentary_to_latching,
                held_global_functions,
                last_pressed_time
            } => {
                if !held_global_functions.is_empty() {
                    if midi_value == *on_value {
                        *last_pressed_time = Some(Instant::now());
                        return MidiChange::None;
                    } else {
                        if let Some(pressed_time) = last_pressed_time {
                            let held_duration = pressed_time.elapsed();
                            *last_pressed_time = None;
                            if held_duration >= Duration::from_millis(500) {
                                return MidiChange::Held;
                            } else {
                                // Momentary footswitch was released before the hold threshold
                                // Treat this as a normal toggle
                                return MidiChange::Toggled;
                            }
                        }
                    }
                }
                let new_state = if *momentary_to_latching {
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
                };

                let toggled = new_state != self.current_value;
                self.current_value = new_state;
                if !toggled {
                    MidiChange::None
                } else {
                    MidiChange::Toggled
                }
            }
        }
    }

    pub fn function_mode(&self, active_pedalboard_id: u32) -> DeviceFunctionMode {
        if self.use_global {
            DeviceFunctionMode::Global
        } else if self
            .parameter_functions
            .iter()
            // Filter to parameter functions which are on the active pedalboard
            .filter(|(path, _range)| path.pedalboard_id == Some(active_pedalboard_id))
            .count()
            == 0
        {
            DeviceFunctionMode::Sensible
        } else {
            DeviceFunctionMode::Parameter
        }
    }

    pub fn display_value_string(&self) -> String {
        match &self.device_type {
            MidiDeviceType::RelativeEncoder { .. } | MidiDeviceType::AbsoluteEncoder { .. } => {
                format!("{:.2}", self.current_value)
            }
            MidiDeviceType::Footswitch { .. } => {
                if self.current_value == 1.0 {
                    "On".into()
                } else {
                    "Off".into()
                }
            }
        }
    }

    pub fn get_held_global_functions(&self) -> Option<&Vec<GlobalMidiFunction>> {
        match &self.device_type {
            MidiDeviceType::Footswitch { held_global_functions, .. } => {
                if held_global_functions.is_empty() {
                    None
                } else {
                    Some(held_global_functions)
                }
            },
            _ => None,
        }
    }
}

/// Which mappings a [`MidiDevice`]'s messages are routed through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceFunctionMode {
    /// The device's global functions.
    Global,
    /// The device's per-parameter functions.
    Parameter,
    /// The device has no functions of its own, so a sensible parameter is chosen instead.
    Sensible,
}
#[derive(Debug, Clone, Serialize, Deserialize, EnumDiscriminants)]
#[strum_discriminants(name(MidiDeviceKind))]
pub enum MidiDeviceType {
    RelativeEncoder {
        sensitivity: f32,
        increment_value: u8,
        decrement_value: u8,
    },
    AbsoluteEncoder {
        min_value: u8,
        max_value: u8,
    },
    Footswitch {
        on_value: u8,
        momentary_to_latching: bool,
        // Global functions which activate when the device is held down
        #[serde(default)]
        held_global_functions: Vec<GlobalMidiFunction>,
        #[serde(skip)]
        last_pressed_time: Option<Instant>,
    },
}

impl MidiDeviceType {
    /// The kind of this device type, without its per-device settings.
    pub fn kind(&self) -> MidiDeviceKind {
        match self {
            MidiDeviceType::RelativeEncoder { .. } => MidiDeviceKind::RelativeEncoder,
            MidiDeviceType::AbsoluteEncoder { .. } => MidiDeviceKind::AbsoluteEncoder,
            MidiDeviceType::Footswitch { .. } => MidiDeviceKind::Footswitch,
        }
    }

    pub fn get_name(&self) -> &'static str {
        match self {
            MidiDeviceType::RelativeEncoder { .. } => "Relative Encoder",
            MidiDeviceType::AbsoluteEncoder { .. } => "Absolute Encoder",
            MidiDeviceType::Footswitch { .. } => "Footswitch",
        }
    }

    /// UI is built for an egui Grid with 2 columns.
    pub fn settings_ui(&mut self, ui: &mut egui::Ui, device_name: &str) {
        ui.style_mut().spacing.slider_width = ui.available_width() * 0.8;
        match self {
            MidiDeviceType::RelativeEncoder {
                sensitivity,
                increment_value,
                decrement_value,
            } => {
                ui.label("Sensitivity:");
                ui.add(egui::Slider::new(sensitivity, 0.01..=1.0));
                ui.end_row();
                ui.label("Increment Value:");
                ui.add(egui::Slider::new(increment_value, 0..=127));
                ui.end_row();
                ui.label("Decrement Value:");
                ui.add(egui::Slider::new(decrement_value, 0..=127));
                ui.end_row();
            }
            MidiDeviceType::AbsoluteEncoder {
                min_value,
                max_value,
            } => {
                ui.label("Min Value:");
                ui.add(egui::Slider::new(min_value, 0..=127));
                ui.end_row();
                ui.label("Max Value:");
                ui.add(egui::Slider::new(max_value, 0..=127));
                ui.end_row();
            }
            MidiDeviceType::Footswitch {
                on_value,
                momentary_to_latching,
                held_global_functions,
                last_pressed_time: _,
            } => {
                ui.label("On Value:");
                ui.add(egui::Slider::new(on_value, 0..=127));
                ui.end_row();
                ui.label("Convert Momentary To Latching:");
                ui.scope(|ui| {
                    crate::ui_helpers::set_large_checkbox_style(ui);
                    ui.checkbox(momentary_to_latching, "")
                });
                ui.end_row();
                ui.label("Held Global Functions:");
                global_midi_function_selector(ui, egui::Id::new(device_name).with("held"), held_global_functions);
                ui.end_row();
            }
        }
    }
}
use rs_pedalboard::pedals::parameters::{ParameterUpdate, PedalParameterRange};
use rs_pedalboard::processor_settings::FloatSettingUpdate;
use rs_pedalboard::unique_time_id;
use rs_pedalboard::pedalboard::ParameterPath;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    time::{Duration, Instant},
};
use strum_macros::EnumDiscriminants;
use eframe::egui;

use super::functions::GlobalMidiFunction;
use super::global_midi_function_selector;
