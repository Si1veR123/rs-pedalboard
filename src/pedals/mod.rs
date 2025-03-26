mod volume;
pub use volume::Volume;
mod fuzz;
pub use fuzz::Fuzz;
mod pitch_shift;
pub use pitch_shift::PitchShift;

use std::collections::HashMap;

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
                if let Some(PedalParameterValue::Float(step)) = self.step {
                    if (*value % step) != 0.0 {
                        return false;
                    }
                }
                true
            }
            PedalParameterValue::Selection(value) => {
                if let Some(PedalParameterValue::Selection(min)) = self.min {
                    if *value < min {
                        return false;
                    }
                }
                if let Some(PedalParameterValue::Selection(max)) = self.max {
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

#[derive(Clone, Debug)]
pub enum PedalParameterValue {
    Float(f32),
    String(String),
    Bool(bool),
    Selection(u8)
}

impl PedalParameterValue {
    pub fn as_float(&self) -> Option<f32> {
        match self {
            PedalParameterValue::Float(value) => Some(*value),
            _ => None
        }
    }

    pub fn as_string(&self) -> Option<&str> {
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

    pub fn as_selection(&self) -> Option<u8> {
        match self {
            PedalParameterValue::Selection(value) => Some(*value),
            _ => None
        }
    }
}


pub trait Pedal: Send {
    fn init(&mut self) {}

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
}
