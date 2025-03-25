pub mod bypass_pedal;

use std::collections::HashMap;

#[derive(Clone, Debug)]
pub enum PedalParameter {
    // (current value, min, max)
    BoundedFloat((f32, f32, f32)),
    String(String),
    Bool(bool),
    // (current value, max option)
    Selection((u8, u8))
}

impl PedalParameter {
    fn is_valid(&self) -> bool {
        match self {
            PedalParameter::BoundedFloat((value, min, max)) => value >= min && value <= max,
            PedalParameter::String(_) => true,
            PedalParameter::Bool(_) => true,
            PedalParameter::Selection((value, max)) => value <= max
        }
    }
}

pub trait Pedal: Send {
    fn init(&mut self) {}

    fn process_audio(&mut self, buffer: &mut [f32]);

    fn get_properties(&self) -> &HashMap<String, PedalParameter>;
    fn get_properties_mut(&mut self) -> &mut HashMap<String, PedalParameter>;

    fn update_property(&mut self, key: &str, value: PedalParameter) -> Option<PedalParameter> {
        if value.is_valid() {
            self.get_properties_mut().insert(key.to_string(), value)
        } else {
            None
        }
    }
    fn get_property(&self, key: &str) -> Option<PedalParameter> {
        self.get_properties().get(key).cloned()
    }
}
