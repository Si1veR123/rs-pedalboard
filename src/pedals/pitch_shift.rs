use std::collections::HashMap;
use std::hash::Hash;
use crate::dsp_algorithms::biquad::BiquadFilter;
use crate::dsp_algorithms::eq::Equalizer;

use super::PedalTrait;
use super::PedalParameter;
use super::PedalParameterValue;
use super::ui::{pedal_knob, pedal_label_rect};

use eframe::egui::Color32;
use eframe::egui::{include_image, self};
use serde::{Serialize, Deserialize};
use signalsmith_stretch::Stretch;


pub struct PitchShift {
    parameters: HashMap<String, PedalParameter>,
    signalsmith_stretch: Stretch,
    eq: Equalizer,
    output_buffer: Vec<f32>,
}

impl Hash for PitchShift {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.parameters.values().map(|p| &p.value).for_each(|v| v.hash(state));
    }
}

impl Clone for PitchShift {
    fn clone(&self) -> Self {
        let cloned_signalsmith = Self::stretch_from_parameters(&self.parameters);

        PitchShift {
            parameters: self.parameters.clone(),
            signalsmith_stretch: cloned_signalsmith,
            eq: self.eq.clone(),
            output_buffer: self.output_buffer.clone(),
        }
    }
}

impl Serialize for PitchShift {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_map(self.parameters.iter())
    }
}

impl<'a> Deserialize<'a> for PitchShift {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        let parameters: HashMap<String, PedalParameter> = HashMap::deserialize(deserializer)?;
        
        let stretch = Self::stretch_from_parameters(&parameters);
        let eq = PitchShift::eq_from_presence(parameters.get("presence").unwrap().value.as_float().unwrap());

        Ok(PitchShift {
            parameters,
            signalsmith_stretch: stretch,
            eq,
            output_buffer: Vec::new(),
        })
    }
}

impl PitchShift {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();

        let init_block_size = 3074 / 128;
        let init_semitones = 0;
        let init_hop = 0;
        let init_tonality_limit = 0.5;

        parameters.insert(
            "semitones".to_string(),
            PedalParameter {
                value: PedalParameterValue::Int(init_semitones),
                min: Some(PedalParameterValue::Int(-12)),
                max: Some(PedalParameterValue::Int(12)),
                step: None,
            }
        );

        // Multiples of 128
        parameters.insert(
            "block_size".to_string(),
            PedalParameter {
                value: PedalParameterValue::Int(init_block_size),
                min: Some(PedalParameterValue::Int(1)),
                max: Some(PedalParameterValue::Int(4096 / 128)),
                step: None,
            }
        );

        // Whether to use 1/8 (slow,0) or 1/4 (faster,1) hop size
        parameters.insert(
            "hop".to_string(),
            PedalParameter {
                value: PedalParameterValue::Int(init_hop),
                min: Some(PedalParameterValue::Int(0)),
                max: Some(PedalParameterValue::Int(1)),
                step: None
            }
        );

        parameters.insert(
            "tonality_limit".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(init_tonality_limit),
                min: Some(PedalParameterValue::Float(0.001)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: Some(PedalParameterValue::Float(0.001)),
            }
        );

        parameters.insert(
            "presence".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(30.0)),
                step: None,
            }
        );

        let stretch = Self::stretch_from_parameters(&parameters);
        let eq = Self::eq_from_presence(0.0);
        PitchShift { parameters, signalsmith_stretch: stretch, eq, output_buffer: Vec::new() }
    }

    pub fn eq_from_presence(presence: f32) -> Equalizer {
        let biquad = BiquadFilter::high_shelf(2900.0, 48000.0, 0.707, presence);
        Equalizer::new(vec![biquad])
    }

    pub fn stretch_from_parameters(parameters: &HashMap<String, PedalParameter>) -> Stretch {
        let block_size = parameters.get("block_size").unwrap().value.as_int().unwrap() * 128;
        let semitones = parameters.get("semitones").unwrap().value.as_int().unwrap();
        let speed = parameters.get("hop").unwrap().value.as_int().unwrap();
        let tonality_limit = parameters.get("tonality_limit").unwrap().value.as_float().unwrap();

        let interval = block_size / if speed == 0 { 8 } else { 4 };
        let mut stretch = Stretch::new(1, block_size as usize, interval as usize);
        stretch.set_transpose_factor_semitones(semitones as f32, Some(tonality_limit));

        stretch
    }
}


impl PedalTrait for PitchShift {
    fn process_audio(&mut self, buffer: &mut [f32]) {
        if self.output_buffer.len() != buffer.len() {
            self.output_buffer.resize(buffer.len(), 0.0);
        }

        self.signalsmith_stretch.process(buffer.as_ref(), &mut self.output_buffer);

        for sample in self.output_buffer.iter_mut() {
            *sample = self.eq.process(*sample);
        }

        buffer.copy_from_slice(&self.output_buffer);
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
                if name == "presence" {
                    let presence = value.as_float().unwrap();
                    parameter.value = value;
                    self.eq = Self::eq_from_presence(presence);
                } else {
                    parameter.value = value;
                    self.signalsmith_stretch = Self::stretch_from_parameters(parameters);
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut eframe::egui::Ui) -> Option<(String, PedalParameterValue)> {
        ui.add(egui::Image::new(include_image!("images/pedal_base.png")));

        let mut to_change = None;
        let semitones_param = self.get_parameters().get("semitones").unwrap();
        if let Some(value) = pedal_knob(ui, "Semitones", semitones_param, eframe::egui::Vec2::new(0.1, 0.02), 0.25, Color32::BLACK) {
            to_change = Some(("semitones".to_string(), value));
        }

        let block_size_param = self.get_parameters().get("block_size").unwrap();
        if let Some(value) = pedal_knob(ui, "Block Size", block_size_param, eframe::egui::Vec2::new(0.38, 0.02), 0.25, Color32::BLACK) {
            to_change =  Some(("block_size".to_string(), value));
        }

        let speed_param = self.get_parameters().get("hop").unwrap();
        if let Some(value) = pedal_knob(ui, "Hop", speed_param, eframe::egui::Vec2::new(0.67, 0.02), 0.25, Color32::BLACK) {
            to_change =  Some(("hop".to_string(), value));
        }

        let tonality_limit_param = self.get_parameters().get("tonality_limit").unwrap();
        if let Some(value) = pedal_knob(ui, "Tonality Limit", tonality_limit_param, eframe::egui::Vec2::new(0.2, 0.22), 0.25, Color32::BLACK) {
            to_change =  Some(("tonality_limit".to_string(), value));
        }

        let presence_param = self.get_parameters().get("presence").unwrap();
        if let Some(value) = pedal_knob(ui, "Presence", presence_param, eframe::egui::Vec2::new(0.55, 0.22), 0.25, Color32::BLACK) {
            to_change =  Some(("presence".to_string(), value));
        }

        let pedal_rect = ui.max_rect();
        ui.put(pedal_label_rect(pedal_rect), egui::Label::new(
            egui::RichText::new("Pitch Shift")
                .color(egui::Color32::from_black_alpha(200))
        ));

        to_change
    }
}
