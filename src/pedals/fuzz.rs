use std::collections::HashMap;
use std::hash::Hash;
use crate::pedals::ui::pedal_switch;
use crate::unique_time_id;

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
    id: u32
}

impl Hash for Fuzz {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Fuzz {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();
        parameters.insert(
            "Gain".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(20.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(100.0)),
                step: None
            },
        );
        parameters.insert(
            "Level".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(2.0)),
                step: None
            },
        );
        parameters.insert(
            "Type".to_string(),
            PedalParameter {
                value: PedalParameterValue::Int(0),
                min: Some(PedalParameterValue::Int(0)),
                max: Some(PedalParameterValue::Int(4)),
                step: None
            },
        );
        parameters.insert(
            "Dry/Wet".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None
            },
        );
        parameters.insert(
            "Active".to_string(),
            PedalParameter {
                value: PedalParameterValue::Bool(true),
                min: None,
                max: None,
                step: None,
            },
        );
        Fuzz { parameters, id: unique_time_id()}
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned
    }
}

impl PedalTrait for Fuzz {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {

        let gain = self.parameters.get("Gain").unwrap().value.as_float().unwrap();
        let level = self.parameters.get("Level").unwrap().value.as_float().unwrap();
        let fuzz_type = self.parameters.get("Type").unwrap().value.as_int().unwrap();
        let dry_wet = self.parameters.get("Dry/Wet").unwrap().value.as_float().unwrap();
        
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
        let gain_param = self.get_parameters().get("Gain").unwrap();
        if let Some(value) = pedal_knob(ui, "", gain_param, Vec2::new(0.1, 0.07), 0.35) {
            to_change = Some(("Gain".to_string(), value));
        }

        let level_param = self.get_parameters().get("Level").unwrap();
        if let Some(value) = pedal_knob(ui, "", level_param, Vec2::new(0.52, 0.07), 0.35) {
            to_change = Some(("Level".to_string(), value));
        }

        let type_param = self.get_parameters().get("Type").unwrap();
        if let Some(value) = pedal_knob(ui, "", type_param, Vec2::new(0.1, 0.3), 0.35) {
            to_change = Some(("Type".to_string(), value));
        }

        let dry_wet_param = self.get_parameters().get("Dry/Wet").unwrap();
        if let Some(value) = pedal_knob(ui, "", dry_wet_param, Vec2::new(0.52, 0.3), 0.35) {
            to_change = Some(("Dry/Wet".to_string(), value));
        }

        let active_param = self.get_parameters().get("Active").unwrap().value.as_bool().unwrap();
        if let Some(value) = pedal_switch(ui, active_param, egui::Vec2::new(0.33, 0.72), 0.16) {
            to_change = Some(("Active".to_string(), PedalParameterValue::Bool(value)));
        }

        to_change
    }
}
