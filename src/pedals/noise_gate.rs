use std::{collections::HashMap, hash::Hash};
use eframe::egui::{self, include_image};
use serde::{ser::SerializeMap, Deserialize, Serialize};

use crate::{pedals::ui::pedal_switch, unique_time_id};

use super::{
    ui::pedal_knob,
    PedalParameter, PedalParameterValue, PedalTrait,
};

#[derive(Clone)]
pub struct NoiseGate {
    parameters: HashMap<String, PedalParameter>,
    gain: f32,
    level: f32,
    sample_rate: Option<f32>,
    id: u32,
}

impl Serialize for NoiseGate {
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

impl<'de> Deserialize<'de> for NoiseGate {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct NoiseGateData {
            id: u32,
            parameters: HashMap<String, PedalParameter>,
        }
        let helper = NoiseGateData::deserialize(deserializer)?;
        
        Ok(NoiseGate {
            parameters: helper.parameters,
            gain: 1.0,
            level: 0.0,
            sample_rate: None,
            id: helper.id
        })
    }
}

impl NoiseGate {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();

        parameters.insert(
            "Threshold Db".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(-50.0),
                min: Some(PedalParameterValue::Float(-70.0)),
                max: Some(PedalParameterValue::Float(0.0)),
                step: None,
            },
        );

        parameters.insert(
            "Reduction".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(10.0),
                min: Some(PedalParameterValue::Float(1.0)),
                max: Some(PedalParameterValue::Float(20.0)),
                step: None,
            },
        );

        parameters.insert(
            "Attack".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(5.0),
                min: Some(PedalParameterValue::Float(1.0)),
                max: Some(PedalParameterValue::Float(50.0)),
                step: None,
            },
        );

        parameters.insert(
            "Release".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(50.0),
                min: Some(PedalParameterValue::Float(1.0)),
                max: Some(PedalParameterValue::Float(50.0)),
                step: None,
            },
        );

        parameters.insert(
            "Dry/Wet".to_string(),
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

        Self {
            parameters,
            gain: 1.0,
            level: 0.0,
            sample_rate: None,
            id: unique_time_id()
        }
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned
    }
}

impl Hash for NoiseGate {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl PedalTrait for NoiseGate {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn set_config(&mut self,_buffer_size:usize, sample_rate:u32) {
        self.sample_rate = Some(sample_rate as f32);
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        if self.sample_rate.is_none() {
            tracing::warn!("NoiseGate: Sample rate not set. Call set_config first.");
            return;
        }

        let threshold_db = self.parameters["Threshold Db"].value.as_float().unwrap();
        let reduction_ratio = self.parameters["Reduction"].value.as_float().unwrap();
        let attack_ms = self.parameters["Attack"].value.as_float().unwrap();
        let release_ms = self.parameters["Release"].value.as_float().unwrap();
        let dry_wet = self.parameters["Dry/Wet"].value.as_float().unwrap();

        // per sample smoothing coefficients (sample rate independent)
        let attack_coeff = (-1.0 / ((attack_ms / 1000.0) * self.sample_rate.unwrap())).exp();
        let release_coeff = (-1.0 / ((release_ms / 1000.0) * self.sample_rate.unwrap())).exp();

        let alpha = 0.99; // Smoothing for level estimation (RMS approximation)
        let mut level = self.level;

        for sample in buffer.iter_mut() {
            let x = *sample;

            // Estimate signal power (RMS-like)
            level = alpha * level + (1.0 - alpha) * (x * x);
            let power_db = 10.0 * level.max(1e-12).log10();

            // Compute gain target based on threshold and ratio
            let mut gain_target = 1.0;
            if power_db < threshold_db {
                let diff = threshold_db - power_db;
                let reduction_db = diff * reduction_ratio;
                gain_target = 10f32.powf(-reduction_db / 20.0);
            }

            // Smoothly approach gain_target using attack/release
            if gain_target > self.gain {
                self.gain = attack_coeff * (self.gain - gain_target) + gain_target;
            } else {
                self.gain = release_coeff * (self.gain - gain_target) + gain_target;
            }

            *sample *= self.gain * dry_wet + x * (1.0 - dry_wet);
        }

        self.level = level;
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String,PedalParameterValue)> {
        ui.add(egui::Image::new(include_image!("images/noise_gate.png")));

        let mut to_change = None;

        let threshold_db_param = self.get_parameters().get("Threshold Db").unwrap();
        if let Some(value) = pedal_knob(ui, "", threshold_db_param, egui::Vec2::new(0.08, 0.03), 0.35) {
            to_change = Some(("Threshold Db".to_string(), value));
        }

        let reduction_param = self.get_parameters().get("Reduction").unwrap();
        if let Some(value) = pedal_knob(ui, "", reduction_param, egui::Vec2::new(0.57, 0.03), 0.35) {
            to_change = Some(("Reduction".to_string(), value));
        }

        let attack_param = self.get_parameters().get("Attack").unwrap();
        if let Some(value) = pedal_knob(ui, "", attack_param, egui::Vec2::new(0.08, 0.34), 0.35) {
            to_change = Some(("Attack".to_string(), value));
        }

        let release_param = self.get_parameters().get("Release").unwrap();
        if let Some(value) = pedal_knob(ui, "", release_param, egui::Vec2::new(0.57, 0.34), 0.35) {
            to_change = Some(("Release".to_string(), value));
        }

        let active_param = self.get_parameters().get("Active").unwrap().value.as_bool().unwrap();
        if let Some(value) = pedal_switch(ui, active_param, egui::Vec2::new(0.33, 0.72), 0.16) {
            to_change = Some(("Active".to_string(), PedalParameterValue::Bool(value)));
        }

        to_change
    }
}
