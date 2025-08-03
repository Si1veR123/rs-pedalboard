use std::collections::HashMap;
use std::hash::Hash;
use crate::dsp_algorithms::variable_delay_phaser::VariableDelayPhaser;
use crate::dsp_algorithms::oscillator::{Oscillator, Sine};
use super::{PedalTrait, PedalParameter, PedalParameterValue};
use super::ui::{pedal_knob, pedal_label_rect, oscillator_selection_window};
use eframe::egui::{self, Color32, include_image, RichText, Vec2};
use serde::{Serialize, Deserialize, ser::SerializeMap};


macro_rules! var_delay_phaser {
    ($name:ident, ($default_rate:expr, $min_rate:expr, $max_rate:expr), ($default_min_depth:expr, $default_max_depth:expr, $min_depth: expr, $max_depth: expr), ($incl_feedback: expr, $default_feedback:expr, $max_feedback:expr), $default_mix: expr) => {
        #[derive(Clone)]
        pub struct $name {
            variable_delay_phaser: Option<VariableDelayPhaser>, // Server only
            parameters: HashMap<String, PedalParameter>,
            oscillator_open: bool
        }

        impl Hash for $name {
            fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
                self.parameters.values().map(|p| &p.value).for_each(|v| v.hash(state));
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                let mut ser_map = serializer.serialize_map(Some(self.parameters.len()))?;
                for (key, value) in &self.parameters {
                    ser_map.serialize_entry(key, value)?;
                }
                ser_map.end()
            }
        }

        impl<'a> Deserialize<'a> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'a>,
            {
                let parameters = HashMap::<String, PedalParameter>::deserialize(deserializer)?;

                Ok(Self {
                    variable_delay_phaser: None,
                    parameters,
                    oscillator_open: false
                })
            }
        }

        impl $name {
            pub fn new() -> Self {
                let mut parameters = HashMap::new();
        
                let init_rate = $default_rate;
                let init_min_depth = $default_min_depth;
                let init_max_depth = $default_max_depth;
                let init_mix = $default_mix;
                // Sample rate on oscillators is not used on clients so the hardcoded sample rate is ok
                let init_oscillator = Oscillator::Sine(Sine::new(48000.0, init_rate, 0.0, 0.0));

                parameters.insert(
                    "min_depth".to_string(),
                    PedalParameter {
                        value: PedalParameterValue::Float(init_min_depth),
                        min: Some(PedalParameterValue::Float($min_depth)),
                        max: Some(PedalParameterValue::Float($max_depth)),
                        step: None
                    },
                );
                parameters.insert(
                    "max_depth".to_string(),
                    PedalParameter {
                        value: PedalParameterValue::Float(init_max_depth),
                        min: Some(PedalParameterValue::Float($min_depth)),
                        max: Some(PedalParameterValue::Float($max_depth)),
                        step: None
                    },
                );
                parameters.insert(
                    "mix".to_string(),
                    PedalParameter {
                        value: PedalParameterValue::Float(init_mix),
                        min: Some(PedalParameterValue::Float(0.0)),
                        max: Some(PedalParameterValue::Float(1.0)),
                        step: None
                    },
                );
                parameters.insert(
                    "oscillator".to_string(),
                    PedalParameter {
                        value: PedalParameterValue::Oscillator(init_oscillator.clone()),
                        min: None,
                        max: None,
                        step: None
                    },
                );

                if $incl_feedback {
                    parameters.insert(
                        "feedback".to_string(),
                        PedalParameter {
                            value: PedalParameterValue::Float($default_feedback),
                            min: Some(PedalParameterValue::Float(0.0)),
                            max: Some(PedalParameterValue::Float($max_feedback)),
                            step: None
                        },
                    );
                }
        
                Self {
                    variable_delay_phaser: None,
                    parameters,
                    oscillator_open: false
                }
            }
        }
        
        impl PedalTrait for $name {
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
                    "min_depth" => {
                        if let PedalParameterValue::Float(depth) = value {
                            let current_max_depth = self.parameters.get("max_depth").unwrap().value.as_float().unwrap();
                            let bounded_depth = depth.min(current_max_depth);
                            self.parameters.get_mut(name).unwrap().value = PedalParameterValue::Float(bounded_depth);

                            if let Some(variable_delay_phaser) = &mut self.variable_delay_phaser {
                                variable_delay_phaser.set_min_depth(bounded_depth);
                            }
                        }
                    },
                    "max_depth" => {
                        if let PedalParameterValue::Float(depth) = value {
                            let current_min_depth = self.parameters.get("min_depth").unwrap().value.as_float().unwrap();
                            let bounded_depth = depth.max(current_min_depth);
                            self.parameters.get_mut(name).unwrap().value = PedalParameterValue::Float(bounded_depth);

                            if let Some(variable_delay_phaser) = &mut self.variable_delay_phaser {
                                variable_delay_phaser.set_max_depth(bounded_depth);
                            }
                        }
                    },
                    "mix" => {
                        if let PedalParameterValue::Float(mix) = value {
                            if let Some(variable_delay_phaser) = &mut self.variable_delay_phaser {
                                variable_delay_phaser.mix = mix;
                            }
                            self.parameters.get_mut(name).unwrap().value = PedalParameterValue::Float(mix);
                        }
                    },
                    "oscillator" => {
                        if let PedalParameterValue::Oscillator(oscillator) = value {
                            if let Some(variable_delay_phaser) = &mut self.variable_delay_phaser {
                                variable_delay_phaser.oscillator = oscillator.clone();
                            }

                            self.parameters.get_mut(name).unwrap().value = PedalParameterValue::Oscillator(oscillator);
                        }
                    },
                    "feedback" => {
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
                    _ => {}
                }
            }

            fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String, PedalParameterValue)> {
                ui.add(egui::Image::new(include_image!("images/pedal_base.png")));

                let mut to_change = None;

                let min_depth_param = self.get_parameters().get("min_depth").unwrap();
                if let Some(value) = pedal_knob(ui, RichText::new("Min Depth").color(Color32::BLACK).size(8.0), min_depth_param, eframe::egui::Vec2::new(0.08, 0.02), 0.25) {
                    to_change =  Some(("min_depth".to_string(), value));
                }

                let max_depth_param = self.get_parameters().get("max_depth").unwrap();
                if let Some(value) = pedal_knob(ui, RichText::new("Max Depth").color(Color32::BLACK).size(8.0), max_depth_param, eframe::egui::Vec2::new(0.38, 0.02), 0.25) {
                    to_change =  Some(("max_depth".to_string(), value));
                }

                let mix_param = self.get_parameters().get("mix").unwrap();
                if let Some(value) = pedal_knob(ui, RichText::new("Mix").color(Color32::BLACK).size(8.0), mix_param, eframe::egui::Vec2::new(0.67, 0.02), 0.25) {
                    to_change =  Some(("mix".to_string(), value));
                }

                let offset_x: f32;
                let offset_y: f32;
                if $incl_feedback {
                    offset_x = 0.06 * ui.available_width();
                    offset_y = 0.3 * ui.available_height();
                    
                } else {
                    offset_x = 0.2 * ui.available_width();
                    offset_y = 0.3 * ui.available_height();
                }

                let oscillator_button_rect = egui::Rect::from_min_size(
                    ui.max_rect().min + Vec2::new(offset_x, offset_y),
                    Vec2::new(0.6 * ui.available_width(), 0.1 * ui.available_height())
                );

                if ui.put(oscillator_button_rect, egui::Button::new(
                    RichText::new("Oscillator")
                        .color(Color32::WHITE)
                        .size(9.0)
                )).clicked() {
                    self.oscillator_open = !self.oscillator_open;
                };

                if self.oscillator_open {
                    if let Some(osc) = oscillator_selection_window(
                        ui,
                        &self.parameters.get("oscillator").unwrap().value.as_oscillator().unwrap(),
                        &mut self.oscillator_open,
                        false,
                        Some($min_rate..=$max_rate)
                    ) {
                        to_change = Some(("oscillator".to_string(), PedalParameterValue::Oscillator(osc)));
                    }
                }

                if $incl_feedback {
                    let feedback_param = self.get_parameters().get("feedback").unwrap();
                    if let Some(value) = pedal_knob(ui, RichText::new("Feedback").color(Color32::BLACK).size(8.0), feedback_param, Vec2::new(0.7, 0.2), 0.25) {
                        to_change = Some(("feedback".to_string(), value));
                    }
                }

                let pedal_rect = ui.max_rect();
                ui.put(pedal_label_rect(pedal_rect), egui::Label::new(
                    egui::RichText::new(stringify!($name))
                        .color(egui::Color32::from_black_alpha(200))
                ));

                to_change
            }

            fn set_config(&mut self, _buffer_size: usize, sample_rate: u32) {
                let parameter_oscillator = self.parameters.get_mut("oscillator").unwrap().value.as_oscillator_mut().unwrap();
                parameter_oscillator.set_sample_rate(sample_rate as f32);
                let variable_delay_phaser_oscillator = parameter_oscillator.clone();

                let min_depth = self.parameters.get("min_depth").unwrap().value.as_float().unwrap();
                let max_depth = self.parameters.get("max_depth").unwrap().value.as_float().unwrap();
                let mix = self.parameters.get("mix").unwrap().value.as_float().unwrap();
                let feedback = if $incl_feedback {
                    self.parameters.get("feedback").unwrap().value.as_float().unwrap()
                } else {
                    0.0
                };

                self.variable_delay_phaser = Some(VariableDelayPhaser::new(min_depth, max_depth, mix, variable_delay_phaser_oscillator, feedback, sample_rate as f32));
            }
        }
    };
}

var_delay_phaser!(Chorus, (1.0, 0.1, 5.0), (5.0, 15.0, 3.0, 40.0), (false, 0.0, 0.0), 0.5);
var_delay_phaser!(Flanger, (0.25, 0.05, 2.0), (0.3, 2.0, 0.1, 6.0), (true, 0.0, 0.95), 0.5);