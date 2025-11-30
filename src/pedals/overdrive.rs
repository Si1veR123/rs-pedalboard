// Roughly modelled after a tube screamer

use std::collections::HashMap;
use std::hash::Hash;

use crate::dsp_algorithms::biquad::BiquadFilter;
use crate::unique_time_id;

use super::PedalTrait;
use super::PedalParameter;
use super::PedalParameterValue;
use super::ui::{pedal_knob, pedal_switch};

use eframe::egui::Image;
use eframe::egui::{include_image, self, Vec2};
use serde::ser::SerializeMap;
use serde::{Serialize, Deserialize};

#[derive(Clone)]
pub struct Overdrive {
    parameters: HashMap<String, PedalParameter>,
    // Processor only
    highpass: Option<BiquadFilter>,
    lowpass: Option<BiquadFilter>,
    sample_rate: Option<f32>,
    id: u32,
}

impl Serialize for Overdrive {
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

impl<'a> Deserialize<'a> for Overdrive {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        #[derive(Deserialize)]
        struct OverdriveData {
            id: u32,
            parameters: HashMap<String, PedalParameter>,
        }
        let helper = OverdriveData::deserialize(deserializer)?;
        Ok(Overdrive {
            parameters: helper.parameters,
            lowpass: None,
            highpass: None,
            sample_rate: None,
            id: helper.id
        })
    }
}

impl Hash for Overdrive {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Overdrive {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();
        parameters.insert(
            "Drive".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(10.0),
                min: Some(PedalParameterValue::Float(1.0)),
                max: Some(PedalParameterValue::Float(50.0)),
                step: None
            },
        );
        parameters.insert(
            "Tone".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.5),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None
            },
        );
        parameters.insert(
            "Level".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(10.0)),
                step: None
            },
        );

        parameters.insert(
            "Active".to_string(),
            PedalParameter {
                value: PedalParameterValue::Bool(true),
                min: None,
                max: None,
                step: None
            },
        );
        Overdrive { parameters, lowpass: None, highpass: None, sample_rate: None, id: unique_time_id() }
    }

    pub fn highpass(sample_rate: f32) -> BiquadFilter {
        BiquadFilter::high_pass(720.0, sample_rate, 0.707)
    }

    pub fn lowpass_from_tone(sample_rate: f32, tone: f32) -> BiquadFilter {
        let freq = 3000.0 + (6000.0 - 3000.0) * tone;
        BiquadFilter::low_pass(freq, sample_rate, 0.407)
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned
    }
}

impl PedalTrait for Overdrive {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn set_config(&mut self,_buffer_size:usize, sample_rate: u32) {
        self.highpass = Some(Self::highpass(sample_rate as f32));
        let tone = self.get_parameters().get("Tone").unwrap().value.as_float().unwrap();
        self.lowpass = Some(Self::lowpass_from_tone(sample_rate as f32, tone));
        self.sample_rate = Some(sample_rate as f32);
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        if self.highpass.is_none() || self.lowpass.is_none() {
            tracing::warn!("Overdrive: Filters not initialized. Call set_config first.");
            return;
        }

        let drive = self.get_parameters().get("Drive").unwrap().value.as_float().unwrap();
        let volume = self.get_parameters().get("Level").unwrap().value.as_float().unwrap();
        let pre_highpass = self.highpass.as_mut().unwrap();
        let post_lowpass = self.lowpass.as_mut().unwrap();
        
        for sample in buffer.iter_mut() {
            let mut x = *sample;
            x = pre_highpass.process(x);

            x *= drive;

            x = x.tanh();

            x = post_lowpass.process(x);
            
            x *= volume;
            *sample = x;
        }
    }

    fn set_parameter_value(&mut self,name: &str,value:PedalParameterValue) {
        let parameters = self.get_parameters_mut();
        if let Some(parameter) = parameters.get_mut(name) {
            if parameter.is_valid(&value) {
                if name == "Tone" {
                    let tone = value.as_float().unwrap();
                    parameter.value = value;
                    if let Some(sample_rate) = self.sample_rate {
                        self.lowpass = Some(Self::lowpass_from_tone(sample_rate, tone));
                    }
                } else {
                    parameter.value = value;
                }
            } else {
                tracing::warn!("Attempted to set invalid value for parameter {}: {:?}", name, value);
            }
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String, PedalParameterValue)> {
        ui.add(Image::new(include_image!("images/overdrive.png")));

        let mut to_change = None;
        let drive_param = self.get_parameters().get("Drive").unwrap();
        if let Some(value) = pedal_knob(ui, "", "Drive", drive_param, egui::Vec2::new(0.127, 0.085), 0.35, self.id) {
            to_change = Some(("Drive".to_string(), value));
        }

        let tone_param = self.get_parameters().get("Tone").unwrap();
        if let Some(value) = pedal_knob(ui, "", "Tone", tone_param, egui::Vec2::new(0.535, 0.085), 0.35, self.id) {
            to_change = Some(("Tone".to_string(), value));
        }

        let level_param = self.get_parameters().get("Level").unwrap();
        if let Some(value) = pedal_knob(ui, "", "Level", level_param, egui::Vec2::new(0.325, 0.335), 0.35, self.id) {
            to_change = Some(("Level".to_string(), value));
        }

        let active_param = self.get_parameters().get("Active").unwrap().value.as_bool().unwrap();
        if let Some(value) = pedal_switch(ui, active_param, Vec2::new(0.33, 0.72), 0.16) {
            to_change = Some(("Active".to_string(), PedalParameterValue::Bool(value)));
        }
        
        to_change
    }
}
