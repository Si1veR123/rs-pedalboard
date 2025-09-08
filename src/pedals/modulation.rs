use std::collections::HashMap;
use std::hash::Hash;
use crate::dsp_algorithms::variable_delay_phaser::VariableDelayPhaser;
use crate::dsp_algorithms::oscillator::{Oscillator, Sine};
use super::{PedalTrait, PedalParameter, PedalParameterValue};
use super::ui::{pedal_knob, pedal_switch};
use eframe::egui::{self, include_image, Vec2};
use serde::{Serialize, Deserialize, ser::SerializeMap};


macro_rules! var_delay_phaser {
    ($name:ident, ($default_rate:expr, $min_rate:expr, $max_rate:expr), ($default_min_depth:expr, $default_max_depth:expr, $min_depth: expr, $max_depth: expr), ($incl_feedback: expr, $default_feedback:expr, $max_feedback:expr), $default_dry_wet: expr) => {
        #[derive(Clone)]
        pub struct $name {
            variable_delay_phaser: Option<VariableDelayPhaser>, // Server only
            parameters: HashMap<String, PedalParameter>,
            id: u32,
        }

        impl Hash for $name {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.id.hash(state);
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let mut ser_map = serializer.serialize_map(Some(2))?;
                ser_map.serialize_entry("id", &self.id)?;
                ser_map.serialize_entry("parameters", &self.parameters)?;
                ser_map.end()
            }
        }

        impl<'a> Deserialize<'a> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'a>,
            {
                #[derive(Deserialize)]
                struct VariableDelayPhaserData {
                    id: u32,
                    parameters: HashMap<String, PedalParameter>,
                }

                let helper = VariableDelayPhaserData::deserialize(deserializer)?;

                Ok(Self {
                    variable_delay_phaser: None,
                    parameters: helper.parameters,
                    id: helper.id,
                })
            }
        }

        impl $name {
            pub fn new() -> Self {
                let mut parameters = HashMap::new();
        
                let init_rate = $default_rate;
                let init_min_depth = $default_min_depth;
                let init_max_depth = $default_max_depth;
                let init_dry_wet = $default_dry_wet;
                // Sample rate on oscillators is not used on clients so the hardcoded sample rate is ok
                let init_oscillator = Oscillator::Sine(Sine::new(48000.0, init_rate, 0.0, 0.0));

                parameters.insert(
                    "Min Depth".to_string(),
                    PedalParameter {
                        value: PedalParameterValue::Float(init_min_depth),
                        min: Some(PedalParameterValue::Float($min_depth)),
                        max: Some(PedalParameterValue::Float($max_depth)),
                        step: None
                    },
                );
                parameters.insert(
                    "Max Depth".to_string(),
                    PedalParameter {
                        value: PedalParameterValue::Float(init_max_depth),
                        min: Some(PedalParameterValue::Float($min_depth)),
                        max: Some(PedalParameterValue::Float($max_depth)),
                        step: None
                    },
                );
                parameters.insert(
                    "Dry/Wet".to_string(),
                    PedalParameter {
                        value: PedalParameterValue::Float(init_dry_wet),
                        min: Some(PedalParameterValue::Float(0.0)),
                        max: Some(PedalParameterValue::Float(1.0)),
                        step: None
                    },
                );
                parameters.insert(
                    "Oscillator".to_string(),
                    PedalParameter {
                        value: PedalParameterValue::Oscillator(init_oscillator.clone()),
                        min: Some(PedalParameterValue::Float($min_rate)),
                        max: Some(PedalParameterValue::Float($max_rate)),
                        step: None
                    },
                );

                if $incl_feedback {
                    parameters.insert(
                        "Feedback".to_string(),
                        PedalParameter {
                            value: PedalParameterValue::Float($default_feedback),
                            min: Some(PedalParameterValue::Float(0.0)),
                            max: Some(PedalParameterValue::Float($max_feedback)),
                            step: None
                        },
                    );
                }

                parameters.insert(
                    "Active".to_string(),
                    PedalParameter {
                        value: PedalParameterValue::Bool(true),
                        min: None,
                        max: None,
                        step: None,
                    },
                );
        
                Self {
                    variable_delay_phaser: None,
                    parameters,
                    id: crate::unique_time_id()
                }
            }

            pub fn clone_with_new_id(&self) -> Self {
                let mut cloned = self.clone();
                cloned.id = crate::unique_time_id();
                cloned
            }
        }
        
        impl PedalTrait for $name {
            fn get_id(&self) -> u32 {
                self.id
            }

            fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
                if self.variable_delay_phaser.is_none() {
                    log::error!("{}: VariableDelayPhaser is not initialized. Call set_config() first.", stringify!($name));
                    return;
                }
                self.variable_delay_phaser.as_mut().unwrap().process_audio(buffer);
            }
        
            fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
                &self.parameters
            }
        
            fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
                &mut self.parameters
            }

            fn set_parameter_value(&mut self, name: &str, value: PedalParameterValue) {
                if !self.parameters.contains_key(name) || !self.parameters.get(name).unwrap().is_valid(&value) {
                    return;
                }

                match name {
                    "Min Depth" => {
                        if let PedalParameterValue::Float(depth) = value {
                            let current_max_depth = self.parameters.get("Max Depth").unwrap().value.as_float().unwrap();
                            let bounded_depth = depth.min(current_max_depth);
                            self.parameters.get_mut(name).unwrap().value = PedalParameterValue::Float(bounded_depth);

                            if let Some(variable_delay_phaser) = &mut self.variable_delay_phaser {
                                variable_delay_phaser.set_min_depth(bounded_depth);
                            }
                        }
                    },
                    "Max Depth" => {
                        if let PedalParameterValue::Float(depth) = value {
                            let current_min_depth = self.parameters.get("Min Depth").unwrap().value.as_float().unwrap();
                            let bounded_depth = depth.max(current_min_depth);
                            self.parameters.get_mut(name).unwrap().value = PedalParameterValue::Float(bounded_depth);

                            if let Some(variable_delay_phaser) = &mut self.variable_delay_phaser {
                                variable_delay_phaser.set_max_depth(bounded_depth);
                            }
                        }
                    },
                    "Dry/Wet" => {
                        if let PedalParameterValue::Float(dry_wet) = value {
                            if let Some(variable_delay_phaser) = &mut self.variable_delay_phaser {
                                variable_delay_phaser.mix = dry_wet;
                            }
                            self.parameters.get_mut(name).unwrap().value = PedalParameterValue::Float(dry_wet);
                        }
                    },
                    "Oscillator" => {
                        if let PedalParameterValue::Oscillator(oscillator) = value {
                            if let Some(variable_delay_phaser) = &mut self.variable_delay_phaser {
                                variable_delay_phaser.oscillator = oscillator.clone();
                            }

                            self.parameters.get_mut(name).unwrap().value = PedalParameterValue::Oscillator(oscillator);
                        }
                    },
                    "Feedback" => {
                        if let PedalParameterValue::Float(feedback) = value {
                            if $incl_feedback {
                                if let Some(variable_delay_phaser) = &mut self.variable_delay_phaser {
                                    variable_delay_phaser.feedback = feedback;
                                }
                            } else {
                                log::warn!("Feedback parameter is not included in this pedal.");
                            }
                            self.parameters.get_mut(name).unwrap().value = PedalParameterValue::Float(feedback);
                        }
                    },
                    _ => {
                        if let Some(parameter) = self.parameters.get_mut(name) {
                            parameter.value = value;
                        } else {
                            log::warn!("Attempted to set unknown parameter: {}", name);
                        }
                    }
                }
            }

            fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String, PedalParameterValue)> {
                if $incl_feedback {
                    ui.add(egui::Image::new(include_image!("images/flanger.png")));
                } else {
                    ui.add(egui::Image::new(include_image!("images/chorus.png")));
                }
                let mut to_change = None;

                let min_depth_param = self.get_parameters().get("Min Depth").unwrap();
                if let Some(value) = pedal_knob(ui, "", min_depth_param, eframe::egui::Vec2::new(0.086, 0.036), 0.3) {
                    to_change =  Some(("Min Depth".to_string(), value));
                }

                let max_depth_param = self.get_parameters().get("Max Depth").unwrap();
                if let Some(value) = pedal_knob(ui, "", max_depth_param, eframe::egui::Vec2::new(0.61, 0.036), 0.3) {
                    to_change =  Some(("Max Depth".to_string(), value));
                }

                if $incl_feedback {
                    let feedback_param = self.get_parameters().get("Feedback").unwrap();
                    if let Some(value) = pedal_knob(ui, "", feedback_param, Vec2::new(0.095, 0.3), 0.3) {
                        to_change = Some(("Feedback".to_string(), value));
                    }

                    let dry_wet_param = self.get_parameters().get("Dry/Wet").unwrap();
                    if let Some(value) = pedal_knob(ui, "", dry_wet_param, eframe::egui::Vec2::new(0.605, 0.3), 0.3) {
                        to_change =  Some(("Dry/Wet".to_string(), value));
                    }
                } else {
                    let dry_wet_param = self.get_parameters().get("Dry/Wet").unwrap();
                    if let Some(value) = pedal_knob(ui, "", dry_wet_param, eframe::egui::Vec2::new(0.35, 0.3), 0.3) {
                        to_change =  Some(("Dry/Wet".to_string(), value));
                    }
                }

                let active_param = self.get_parameters().get("Active").unwrap().value.as_bool().unwrap();
                if let Some(value) = pedal_switch(ui, active_param, egui::Vec2::new(0.33, 0.72), 0.16) {
                    to_change = Some(("Active".to_string(), PedalParameterValue::Bool(value)));
                }

                to_change
            }

            fn set_config(&mut self, _buffer_size: usize, sample_rate: u32) {
                let parameter_oscillator = self.parameters.get_mut("Oscillator").unwrap().value.as_oscillator_mut().unwrap();
                parameter_oscillator.set_sample_rate(sample_rate as f32);
                let variable_delay_phaser_oscillator = parameter_oscillator.clone();

                let min_depth = self.parameters.get("Min Depth").unwrap().value.as_float().unwrap();
                let max_depth = self.parameters.get("Max Depth").unwrap().value.as_float().unwrap();
                let dry_wet = self.parameters.get("Dry/Wet").unwrap().value.as_float().unwrap();
                let feedback = if $incl_feedback {
                    self.parameters.get("Feedback").unwrap().value.as_float().unwrap()
                } else {
                    0.0
                };

                self.variable_delay_phaser = Some(VariableDelayPhaser::new(min_depth, max_depth, dry_wet, variable_delay_phaser_oscillator, feedback, sample_rate as f32));
            }
        }
    };
}

var_delay_phaser!(Chorus, (1.0, 0.1, 5.0), (5.0, 15.0, 3.0, 40.0), (false, 0.0, 0.0), 0.5);
var_delay_phaser!(Flanger, (0.25, 0.05, 2.0), (0.3, 2.0, 0.1, 6.0), (true, 0.0, 0.95), 0.5);