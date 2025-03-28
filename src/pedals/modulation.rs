use std::collections::HashMap;

use crate::dsp_algorithms::variable_delay_phaser::VariableDelayPhaser;
use super::{Pedal, PedalParameter, PedalParameterValue};

macro_rules! var_delay_phaser {
    ($name:ident, ($default_rate:expr, $min_rate:expr, $max_rate:expr), ($default_min_depth:expr, $default_max_depth:expr, $min_depth: expr, $max_depth: expr), $default_mix: expr) => {
        pub struct $name {
            variable_delay_phaser: VariableDelayPhaser,
            parameters: HashMap<String, PedalParameter>,
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
                        value: PedalParameterValue::Selection(init_oscillator),
                        min: Some(PedalParameterValue::Selection(0)),
                        max: Some(PedalParameterValue::Selection(3)),
                        step: None
                    },
                );
        
                Self {
                    variable_delay_phaser: VariableDelayPhaser::new(init_min_depth, init_max_depth, init_rate, init_mix, init_oscillator as usize),
                    parameters
                }
            }
        }
        
        impl Pedal for $name {
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
                        if let PedalParameterValue::Selection(oscillator) = value {
                            self.variable_delay_phaser.set_oscillator(oscillator);
                            self.parameters.get_mut(name).unwrap().value = PedalParameterValue::Selection(oscillator);
                        }
                    },
                    _ => {}
                }
            }
        }
    };
}

var_delay_phaser!(Chorus, (0.8, 0.05, 6.0), (8.0, 25.0, 5.0, 50.0), 0.5);
var_delay_phaser!(Flanger, (3.0, 0.05, 15.0), (0.5, 5.0, 0.0, 10.0), 0.5);