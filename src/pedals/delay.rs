use std::collections::{HashMap, VecDeque};
use std::iter;
use std::hash::Hash;

use crate::dsp_algorithms::{biquad, eq};
use super::ui::{pedal_label_rect, pedal_knob};
use super::{PedalParameter, PedalParameterValue, PedalTrait};

use eframe::egui::{self, include_image};
use serde::ser::SerializeMap;
use serde::{Serialize, Deserialize};

#[derive(Clone)]
pub struct Delay {
    pub parameters: HashMap<String, PedalParameter>,
    delay_buffer: VecDeque<f32>,
    tone_eq: eq::Equalizer
}

impl Hash for Delay {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Probably not technically correct since values may change order but good enough for now
        self.parameters.values().map(|p| &p.value).for_each(|v| v.hash(state));
    }
}

impl Serialize for Delay {
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

impl<'a> Deserialize<'a> for Delay {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        let parameters: HashMap<String, PedalParameter> = HashMap::deserialize(deserializer)?;
        // Just unwrap since if the parameter is missing, the pedal is going to be unusable anyway
        let delay = parameters.get("delay").unwrap().value.as_float().unwrap();
        let delay_samples = ((delay / 1000.0) * 48000.0) as usize;
        let warmth = parameters.get("warmth").unwrap().value.as_float().unwrap();
        let eq = Delay::eq_from_warmth(warmth);
        Ok(Delay { parameters, delay_buffer: VecDeque::from_iter(iter::repeat(0.0).take(delay_samples)), tone_eq: eq })
    }
}

impl Delay {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();

        // Units of 10ms for faster pedal knobs
        let init_delay_ten_ms = 430.0 / 10.0;
        let init_delay_samples = ((init_delay_ten_ms / 100.0) * 48000.0) as usize;
        let init_warmth = 0.0;

        parameters.insert(
            "delay".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(init_delay_ten_ms),
                min: Some(PedalParameterValue::Float(1.0)),
                max: Some(PedalParameterValue::Float(100.0)),
                step: None
            },
        );
        parameters.insert(
            "decay".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.5),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None
            },
        );
        parameters.insert(
            "mix".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.5),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None
            },
        );
        parameters.insert(
            "warmth".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(init_warmth),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None
            },
        );

        let eq = Self::eq_from_warmth(init_warmth);

        Delay { parameters, delay_buffer: VecDeque::from_iter(iter::repeat(0.0).take(init_delay_samples)), tone_eq: eq }
    }

    pub fn eq_from_warmth(tone: f32) -> eq::Equalizer {
        let biquad = biquad::BiquadFilter::high_shelf(4000.0, 48000.0, 0.707, -tone*10.0);
        let eq = eq::Equalizer::new(vec![biquad]);
        eq
    }
}

impl PedalTrait for Delay {
    fn process_audio(&mut self, buffer: &mut [f32]) {
        let decay = self.parameters.get("decay").unwrap().value.as_float().unwrap();
        let mix = self.parameters.get("mix").unwrap().value.as_float().unwrap();
        for sample in buffer.iter_mut() {
            let delay_sample = self.delay_buffer.pop_front().unwrap();

            let mut new_sample = *sample + (delay_sample * decay);
            new_sample = self.tone_eq.process(new_sample);
            self.delay_buffer.push_back(new_sample);

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
                if name == "delay" {
                    let delay_ten_ms = value.as_float().unwrap();
                    let delay_samples = ((delay_ten_ms / 100.0) * 48000.0) as usize;
                    let old_delay = parameter.value.as_float().unwrap();
                    let old_delay_samples = ((old_delay / 1000.0) * 48000.0) as usize;

                    parameter.value = value;

                    if delay_samples < old_delay_samples {
                        self.delay_buffer.truncate(old_delay_samples - delay_samples);
                    } else {
                        self.delay_buffer = VecDeque::from_iter(iter::repeat(0.0).take(delay_samples as usize));
                    }
                } else if name == "warmth" {
                    let warmth = value.as_float().unwrap();
                    parameter.value = value;
                    self.tone_eq = Self::eq_from_warmth(warmth);
                } else {
                    parameter.value = value;
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui) -> Option<(String, PedalParameterValue)> {
        ui.add(egui::Image::new(include_image!("images/pedal_base.png")));

        let mut to_change = None;
        let delay_param = self.get_parameters().get("delay").unwrap();
        if let Some(value) = pedal_knob(ui, "Delay", delay_param, egui::Vec2::new(0.12, 0.01), 0.25) {
            to_change = Some(("delay".to_string(), value));
        }

        let decay_param = self.get_parameters().get("decay").unwrap();
        if let Some(value) = pedal_knob(ui, "Decay", decay_param, egui::Vec2::new(0.47, 0.01), 0.25) {
            to_change = Some(("decay".to_string(), value));
        }

        let warmth_param = self.get_parameters().get("warmth").unwrap();
        if let Some(value) = pedal_knob(ui, "Warmth", warmth_param, egui::Vec2::new(0.3, 0.17), 0.25) {
            to_change = Some(("warmth".to_string(), value));
        }

        let mix_param = self.get_parameters().get("mix").unwrap();
        if let Some(value) = pedal_knob(ui, "Mix", mix_param, egui::Vec2::new(0.64, 0.17), 0.25) {
            to_change = Some(("mix".to_string(), value));
        }

        let pedal_rect = ui.max_rect();
        ui.put(pedal_label_rect(pedal_rect), egui::Label::new(
            egui::RichText::new("Delay")
                .color(egui::Color32::from_black_alpha(200))
        ));

        to_change
    }
}
