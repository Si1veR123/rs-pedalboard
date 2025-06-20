use std::{collections::HashMap, hash::Hash};

use eframe::egui::{self, include_image, Color32, RichText};
use serde::{ser::SerializeMap, Deserialize, Serialize};

use super::{ui::{pedal_knob, pedal_label_rect}, PedalParameter, PedalParameterValue, PedalTrait};

#[derive(Clone)]
pub struct NoiseGate {
    parameters: HashMap<String, PedalParameter>,

    gain: f32,
    is_open: bool,
}

impl Serialize for NoiseGate {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser_map = serializer.serialize_map(Some(self.parameters.len()))?;
        for (key, value) in &self.parameters {
            ser_map.serialize_entry(key, value)?;
        }
        Ok(ser_map.end()?)
    }
}

impl<'de> Deserialize<'de> for NoiseGate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let parameters = HashMap::<String, PedalParameter>::deserialize(deserializer)?;

        let mut noise_gate = Self::new();
        noise_gate.parameters = parameters;

        Ok(noise_gate)
    }
}

impl NoiseGate {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();

        let init_threshold_open = 0.05;
        let init_threshold_close = 0.01;
        let init_release = 0.0001;

        parameters.insert(
            "threshold_open".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(init_threshold_open),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(0.1)),
                step: Some(PedalParameterValue::Float(0.001)),
            },
        );

        parameters.insert(
            "threshold_close".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(init_threshold_close),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(0.1)),
                step: Some(PedalParameterValue::Float(0.001)),
            },
        );

        parameters.insert(
            "release".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(init_release),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(0.001)),
                step: Some(PedalParameterValue::Float(0.00001)),
            },
        );

        Self {
            parameters,
            gain: 1.0,
            is_open: false,
        }
    }
}


impl Hash for NoiseGate {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.parameters.values().map(|p| &p.value).for_each(|v| v.hash(state));
    }
}

impl PedalTrait for NoiseGate {
    fn process_audio(&mut self, buffer: &mut [f32]) {
    
        let threshold_open = self.parameters.get("threshold_open").unwrap().value.as_float().unwrap();
        let threshold_close = self.parameters.get("threshold_close").unwrap().value.as_float().unwrap();
        let release = self.parameters.get("release").unwrap().value.as_float().unwrap();

        for sample in buffer.iter_mut() {
            if self.is_open {
                if sample.abs() < threshold_close {
                    self.is_open = false;
                }
            } else {
                if sample.abs() > threshold_open {
                    self.is_open = true;
                }
            }

            if self.is_open {
                self.gain = 1.0;
            } else {
                self.gain -= release;
                if self.gain < 0.0 {
                    self.gain = 0.0;
                }
            }

            *sample *= self.gain
        }   
        
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn set_parameter_value(&mut self, name: &str, value:PedalParameterValue) {
        if !self.parameters.contains_key(name) || !self.parameters.get(name).unwrap().is_valid(&value) {
            return;
        }

        match name {
            "threshold_open" => {
                if let PedalParameterValue::Float(val) = value {
                    self.parameters.get_mut(name).unwrap().value = PedalParameterValue::Float(val);

                    let threshold_close = self.parameters.get_mut("threshold_close").unwrap();
                    if val < threshold_close.value.as_float().unwrap() {
                        threshold_close.value = PedalParameterValue::Float(val);
                    }
                }
            }
            "threshold_close" => {
                if let PedalParameterValue::Float(val) = value {
                    self.parameters.get_mut(name).unwrap().value = PedalParameterValue::Float(val);

                    let threshold_open = self.parameters.get_mut("threshold_open").unwrap();
                    if val > threshold_open.value.as_float().unwrap() {
                        threshold_open.value = PedalParameterValue::Float(val);
                    }
                }
            }
            _ => {
                self.parameters.get_mut(name).unwrap().value = value;
            }
        }
        
    }

    fn ui(&mut self, ui: &mut eframe::egui::Ui) -> Option<(String,PedalParameterValue)> {
        ui.add(egui::Image::new(include_image!("images/pedal_base.png")));

        let mut to_change = None;

        let threshold_open_param = self.get_parameters().get("threshold_open").unwrap();
        if let Some(value) = pedal_knob(ui, RichText::new("Threshold Open").color(Color32::BLACK).size(8.0), threshold_open_param, egui::Vec2::new(0.025, 0.06), 0.3) {
            to_change = Some(("threshold_open".to_string(), value));
        }

        let threshold_close_param = self.get_parameters().get("threshold_close").unwrap();
        if let Some(value) = pedal_knob(ui, RichText::new("Threshold Close").color(Color32::BLACK).size(8.0), threshold_close_param, egui::Vec2::new(0.35, 0.06), 0.3) {
            to_change = Some(("threshold_close".to_string(), value));
        }

        let release_param = self.get_parameters().get("release").unwrap();
        if let Some(value) = pedal_knob(ui, RichText::new("Release").color(Color32::BLACK).size(8.0), release_param, egui::Vec2::new(0.675, 0.06), 0.3) {
            to_change = Some(("release".to_string(), value));
        }

        let pedal_rect = ui.max_rect();
        ui.put(pedal_label_rect(pedal_rect), egui::Label::new(
            egui::RichText::new("Noise Gate")
                .color(egui::Color32::from_black_alpha(200))
        ));

        to_change
    }
}
