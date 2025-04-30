use std::collections::HashMap;
use std::hash::Hash;

use super::ui::fill_ui_with_image_width;
use super::PedalTrait;
use super::PedalParameter;
use super::PedalParameterValue;
use super::ui::pedal_knob;

use eframe::egui::include_image;
use eframe::egui::Vec2;
use serde::{Serialize, Deserialize};
#[derive(Serialize, Deserialize, Clone)]
pub struct Volume {
    parameters: HashMap<String, PedalParameter>,
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

    fn ui(&mut self, ui: &mut eframe::egui::Ui) -> Option<(String, PedalParameterValue)> {
        fill_ui_with_image_width(ui, include_image!("images/pedal_base.png"));

        let volume_param = self.get_parameters().get("volume").unwrap();
        if let Some(value) = pedal_knob(ui, "Volume", volume_param, eframe::egui::Vec2::new(0.35, 0.1), 0.3) {
            return Some(("volume".to_string(), value));
        }
        None
    }
}
