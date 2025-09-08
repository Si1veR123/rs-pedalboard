use std::collections::HashMap;
use std::hash::Hash;

use crate::dsp_algorithms::moving_bandpass::MovingBandPass;
use crate::pedals::ui::pedal_switch;
use crate::pedals::{PedalParameter, PedalParameterValue, PedalTrait};
use super::ui::pedal_knob;

use eframe::egui::{self, include_image};
use serde::{Serialize, Deserialize, ser::SerializeMap};

#[derive(Clone)]
pub struct AutoWah {
    parameters: HashMap<String, PedalParameter>,
    filter: Option<(MovingBandPass, u32)>,
    envelope: f32,
    id: u32
}

impl Serialize for AutoWah {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser_map = serializer.serialize_map(Some(2))?;
        ser_map.serialize_entry("id", &self.id)?;
        ser_map.serialize_entry("parameters", &self.parameters)?;
        ser_map.end()
    }
}

impl<'a> Deserialize<'a> for AutoWah {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        #[derive(Deserialize)]
        struct AutoWahData {
            id: u32,
            parameters: HashMap<String, PedalParameter>,
        }

        let helper = AutoWahData::deserialize(deserializer)?;
        Ok(AutoWah {
            parameters: helper.parameters,
            filter: None,
            envelope: 0.0,
            id: helper.id
        })
    }
}

impl Hash for AutoWah {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl AutoWah {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();
        parameters.insert(
            "Width".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.5),
                min: Some(PedalParameterValue::Float(0.1)),
                max: Some(PedalParameterValue::Float(2.0)),
                step: None,
            },
        );
        parameters.insert(
            "Sensitivity".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1000.0), // in Hz
                min: Some(PedalParameterValue::Float(100.0)),
                max: Some(PedalParameterValue::Float(3000.0)),
                step: None,
            },
        );
        parameters.insert(
            "Base Freq".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(100.0), // in Hz
                min: Some(PedalParameterValue::Float(50.0)),
                max: Some(PedalParameterValue::Float(1000.0)),
                step: None,
            },
        );
        parameters.insert(
            "Envelope Smoothing".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.9999), // smoothing factor
                min: Some(PedalParameterValue::Float(0.999)),
                max: Some(PedalParameterValue::Float(0.999999)),
                step: None,
            },
        );
        parameters.insert(
            "Dry Wet".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None,
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

        AutoWah {
            parameters,
            filter: None,
            envelope: 0.0,
            id: crate::unique_time_id()
        }
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = crate::unique_time_id();
        cloned
    }
}

impl PedalTrait for AutoWah {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        let (filter, _sample_rate) = match &mut self.filter {
            Some((f, sr)) => (f, sr),
            None => return,
        };

        let sensitivity = self.parameters["Sensitivity"].value.as_float().unwrap();
        let base_freq = self.parameters["Base Freq"].value.as_float().unwrap();
        let envelope_smoothing = self.parameters["Envelope Smoothing"].value.as_float().unwrap();
        let dry_wet = self.parameters["Dry Wet"].value.as_float().unwrap();

        for sample in buffer.iter_mut() {
            self.envelope = envelope_smoothing * self.envelope.max(sample.abs()) + (1.0 - envelope_smoothing) * sample.abs();
            
            let freq = base_freq + self.envelope * sensitivity;
            filter.set_freq(freq);

            *sample = filter.process(*sample) * dry_wet + *sample * (1.0 - dry_wet);
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

                if name == "Width" {
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
        let width = self.parameters["Width"].value.as_float().unwrap();
        let base_freq = self.parameters["Base Freq"].value.as_float().unwrap();
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
        ui.add(egui::Image::new(include_image!("images/autowah.png")));

        let mut to_change = None;

        let base_freq_param = self.get_parameters().get("Base Freq").unwrap();
        if let Some(value) = pedal_knob(ui, "", base_freq_param, egui::Vec2::new(0.68, 0.045), 0.25) {
            to_change = Some(("Base Freq".to_string(), value));
        }

        let sensitivity_param = self.get_parameters().get("Sensitivity").unwrap();
        if let Some(value) = pedal_knob(ui, "", sensitivity_param, egui::Vec2::new(0.68, 0.17), 0.25) {
            to_change = Some(("Sensitivity".to_string(), value));
        }

        let width_param = self.get_parameters().get("Width").unwrap();
        if let Some(value) = pedal_knob(ui, "", width_param, egui::Vec2::new(0.68, 0.295), 0.25) {
            to_change = Some(("Width".to_string(), value));
        }

        let envelope_smoothing_param = self.get_parameters().get("Envelope Smoothing").unwrap();
        if let Some(value) = pedal_knob(ui, "", envelope_smoothing_param, egui::Vec2::new(0.68, 0.425), 0.25) {
            to_change = Some(("Envelope Smoothing".to_string(), value));
        }

        let active_param = self.get_parameters().get("Active").unwrap().value.as_bool().unwrap();
        if let Some(value) = pedal_switch(ui, active_param, egui::Vec2::new(0.33, 0.72), 0.16) {
            to_change = Some(("Active".to_string(), PedalParameterValue::Bool(value)));
        }

        to_change
    }
}