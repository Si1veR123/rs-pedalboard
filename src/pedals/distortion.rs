// Roughly modelled after a DS-1

use std::collections::HashMap;
use std::hash::Hash;

use crate::dsp_algorithms::biquad::BiquadFilter;
use crate::unique_time_id;

use super::ui::{pedal_knob, pedal_switch};
use super::PedalParameter;
use super::PedalParameterValue;
use super::PedalTrait;

use eframe::egui::Image;
use eframe::egui::{self, include_image, Vec2};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct Distortion {
    parameters: HashMap<String, PedalParameter>,
    // Processor only
    low_tilt: Option<BiquadFilter>,
    high_tilt: Option<BiquadFilter>,
    post_filter: Option<BiquadFilter>,
    sample_rate: Option<f32>,
    id: u32,
}

impl Serialize for Distortion {
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

impl<'de> Deserialize<'de> for Distortion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct DistortionData {
            id: u32,
            parameters: HashMap<String, PedalParameter>,
        }

        let helper = DistortionData::deserialize(deserializer)?;
        Ok(Distortion {
            id: helper.id,
            parameters: helper.parameters,
            low_tilt: None,
            high_tilt: None,
            post_filter: None,
            sample_rate: None,
        })
    }
}

impl Hash for Distortion {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Distortion {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();
        parameters.insert(
            "Drive".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(10.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(50.0)),
                step: None,
            },
        );
        parameters.insert(
            "Tone".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.5),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None,
            },
        );
        parameters.insert(
            "Level".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(3.0)),
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
        parameters.insert(
            "Asymmetry".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.5),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None,
            },
        );
        Distortion {
            parameters,
            low_tilt: None,
            high_tilt: None,
            post_filter: None,
            sample_rate: None,
            id: unique_time_id(),
        }
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned
    }

    pub fn post_eq(sample_rate: f32) -> (BiquadFilter, BiquadFilter) {
        let pivot = 2500.0;
        let low = BiquadFilter::low_pass(pivot, sample_rate, 0.707);
        let high = BiquadFilter::high_pass(pivot, sample_rate, 0.707);
        (low, high)
    }

    fn tone_mix(tone: f32) -> f32 {
        0.2 + tone.clamp(0.0, 1.0) * 0.6
    }

    pub fn hard_diode(x: f32, threshold: f32, knee: f32) -> f32 {
        if x > threshold {
            threshold + (x - threshold) / (1.0 + knee * (x - threshold).abs())
        } else if x < -threshold {
            -threshold + (x + threshold) / (1.0 + knee * (x + threshold).abs())
        } else {
            x
        }
    }

    fn asymmetric_diode(
        x: f32,
        positive_threshold: f32,
        negative_threshold: f32,
        knee: f32,
    ) -> f32 {
        if x >= positive_threshold {
            positive_threshold
                + (x - positive_threshold) / (1.0 + knee * (x - positive_threshold).abs())
        } else if x <= -negative_threshold {
            -negative_threshold
                + (x + negative_threshold) / (1.0 + knee * (x + negative_threshold).abs())
        } else {
            x
        }
    }
}

impl PedalTrait for Distortion {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn set_config(&mut self, _buffer_size: usize, sample_rate: u32) {
        self.sample_rate = Some(sample_rate as f32);
        let (low_tilt, high_tilt) = Self::post_eq(sample_rate as f32);
        self.low_tilt = Some(low_tilt);
        self.high_tilt = Some(high_tilt);
        self.post_filter = Some(BiquadFilter::low_pass(9000.0, sample_rate as f32, 0.707));
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        if self.high_tilt.is_none() || self.low_tilt.is_none() || self.post_filter.is_none() {
            tracing::warn!("Distortion: Filters not initialized. Call set_config first.");
            return;
        }

        let drive = self
            .get_parameters()
            .get("Drive")
            .unwrap()
            .value
            .as_float()
            .unwrap();
        let volume = self
            .get_parameters()
            .get("Level")
            .unwrap()
            .value
            .as_float()
            .unwrap();
        let asymmetry = self
            .get_parameters()
            .get("Asymmetry")
            .map(|p| p.value.as_float().unwrap())
            .unwrap_or(0.5); // Default to 0.5 if not found, as this is a new parameter
        let asymmetry_amount = (asymmetry - 0.5) * 2.0;

        let tone = self
            .get_parameters()
            .get("Tone")
            .unwrap()
            .value
            .as_float()
            .unwrap();
        let tone_mix = Self::tone_mix(tone);

        for sample in buffer.iter_mut() {
            let mut x = *sample;

            x *= 1.0 + drive * 0.5;
            let positive_threshold = 1.0 * (1.0 - 0.2 * asymmetry_amount);
            let negative_threshold = 1.0 * (1.0 + 0.2 * asymmetry_amount);
            x = Self::asymmetric_diode(x, positive_threshold, negative_threshold, 5.0);
            x /= (1.0 + drive * 0.5).sqrt();

            let low = self.low_tilt.as_mut().unwrap().process(x);
            let high = self.high_tilt.as_mut().unwrap().process(x);

            x = low * (1.0 - tone_mix) + high * tone_mix;
            x = self.post_filter.as_mut().unwrap().process(x);

            x *= volume;
            *sample = x;
        }
    }

    fn set_parameter_value(&mut self, name: &str, value: PedalParameterValue) {
        let parameters = self.get_parameters_mut();
        if let Some(parameter) = parameters.get_mut(name) {
            if parameter.is_valid(&value) {
                parameter.value = value;
            } else {
                tracing::warn!(
                    "Attempted to set invalid value for parameter {}: {:?}",
                    name,
                    value
                );
            }
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _message_buffer: &[String],
    ) -> Option<(String, PedalParameterValue)> {
        ui.add(Image::new(include_image!("images/distortion.png")));

        let mut to_change = None;
        let drive_param = self.get_parameters().get("Drive").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Drive",
            drive_param,
            egui::Vec2::new(0.127, 0.085),
            0.35,
            self.id,
        ) {
            to_change = Some(("Drive".to_string(), value));
        }

        let tone_param = self.get_parameters().get("Tone").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Tone",
            tone_param,
            egui::Vec2::new(0.535, 0.085),
            0.35,
            self.id,
        ) {
            to_change = Some(("Tone".to_string(), value));
        }

        let level_param = self.get_parameters().get("Level").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Level",
            level_param,
            egui::Vec2::new(0.325, 0.335),
            0.35,
            self.id,
        ) {
            to_change = Some(("Level".to_string(), value));
        }

        let active_param = self
            .get_parameters()
            .get("Active")
            .unwrap()
            .value
            .as_bool()
            .unwrap();
        if let Some(value) = pedal_switch(ui, active_param, Vec2::new(0.33, 0.72), 0.16) {
            to_change = Some(("Active".to_string(), PedalParameterValue::Bool(value)));
        }

        to_change
    }
}
