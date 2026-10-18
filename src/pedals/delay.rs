use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::iter;

use super::ui::pedal_knob;
use super::{PedalParameter, PedalParameterValue, PedalTrait};
use crate::dsp::{biquad, eq, ConvertSampleDbLinear};
use crate::pedals::ui::pedal_switch;
use crate::unique_time_id;

use eframe::egui::{self, include_image};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize};
use tracing::warn;

#[derive(Clone)]
pub struct Delay {
    pub parameters: HashMap<String, PedalParameter>,
    // Processor only
    delay_buffer: Option<VecDeque<f32>>,
    tone_eq: Option<eq::Equalizer>,
    sample_rate: Option<f32>,
    delay_multiplier: Option<f32>,
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
            delay_multiplier: None,
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
                step: None,
            },
        );
        parameters.insert(
            "Decay".to_string(),
            PedalParameter {
                // Internally store the decay as positive, but this maps to -decay dB per sec
                value: PedalParameterValue::Float(10.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(40.0)),
                step: None,
            },
        );
        parameters.insert(
            "Dry/Wet".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.5),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None,
            },
        );
        parameters.insert(
            "Warmth".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(init_warmth),
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

        Delay {
            parameters,
            delay_buffer: None,
            tone_eq: None,
            sample_rate: None,
            delay_multiplier: None,
            id: unique_time_id(),
        }
    }

    pub fn set_warmth_eq(&mut self) {
        if !self.sample_rate.is_some() {
            warn!("Delay: Call set_config() before setting the warmth EQ.");
        }

        let tone = self
            .parameters
            .get("Warmth")
            .map(|p| p.value.as_float().expect("Warmth parameter should be float"))
            .unwrap_or(0.0);

        let sample_rate = self.sample_rate.unwrap();
        let biquad = biquad::BiquadFilter::high_shelf(4000.0, sample_rate, 0.707, -tone * 10.0);
        let eq = eq::Equalizer::new(vec![biquad]);
        self.tone_eq = Some(eq);
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned
    }

    pub fn set_decay_multiplier(&mut self) {
        if !self.sample_rate.is_some() {
            warn!("Delay: Call set_config() before setting the warmth EQ.");
        }

        let decay_db_per_sec = self
            .parameters
            .get("Decay")
            .map(|p| p.value.as_float().expect("Decay parameter should be float"))
            .unwrap_or(10.0);

        let decay_linear_per_sec = f32::db_to_linear(-decay_db_per_sec);
        let decay_linear_per_sample = decay_linear_per_sec.powf(1.0 / self.sample_rate.unwrap());
        self.delay_multiplier = Some(decay_linear_per_sample);
    }
}

impl PedalTrait for Delay {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn set_config(&mut self, _buffer_size: usize, sample_rate: u32) {
        if self.sample_rate == Some(sample_rate as f32) {
            return;
        }

        self.sample_rate = Some(sample_rate as f32);
        self.set_warmth_eq();
        self.set_decay_multiplier();
        
        let delay_ms = self
            .parameters
            .get("Delay")
            .unwrap()
            .value
            .as_float()
            .unwrap();
        let delay_samples = ((delay_ms / 1000.0) * sample_rate as f32) as usize;
        self.delay_buffer = Some(VecDeque::from_iter(iter::repeat(0.0).take(delay_samples)));
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        if self.tone_eq.is_none() || self.delay_buffer.is_none() || self.delay_multiplier.is_none() {
            tracing::warn!("Delay: Call set_config() before processing audio.");
            return;
        }

        let mix = self
            .parameters
            .get("Dry/Wet")
            .unwrap()
            .value
            .as_float()
            .unwrap();

        let decay_multiplier = self.delay_multiplier.as_ref().unwrap();

        for sample in buffer.iter_mut() {
            let delay_sample = self.delay_buffer.as_mut().unwrap().pop_front().unwrap();

            let mut new_sample = *sample + (delay_sample * decay_multiplier);
            new_sample = self.tone_eq.as_mut().unwrap().process(new_sample);
            self.delay_buffer.as_mut().unwrap().push_back(new_sample);

            *sample = *sample * (1.0 - mix) + delay_sample * mix;
        }
    }

