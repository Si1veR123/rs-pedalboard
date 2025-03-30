use std::collections::HashMap;

use super::PedalTrait;
use super::PedalParameter;
use super::PedalParameterValue;

use serde::{Serialize, Deserialize};
#[derive(Serialize, Deserialize, Clone)]
pub struct Volume {
    parameters: HashMap<String, PedalParameter>,
}

impl Volume {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();
        parameters.insert(
            "volume".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(3.0)),
                step: None
            },
        );
        Volume { parameters }
    }
}

impl PedalTrait for Volume {
    fn process_audio(&mut self, buffer: &mut [f32]) {
        let volume = self.parameters.get("volume").unwrap().value.as_float().unwrap();
        
        for sample in buffer.iter_mut() {
            *sample *= volume;
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }
}
