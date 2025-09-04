use std::collections::HashMap;
use std::hash::Hash;

use crate::unique_time_id;

use super::PedalTrait;
use super::PedalParameter;
use super::PedalParameterValue;
use super::ui::{pedal_knob, pedal_switch};

use eframe::egui::Color32;
use eframe::egui::Image;
use eframe::egui::RichText;
use eframe::egui::{include_image, self, Vec2};
use serde::{Serialize, Deserialize};


#[derive(Serialize, Deserialize)]
pub struct Volume {
    parameters: HashMap<String, PedalParameter>,
    id: u32,
}

impl Clone for Volume {
    fn clone(&self) -> Self {
        Volume {
            parameters: self.parameters.clone(),
            id: unique_time_id()
        }
    }
}

impl Hash for Volume {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.parameters.values().map(|p| &p.value).for_each(|v| v.hash(state));
    }
}

impl Volume {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();
        parameters.insert(
            "volume".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(5.0)),
                step: None
            },
        );
        parameters.insert(
            "active".to_string(),
            PedalParameter {
                value: PedalParameterValue::Bool(true),
                min: None,
                max: None,
                step: None
            },
        );
        Volume { parameters, id: unique_time_id() }
    }
}

impl PedalTrait for Volume {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
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

    fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String, PedalParameterValue)> {
        ui.add(Image::new(include_image!("images/volume.png")));

        let volume_param = self.get_parameters().get("volume").unwrap();
        let mut changed = None;
        if let Some(value) = pedal_knob(ui, RichText::new(&format!("{:.2}", volume_param.value.as_float().unwrap())).color(Color32::BLACK).size(10.0), volume_param, Vec2::new(0.3, 0.2), 0.4) {
            changed = Some(("volume".to_string(), value));
        }
        let active_param = self.get_parameters().get("active").unwrap().value.as_bool().unwrap();
        if let Some(value) = pedal_switch(ui, active_param, Vec2::new(0.33, 0.72), 0.16) {
            changed = Some(("active".to_string(), PedalParameterValue::Bool(value)));
        }
        
        changed
    }
}