    fn reset_buffer(&mut self) {
        if let Some(delay_buffer) = &mut self.delay_buffer {
            delay_buffer.iter_mut().for_each(|s| *s = 0.0);
        }
    }

    fn parameter_value_display_string(
        &self,
        name: &str,
        value: &PedalParameterValue,
    ) -> Option<String> {
        match name {
            "Delay" => Some(format!("{:.0} ms", value.as_float().unwrap())),
            "Decay" => Some(format!("-{:.0} dB", value.as_float().unwrap())),
            "Warmth" => Some(format!("{:.0}%", value.as_float().unwrap() * 100.0)),
            "Dry/Wet" => Some(format!("{:.0}%", value.as_float().unwrap() * 100.0)),
            _ => None,
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn set_parameter_value(&mut self, name: &str, value: PedalParameterValue) {
        let sample_rate_set = self.sample_rate.is_some();

        let parameters = self.get_parameters_mut();
        if let Some(parameter) = parameters.get_mut(name) {
            if parameter.is_valid(&value) {
                if name == "Delay" {
                    // Due to borrowing rules, we must set the parameter value before modifying the delay buffer
                    let delay_ms = value.as_float().unwrap();
                    parameter.value = value;

                    if let Some(delay_buffer) = &mut self.delay_buffer {
                        if let Some(sample_rate) = self.sample_rate {
                            // Modify the delay buffer to match new delay time
                            let delay_samples = ((delay_ms / 1000.0) * sample_rate) as usize;
                            if delay_samples > delay_buffer.len() {
                                delay_buffer.extend(
                                    iter::repeat(0.0).take(delay_samples - delay_buffer.len()),
                                );
                            } else {
                                // shift samples back so the most recent audio is kept when shortening
                                let shift = delay_buffer.len() - delay_samples;
                                for i in 0..delay_samples {
                                    delay_buffer[i] = delay_buffer[i + shift];
                                }
                                delay_buffer.truncate(delay_samples);
                            }
                        }
                    }
                } else if name == "Warmth" {
                    parameter.value = value;
                    if sample_rate_set {
                        self.set_warmth_eq();
                    }
                } else if name == "Decay" {
                    parameter.value = value;
                    if sample_rate_set {
                        self.set_decay_multiplier();
                    }
                } else {
                    parameter.value = value;
                }
            }
        }
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _message_buffer: &[String],
    ) -> Option<(String, PedalParameterValue)> {
        ui.add(egui::Image::new(include_image!("images/delay.png")));

        let mut to_change = None;
        let delay_param = self.get_parameters().get("Delay").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Delay",
            delay_param,
            egui::Vec2::new(0.125, 0.038),
            0.3,
            self.id,
        ) {
            to_change = Some(("Delay".to_string(), value));
        }

        let decay_param = self.get_parameters().get("Decay").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Decay",
            decay_param,
            egui::Vec2::new(0.58, 0.145),
            0.3,
            self.id,
        ) {
            to_change = Some(("Decay".to_string(), value));
        }

        let warmth_param = self.get_parameters().get("Warmth").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Warmth",
            warmth_param,
            egui::Vec2::new(0.125, 0.27),
            0.3,
            self.id,
        ) {
            to_change = Some(("Warmth".to_string(), value));
        }

        let dry_wet_param = self.get_parameters().get("Dry/Wet").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Dry/Wet",
            dry_wet_param,
            egui::Vec2::new(0.58, 0.365),
            0.3,
            self.id,
        ) {
            to_change = Some(("Dry/Wet".to_string(), value));
        }

        let active_param = self
            .get_parameters()
            .get("Active")
            .unwrap()
            .value
            .as_bool()
            .unwrap();
        if let Some(value) = pedal_switch(ui, active_param, egui::Vec2::new(0.33, 0.72), 0.16) {
            to_change = Some(("Active".to_string(), PedalParameterValue::Bool(value)));
        }

        to_change
    }
}