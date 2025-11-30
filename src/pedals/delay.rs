use std::collections::{HashMap, VecDeque};
use std::iter;
use std::hash::Hash;

use crate::dsp_algorithms::{biquad, eq};
use crate::pedals::ui::pedal_switch;
use crate::unique_time_id;
use super::ui::pedal_knob;
use super::{PedalParameter, PedalParameterValue, PedalTrait};

use eframe::egui::{self, include_image};
use serde::ser::SerializeMap;
use serde::{Serialize, Deserialize};

#[derive(Clone)]
pub struct Delay {
    pub parameters: HashMap<String, PedalParameter>,
    // Server only
    delay_buffer: Option<VecDeque<f32>>,
    tone_eq: Option<eq::Equalizer>,
    sample_rate: Option<f32>,
    id: u32,
}

impl Hash for Delay {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Serialize for Delay {
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

impl<'de> Deserialize<'de> for Delay {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct DelayData {
            id: u32,
            parameters: HashMap<String, PedalParameter>,
        }

        let helper = DelayData::deserialize(deserializer)?;
        Ok(Delay {
            id: helper.id,
            parameters: helper.parameters,
            delay_buffer: None,
            tone_eq: None,
            sample_rate: None,
        })
    }
}

impl Delay {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();

        let init_delay = 430.0;
        let init_warmth = 0.0;

        parameters.insert(
            "Delay".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(init_delay),
                min: Some(PedalParameterValue::Float(10.0)),
                max: Some(PedalParameterValue::Float(1000.0)),
                step: None
            },
        );
        parameters.insert(
            "Decay".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.5),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None
            },
        );
        parameters.insert(
            "Dry/Wet".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.5),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None
            },
        );
        parameters.insert(
            "Warmth".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(init_warmth),
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

        Delay {
            parameters,
            delay_buffer: None,
            tone_eq: None,
            sample_rate: None,
            id: unique_time_id(),
        }
    }

    pub fn eq_from_warmth(tone: f32, sample_rate: f32) -> eq::Equalizer {
        let biquad = biquad::BiquadFilter::high_shelf(4000.0, sample_rate, 0.707, -tone*10.0);
        let eq = eq::Equalizer::new(vec![biquad]);
        eq
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned
    }
}

impl PedalTrait for Delay {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn set_config(&mut self, _buffer_size: usize, sample_rate: u32) {
        self.tone_eq = Some(
            Self::eq_from_warmth(self.parameters.get("Warmth").unwrap().value.as_float().unwrap(), sample_rate as f32)
        );
        self.sample_rate = Some(sample_rate as f32);
        let delay_ten_ms = self.parameters.get("Delay").unwrap().value.as_float().unwrap();
        let delay_samples = ((delay_ten_ms / 100.0) * sample_rate as f32) as usize;
        self.delay_buffer = Some(
            VecDeque::from_iter(iter::repeat(0.0).take(delay_samples))
        );
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        if self.tone_eq.is_none() || self.delay_buffer.is_none() {
            tracing::warn!("Delay: Call set_config() before processing audio.");
            return;
        }

        let decay = self.parameters.get("Decay").unwrap().value.as_float().unwrap();
        let mix = self.parameters.get("Dry/Wet").unwrap().value.as_float().unwrap();
        for sample in buffer.iter_mut() {
            let delay_sample = self.delay_buffer.as_mut().unwrap().pop_front().unwrap();

            let mut new_sample = *sample + (delay_sample * decay);
            new_sample = self.tone_eq.as_mut().unwrap().process(new_sample);
            self.delay_buffer.as_mut().unwrap().push_back(new_sample);

            *sample = *sample * (1.0 - mix) + delay_sample * mix;
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn set_parameter_value(&mut self, name: &str, value: PedalParameterValue) {
        let parameters = self.get_parameters_mut();
        if let Some(parameter) = parameters.get_mut(name) {
            if parameter.is_valid(&value) {
                if name == "Delay" {
                    let delay_ms = value.as_float().unwrap();
                    parameter.value = value;

                    if let Some(delay_buffer) = &mut self.delay_buffer {
                        if let Some(sample_rate) = self.sample_rate {
                            let delay_samples = ((delay_ms / 1000.0) * sample_rate) as usize;
                            if delay_samples > delay_buffer.len() {
                                delay_buffer.extend(iter::repeat(0.0).take(delay_samples - delay_buffer.len()));
                            } else {
                                delay_buffer.truncate(delay_samples);
                            }
                        }
                    }
                } else if name == "Warmth" {
                    let warmth = value.as_float().unwrap();
                    parameter.value = value;
                    if let Some(sample_rate) = self.sample_rate {
                        self.tone_eq = Some(
                            Self::eq_from_warmth(warmth, sample_rate)
                        );
                    }
                    
                } else {
                    parameter.value = value;
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String, PedalParameterValue)> {
        ui.add(egui::Image::new(include_image!("images/delay.png")));

        let mut to_change = None;
        let delay_param = self.get_parameters().get("Delay").unwrap();
        if let Some(value) = pedal_knob(ui, "", "Delay", delay_param, egui::Vec2::new(0.125, 0.038), 0.3, self.id) {
            to_change = Some(("Delay".to_string(), value));
        }

        let decay_param = self.get_parameters().get("Decay").unwrap();
        if let Some(value) = pedal_knob(ui, "", "Decay", decay_param, egui::Vec2::new(0.58, 0.145), 0.3, self.id) {
            to_change = Some(("Decay".to_string(), value));
        }

        let warmth_param = self.get_parameters().get("Warmth").unwrap();
        if let Some(value) = pedal_knob(ui, "", "Warmth", warmth_param, egui::Vec2::new(0.125, 0.27), 0.3, self.id) {
            to_change = Some(("Warmth".to_string(), value));
        }

        let dry_wet_param = self.get_parameters().get("Dry/Wet").unwrap();
        if let Some(value) = pedal_knob(ui, "", "Dry/Wet", dry_wet_param, egui::Vec2::new(0.58, 0.365), 0.3, self.id) {
            to_change = Some(("Dry/Wet".to_string(), value));
        }

        let active_param = self.get_parameters().get("Active").unwrap().value.as_bool().unwrap();
        if let Some(value) = pedal_switch(ui, active_param, egui::Vec2::new(0.33, 0.72), 0.16) {
            to_change = Some(("Active".to_string(), PedalParameterValue::Bool(value)));
        }

        to_change
    }
}
