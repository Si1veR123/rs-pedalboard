use std::collections::HashMap;
use std::hash::Hash;

use crate::dsp_algorithms::moving_bangpass::MovingBandPass;
use crate::pedals::{PedalParameter, PedalParameterValue, PedalTrait};
use super::ui::{pedal_label_rect, pedal_knob};

use eframe::egui::{self, include_image, Color32, RichText};
use serde::{Serialize, Deserialize, ser::SerializeMap};

#[derive(Clone)]
pub struct AutoWah {
    parameters: HashMap<String, PedalParameter>,
    filter: Option<(MovingBandPass, u32)>,
    envelope: f32,
}

impl Serialize for AutoWah {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser_map = serializer.serialize_map(Some(self.parameters.len()))?;
        for (key, value) in &self.parameters {
            ser_map.serialize_entry(key, value)?;
        }
        ser_map.end()
    }
}

impl<'a> Deserialize<'a> for AutoWah {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        let parameters = HashMap::<String, PedalParameter>::deserialize(deserializer)?;
        Ok(AutoWah {
            parameters,
            filter: None,
            envelope: 0.0,
        })
    }
}

impl Hash for AutoWah {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.parameters.values().map(|p| &p.value).for_each(|v| v.hash(state));
    }
}

impl AutoWah {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();
        parameters.insert(
            "width".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.5),
                min: Some(PedalParameterValue::Float(0.1)),
                max: Some(PedalParameterValue::Float(2.0)),
                step: None,
            },
        );
        parameters.insert(
            "sensitivity".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1000.0), // in Hz
                min: Some(PedalParameterValue::Float(100.0)),
                max: Some(PedalParameterValue::Float(3000.0)),
                step: None,
            },
        );
        parameters.insert(
            "base_freq".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(100.0), // in Hz
                min: Some(PedalParameterValue::Float(50.0)),
                max: Some(PedalParameterValue::Float(1000.0)),
                step: None,
            },
        );
        parameters.insert(
            "envelope_smoothing".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.9999), // smoothing factor
                min: Some(PedalParameterValue::Float(0.999)),
                max: Some(PedalParameterValue::Float(0.999999)),
                step: None,
            },
        );

        AutoWah {
            parameters,
            filter: None,
            envelope: 0.0,
        }
    }
}

impl PedalTrait for AutoWah {
    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        let (filter, _sample_rate) = match &mut self.filter {
            Some((f, sr)) => (f, sr),
            None => return,
        };

        let sensitivity = self.parameters["sensitivity"].value.as_float().unwrap();
        let base_freq = self.parameters["base_freq"].value.as_float().unwrap();
        let envelope_smoothing = self.parameters["envelope_smoothing"].value.as_float().unwrap();
        for sample in buffer.iter_mut() {
            self.envelope = envelope_smoothing * self.envelope.max(sample.abs()) + (1.0 - envelope_smoothing) * sample.abs();
            
            let freq = base_freq + self.envelope * sensitivity;
            filter.set_freq(freq);

            *sample = filter.process(*sample);
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn set_parameter_value(&mut self,name: &str,value:PedalParameterValue) {
        let parameters = self.get_parameters_mut();
        if let Some(parameter) = parameters.get_mut(name) {
            if parameter.is_valid(&value) {
                parameter.value = value;

                if name == "width" {
                    let width = parameter.value.as_float().unwrap();
                    if let Some((bandpass, _)) = &mut self.filter {
                        bandpass.set_width(width);
                    }
                }
            } else {
                log::warn!("Attempted to set invalid value for parameter {}: {:?}", name, value);
            }
        }
    }

    fn set_config(&mut self, _buffer_size: usize, sample_rate: u32) {
        let width = self.parameters["width"].value.as_float().unwrap();
        let base_freq = self.parameters["base_freq"].value.as_float().unwrap();
        let filter = MovingBandPass::new(
            base_freq,
            sample_rate as f32,
            width,
            64,
            5.0
        );
        self.filter = Some((filter, sample_rate));
    }

    fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String, PedalParameterValue)> {
        ui.add(egui::Image::new(include_image!("images/pedal_base.png")));

        let mut to_change = None;

        let width_param = self.get_parameters().get("width").unwrap();
        if let Some(value) = pedal_knob(ui, RichText::new("Width").color(Color32::BLACK).size(8.0), width_param, egui::Vec2::new(0.12, 0.01), 0.25) {
            to_change = Some(("width".to_string(), value));
        }

        let sensitivity_param = self.get_parameters().get("sensitivity").unwrap();
        if let Some(value) = pedal_knob(ui, RichText::new("Sensitivity").color(Color32::BLACK).size(8.0), sensitivity_param, egui::Vec2::new(0.47, 0.01), 0.25) {
            to_change = Some(("sensitivity".to_string(), value));
        }

        let base_freq_param = self.get_parameters().get("base_freq").unwrap();
        if let Some(value) = pedal_knob(ui, RichText::new("Base Freq").color(Color32::BLACK).size(8.0), base_freq_param, egui::Vec2::new(0.3, 0.17), 0.25) {
            to_change = Some(("base_freq".to_string(), value));
        }

        let envelope_smoothing_param = self.get_parameters().get("envelope_smoothing").unwrap();
        if let Some(value) = pedal_knob(ui, RichText::new("Env. Smoothing").color(Color32::BLACK).size(8.0), envelope_smoothing_param, egui::Vec2::new(0.64, 0.17), 0.25) {
            to_change = Some(("envelope_smoothing".to_string(), value));
        }

        let pedal_rect = ui.max_rect();
        ui.put(pedal_label_rect(pedal_rect), egui::Label::new(
            egui::RichText::new("Autowah")
                .color(egui::Color32::from_black_alpha(200))
        ));

        to_change
    }
}