use std::collections::HashMap;
use std::hash::Hash;

use super::PedalTrait;
use super::PedalParameter;
use super::PedalParameterValue;
use super::ui::{pedal_knob, pedal_label_rect};

use eframe::egui::Color32;
use eframe::egui::RichText;
use eframe::egui::{include_image, self};
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
        ui.add(eframe::egui::Image::new(include_image!("images/pedal_base.png")).max_height(ui.available_height()));

        let volume_param = self.get_parameters().get("volume").unwrap();
        let mut changed = None;
        if let Some(value) = pedal_knob(ui, RichText::new(&format!("{:.2}", volume_param.value.as_float().unwrap())).color(Color32::BLACK).size(10.0), volume_param, eframe::egui::Vec2::new(0.325, 0.07), 0.35) {
            changed = Some(("volume".to_string(), value));
        }

        let pedal_rect = ui.max_rect();
        ui.put(pedal_label_rect(pedal_rect), egui::Label::new(
            egui::RichText::new("Volume")
                .color(egui::Color32::from_black_alpha(200))
        ));
        
        changed
    }
}
