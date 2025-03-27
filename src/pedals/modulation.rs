use std::collections::HashMap;

use crate::dsp_algorithms::variable_delay_phaser::VariableDelayPhaser;
use super::{Pedal, PedalParameter, PedalParameterValue};

macro_rules! var_delay_phaser {
    ($name:ident, ($default_rate:expr, $min_rate:expr, $max_rate:expr), ($default_depth:expr, $min_depth: expr, $max_depth: expr), $default_mix: expr) => {
        pub struct $name {
            variable_delay_phaser: VariableDelayPhaser,
            parameters: HashMap<String, PedalParameter>,
        }

        impl $name {
            pub fn new() -> Self {
                let mut parameters = HashMap::new();
        
                let init_rate = $default_rate;
                let init_depth = $default_depth;
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
                    "depth".to_string(),
                    PedalParameter {
                        value: PedalParameterValue::Float(init_depth),
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
                    variable_delay_phaser: VariableDelayPhaser::new(init_depth, init_rate, init_mix, init_oscillator as usize),
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
        }
    };
}

var_delay_phaser!(Chorus, (0.8, 0.05, 6.0), (10.0, 5.0, 40.0), 0.5);
var_delay_phaser!(Flanger, (5.0, 0.1, 10.0), (2.0, 0.1, 6.0), 0.5);