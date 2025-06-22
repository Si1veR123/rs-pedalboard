use std::collections::HashMap;
use std::hash::Hash;
use crate::dsp_algorithms::variable_delay_phaser::VariableDelayPhaser;
use super::{PedalTrait, PedalParameter, PedalParameterValue};
use super::ui::{pedal_knob, pedal_label_rect};
use eframe::egui::{self, Color32, include_image, RichText};
use serde::{Serialize, Deserialize};


macro_rules! var_delay_phaser {
    ($name:ident, $serde_name:ident, ($default_rate:expr, $min_rate:expr, $max_rate:expr), ($default_min_depth:expr, $default_max_depth:expr, $min_depth: expr, $max_depth: expr), ($incl_feedback: expr, $default_feedback:expr, $max_feedback:expr), $default_mix: expr) => {
        #[derive(Clone)]
        pub struct $name {
            variable_delay_phaser: VariableDelayPhaser,
            parameters: HashMap<String, PedalParameter>,
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
                let serde = $serde_name::from(self);
                serde.serialize(serializer)
            }
        }

        impl<'a> Deserialize<'a> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'a>,
            {
                let serde = $serde_name::deserialize(deserializer)?;
                Ok($name::from(serde))
            }
        }

        #[derive(Clone, Serialize, Deserialize)]
        struct $serde_name {
            rate: f32,
            min_depth: f32,
            max_depth: f32,
            mix: f32,
            oscillator: i16
        }

        impl From<&$name> for $serde_name {
            fn from(pedal: &$name) -> Self {
                let rate = pedal.parameters.get("rate").unwrap().value.as_float().unwrap();
                let min_depth = pedal.parameters.get("min_depth").unwrap().value.as_float().unwrap();
                let max_depth = pedal.parameters.get("max_depth").unwrap().value.as_float().unwrap();
                let mix = pedal.parameters.get("mix").unwrap().value.as_float().unwrap();
                let oscillator = pedal.parameters.get("oscillator").unwrap().value.as_int().unwrap();

                Self {
                    rate,
                    min_depth,
                    max_depth,
                    mix,
                    oscillator
                }
            }
        }

        impl From<$serde_name> for $name {
            fn from(serde: $serde_name) -> Self {
                let mut pedal = Self::new();
                pedal.set_parameter_value("rate", PedalParameterValue::Float(serde.rate));
                pedal.set_parameter_value("min_depth", PedalParameterValue::Float(serde.min_depth));
                pedal.set_parameter_value("max_depth", PedalParameterValue::Float(serde.max_depth));
                pedal.set_parameter_value("mix", PedalParameterValue::Float(serde.mix));
                pedal.set_parameter_value("oscillator", PedalParameterValue::Int(serde.oscillator));
                pedal
            }
        }

        impl $name {
            pub fn new() -> Self {
                let mut parameters = HashMap::new();
        
                let init_rate = $default_rate;
                let init_min_depth = $default_min_depth;
                let init_max_depth = $default_max_depth;
                let init_mix = $default_mix;
                let init_oscillator = 0;
        
                parameters.insert(
                    "rate".to_string(),
                    PedalParameter {
                        value: PedalParameterValue::Float(init_rate),
                        min: Some(PedalParameterValue::Float($min_rate)),
                        max: Some(PedalParameterValue::Float($max_rate)),
                        step: None
                    },
                );
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
                        value: PedalParameterValue::Int(init_oscillator),
                        min: Some(PedalParameterValue::Int(0)),
                        max: Some(PedalParameterValue::Int(3)),
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
                    variable_delay_phaser: VariableDelayPhaser::new(init_min_depth, init_max_depth, init_rate, init_mix, init_oscillator as usize, $default_feedback),
                    parameters
                }
            }
        }
        
        impl PedalTrait for $name {
            fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
                self.variable_delay_phaser.process_audio(buffer);
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
                    "rate" => {
                        if let PedalParameterValue::Float(rate) = value {
                            self.variable_delay_phaser.set_rate(rate);
                            self.parameters.get_mut(name).unwrap().value = PedalParameterValue::Float(rate);
                        }
                    },
                    "min_depth" => {
                        if let PedalParameterValue::Float(depth) = value {
                            let current_max_depth = self.parameters.get("max_depth").unwrap().value.as_float().unwrap();
                            let bounded_depth = depth.min(current_max_depth);
                            self.variable_delay_phaser.set_min_depth(bounded_depth);
                            self.parameters.get_mut(name).unwrap().value = PedalParameterValue::Float(bounded_depth);
                        }
                    },
                    "max_depth" => {
                        if let PedalParameterValue::Float(depth) = value {
                            let current_min_depth = self.parameters.get("min_depth").unwrap().value.as_float().unwrap();
                            let bounded_depth = depth.max(current_min_depth);
                            self.variable_delay_phaser.set_max_depth(bounded_depth);
                            self.parameters.get_mut(name).unwrap().value = PedalParameterValue::Float(bounded_depth);
                        }
                    },
                    "mix" => {
                        if let PedalParameterValue::Float(mix) = value {
                            self.variable_delay_phaser.mix = mix;
                            self.parameters.get_mut(name).unwrap().value = PedalParameterValue::Float(mix);
                        }
                    },
                    "oscillator" => {
                        if let PedalParameterValue::Int(oscillator) = value {
                            self.variable_delay_phaser.set_oscillator(oscillator as u16);
                            self.parameters.get_mut(name).unwrap().value = PedalParameterValue::Int(oscillator);
                        }
                    },
                    "feedback" => {
                        if let PedalParameterValue::Float(feedback) = value {
                            self.variable_delay_phaser.feedback = feedback;
                            self.parameters.get_mut(name).unwrap().value = PedalParameterValue::Float(feedback);
                        }
                    },
                    _ => {}
                }
            }

            fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String, PedalParameterValue)> {
                ui.add(egui::Image::new(include_image!("images/pedal_base.png")));

                let mut to_change = None;
                let rate_param = self.get_parameters().get("rate").unwrap();
                if let Some(value) = pedal_knob(ui, RichText::new("Rate").color(Color32::BLACK).size(8.0), rate_param, eframe::egui::Vec2::new(0.1, 0.02), 0.25) {
                    to_change = Some(("rate".to_string(), value));
                }

                let min_depth_param = self.get_parameters().get("min_depth").unwrap();
                if let Some(value) = pedal_knob(ui, RichText::new("Min Depth").color(Color32::BLACK).size(8.0), min_depth_param, eframe::egui::Vec2::new(0.38, 0.02), 0.25) {
                    to_change =  Some(("min_depth".to_string(), value));
                }

                let max_depth_param = self.get_parameters().get("max_depth").unwrap();
                if let Some(value) = pedal_knob(ui, RichText::new("Max Depth").color(Color32::BLACK).size(8.0), max_depth_param, eframe::egui::Vec2::new(0.67, 0.02), 0.25) {
                    to_change =  Some(("max_depth".to_string(), value));
                }

                let mix_param = self.get_parameters().get("mix").unwrap();
                if let Some(value) = pedal_knob(ui, RichText::new("Mix").color(Color32::BLACK).size(8.0), mix_param, eframe::egui::Vec2::new(0.2, 0.22), 0.25) {
                    to_change =  Some(("mix".to_string(), value));
                }

                let oscillator_param = self.get_parameters().get("oscillator").unwrap();
                if let Some(value) = pedal_knob(ui, RichText::new("Oscillator").color(Color32::BLACK).size(8.0), oscillator_param, eframe::egui::Vec2::new(0.55, 0.22), 0.25) {
                    to_change =  Some(("oscillator".to_string(), value));
                }

                if $incl_feedback {
                    let feedback_param = self.get_parameters().get("feedback").unwrap();
                    if let Some(value) = pedal_knob(ui, RichText::new("Feedback").color(Color32::BLACK).size(8.0), feedback_param, eframe::egui::Vec2::new(0.8, 0.22), 0.25) {
                        to_change =  Some(("feedback".to_string(), value));
                    }
                }

                let pedal_rect = ui.max_rect();
                ui.put(pedal_label_rect(pedal_rect), egui::Label::new(
                    egui::RichText::new(stringify!($name))
                        .color(egui::Color32::from_black_alpha(200))
                ));

                to_change
            }
        }
    };
}

var_delay_phaser!(Chorus, ChorusSerde, (0.2, 0.05, 2.0), (8.0, 25.0, 5.0, 40.0), (false, 0.0, 0.0), 0.5);
var_delay_phaser!(Flanger, FlangerSerde, (1.0, 0.05, 10.0), (0.5, 5.0, 0.0, 8.0), (true, 0.0, 0.95), 0.5);