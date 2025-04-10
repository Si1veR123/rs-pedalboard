use std::collections::HashMap;
use std::hash::Hash;
use crate::dsp_algorithms::variable_delay_phaser::VariableDelayPhaser;
use super::{PedalTrait, PedalParameter, PedalParameterValue};
use super::ui::pedal_knob;
use serde::{Serialize, Deserialize};


macro_rules! var_delay_phaser {
    ($name:ident, $serde_name:ident, ($default_rate:expr, $min_rate:expr, $max_rate:expr), ($default_min_depth:expr, $default_max_depth:expr, $min_depth: expr, $max_depth: expr), $default_mix: expr) => {
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
            oscillator: u16
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
                pedal.set_parameter_value("oscillator", PedalParameterValue::Int(serde.oscillator as u16));
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
        
                Self {
                    variable_delay_phaser: VariableDelayPhaser::new(init_min_depth, init_max_depth, init_rate, init_mix, init_oscillator as usize),
                    parameters
                }
            }
        }
        
        impl PedalTrait for $name {
            fn process_audio(&mut self, buffer: &mut [f32]) {
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
                            self.variable_delay_phaser.set_oscillator(oscillator);
                            self.parameters.get_mut(name).unwrap().value = PedalParameterValue::Int(oscillator);
                        }
                    },
                    _ => {}
                }
            }

            fn ui(&mut self, ui: &mut eframe::egui::Ui) -> Option<(String, PedalParameterValue)> {
                let mut to_change = None;
                for (parameter_name, parameter) in self.get_parameters().iter() {
                    if let Some(value) = pedal_knob(ui, parameter_name, parameter) {
                        to_change = Some((parameter_name.clone(), value));
                    }
                }
        
                to_change
            }
        }
    };
}

var_delay_phaser!(Chorus, ChorusSerde, (0.8, 0.05, 6.0), (8.0, 25.0, 5.0, 50.0), 0.5);
// TODO: Flanger feedback?
var_delay_phaser!(Flanger, FlangerSerde, (3.0, 0.05, 15.0), (0.5, 5.0, 0.0, 10.0), 0.5);