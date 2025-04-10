use std::collections::HashMap;
use std::hash::Hash;

use enum_dispatch::enum_dispatch;
use serde::{ Deserialize, Serialize};
use eframe::egui;

mod volume;
pub use volume::Volume;
mod fuzz;
pub use fuzz::Fuzz;
mod pitch_shift;
pub use pitch_shift::PitchShift;
mod modulation;
pub use modulation::{Chorus, Flanger};
mod delay;
pub use delay::Delay;
mod eq;
pub use eq::GraphicEq7;
mod ui;

#[derive(Serialize, Deserialize, Clone)]
pub struct PedalParameter {
    pub value: PedalParameterValue,
    // min and max are used for floats and selections
    min: Option<PedalParameterValue>,
    max: Option<PedalParameterValue>,
    // For floats only
    step: Option<PedalParameterValue>
}

impl PedalParameter {
    pub fn is_valid(&self, value: &PedalParameterValue) -> bool {
        match value {
            PedalParameterValue::Float(value) => {
                if let Some(PedalParameterValue::Float(min)) = self.min {
                    if *value < min {
                        return false;
                    }
                }
                if let Some(PedalParameterValue::Float(max)) = self.max {
                    if *value > max {
                        return false;
                    }
                }
                
                // Don't validate float step, but it can be used for hinting to UI

                true
            }
            PedalParameterValue::Int(value) => {
                if let Some(PedalParameterValue::Int(min)) = self.min {
                    if *value < min {
                        return false;
                    }
                }
                if let Some(PedalParameterValue::Int(max)) = self.max {
                    if *value > max {
                        return false;
                    }
                }
                true
            },
            _ => true
        }
    }
}


#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PedalParameterValue {
    Float(f32),
    String(String),
    Bool(bool),
    Int(u16)
}

impl Hash for PedalParameterValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            PedalParameterValue::Float(value) => value.to_bits().hash(state),
            PedalParameterValue::String(value) => value.hash(state),
            PedalParameterValue::Bool(value) => value.hash(state),
            PedalParameterValue::Int(value) => value.hash(state)
        }
    }
}

impl PedalParameterValue {
    pub fn as_float(&self) -> Option<f32> {
        match self {
            PedalParameterValue::Float(value) => Some(*value),
            _ => None
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            PedalParameterValue::String(value) => Some(value),
            _ => None
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            PedalParameterValue::Bool(value) => Some(*value),
            _ => None
        }
    }

    pub fn as_int(&self) -> Option<u16> {
        match self {
            PedalParameterValue::Int(value) => Some(*value),
            _ => None
        }
    }
}

#[enum_dispatch]
pub trait PedalTrait: Send + Hash {
    fn process_audio(&mut self, buffer: &mut [f32]);

    fn get_parameters(&self) -> &HashMap<String, PedalParameter>;
    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter>;

    fn set_parameter_value(&mut self, name: &str, value: PedalParameterValue) {
        let parameters = self.get_parameters_mut();
        if let Some(parameter) = parameters.get_mut(name) {
            if parameter.is_valid(&value) {
                parameter.value = value;
            } else {
                log::warn!("Attempted to set invalid value for parameter {}: {:?}", name, value);
            }
        }
    }

    /// Returns the name of the parameter that needs to be changed, and its value
    fn ui(&mut self, _ui: &mut egui::Ui) -> Option<(String, PedalParameterValue)> { None }
}


/// Wrapper enum type for serialization in Vec
#[derive(Serialize, Deserialize, Clone, Hash)]
#[enum_dispatch(PedalTrait)]
pub enum Pedal {
    Volume(Volume),
    Fuzz(Fuzz),
    PitchShift(PitchShift),
    Chorus(Chorus),
    Flanger(Flanger),
    Delay(Delay),
    GraphicEq7(GraphicEq7)
}
