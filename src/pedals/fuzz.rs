use std::collections::HashMap;
use std::hash::Hash;
use super::PedalTrait;
use super::PedalParameter;
use super::PedalParameterValue;
use super::ui::pedal_knob;

use eframe::egui;
use eframe::egui::include_image;
use eframe::egui::Vec2;
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
        parameters.insert(
            "type".to_string(),
            PedalParameter {
                value: PedalParameterValue::Int(0),
                min: Some(PedalParameterValue::Int(0)),
                max: Some(PedalParameterValue::Int(4)),
                step: None
            },
        );
        parameters.insert(
            "dry_wet".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None
            },
        );
        Fuzz { parameters }
    }
}

impl PedalTrait for Fuzz {
    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {

        let gain = self.parameters.get("gain").unwrap().value.as_float().unwrap();
        let level = self.parameters.get("level").unwrap().value.as_float().unwrap();
        let fuzz_type = self.parameters.get("type").unwrap().value.as_int().unwrap();
        let dry_wet = self.parameters.get("dry_wet").unwrap().value.as_float().unwrap();
        
        for sample in buffer.iter_mut() {
            let x = *sample * gain;

            let unmixed_sample = match fuzz_type {
                // 0: Smooth (tanh)
                0 => x.tanh(),
                // 1: Cubic
                1 => x - (x.powf(3.0)) / 3.0,
                // 2: x / (1 + |x|)
                2 => x / (1.0 + x.abs()),
                // 3: atan - smooth, tube-like but a bit brighter than tanh
                3 => (2.0 / std::f32::consts::PI) * x.atan(),
                // 4: Square (sign)
                4 => {
                    if x > 0.0 {
                        1.0
                    } else if x < 0.0 {
                        -1.0
                    } else {
                        0.0
                    }
                },
                _ => {
                    log::warn!("Fuzz: Unknown fuzz type {}.", fuzz_type);
                    x
                }
            };

            *sample = (unmixed_sample * dry_wet) + (*sample * (1.0 - dry_wet));
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

    fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String, PedalParameterValue)> {
        ui.add(egui::Image::new(include_image!("images/fuzz.png")));

        let mut to_change = None;
        let gain_param = self.get_parameters().get("gain").unwrap();
        if let Some(value) = pedal_knob(ui, "", gain_param, Vec2::new(0.1, 0.07), 0.35) {
            to_change = Some(("gain".to_string(), value));
        }

        let level_param = self.get_parameters().get("level").unwrap();
        if let Some(value) = pedal_knob(ui, "", level_param, Vec2::new(0.52, 0.07), 0.35) {
            to_change = Some(("level".to_string(), value));
        }

        let type_param = self.get_parameters().get("type").unwrap();
        if let Some(value) = pedal_knob(ui, "", type_param, Vec2::new(0.1, 0.3), 0.35) {
            to_change = Some(("type".to_string(), value));
        }

        let dry_wet_param = self.get_parameters().get("dry_wet").unwrap();
        if let Some(value) = pedal_knob(ui, "", dry_wet_param, Vec2::new(0.52, 0.3), 0.35) {
            to_change = Some(("dry_wet".to_string(), value));
        }

        to_change
    }
}
