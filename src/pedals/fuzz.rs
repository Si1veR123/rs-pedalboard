use std::collections::HashMap;
use std::hash::Hash;
use super::PedalTrait;
use super::PedalParameter;
use super::PedalParameterValue;
use super::ui::pedal_knob;

use serde::{Serialize, Deserialize};
#[derive(Serialize, Deserialize, Clone)]
pub struct Fuzz {
    parameters: HashMap<String, PedalParameter>,
}

impl Hash for Fuzz {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.parameters.values().map(|p| &p.value).for_each(|v| v.hash(state));
    }
}

impl Fuzz {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();
        parameters.insert(
            "gain".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(20.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(100.0)),
                step: None
            },
        );
        parameters.insert(
            "level".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(2.0)),
                step: None
            },
        );
        Fuzz { parameters }
    }
}

impl PedalTrait for Fuzz {
    fn process_audio(&mut self, buffer: &mut [f32]) {

        let gain = self.parameters.get("gain").unwrap().value.as_float().unwrap();
        let level = self.parameters.get("level").unwrap().value.as_float().unwrap();
        
        for sample in buffer.iter_mut() {
            *sample = (*sample * gain).tanh();
        }
        for sample in buffer.iter_mut() {
            *sample *= level;
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn ui(&mut self, ui: &mut eframe::egui::Ui) -> Option<(String, PedalParameterValue)> {
        //let mut to_change = None;
        //for (parameter_name, parameter) in self.get_parameters().iter() {
        //    if let Some(value) = pedal_knob(ui, parameter_name, parameter) {
        //        to_change = Some((parameter_name.clone(), value));
        //    }
        //}

        //to_change
        None
    }
}
