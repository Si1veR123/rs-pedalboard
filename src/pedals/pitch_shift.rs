use std::collections::HashMap;
use std::hash::Hash;
use crate::dsp_algorithms::biquad::BiquadFilter;
use crate::dsp_algorithms::eq::Equalizer;
use crate::pedals::ui::pedal_switch;

use super::PedalTrait;
use super::PedalParameter;
use super::PedalParameterValue;
use super::ui::pedal_knob;

use eframe::egui::{include_image, self};
use serde::{Serialize, Deserialize};
use signalsmith_stretch::Stretch;


pub struct PitchShift {
    parameters: HashMap<String, PedalParameter>,

    // Server only
    signalsmith_stretch: Option<Stretch>,
    // (eq, sample rate)
    eq: Option<(Equalizer, u32)>,
    output_buffer: Vec<f32>,
}

impl Hash for PitchShift {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.parameters.values().map(|p| &p.value).for_each(|v| v.hash(state));
    }
}

impl Clone for PitchShift {
    fn clone(&self) -> Self {
        if let Some((_eq, sample_rate)) = &self.eq {
            PitchShift {
                parameters: self.parameters.clone(),
                signalsmith_stretch: Some(Self::stretch_from_parameters(&self.parameters, *sample_rate as f32)),
                eq: self.eq.clone(),
                output_buffer: self.output_buffer.clone(),
            }
        } else {
            PitchShift {
                parameters: self.parameters.clone(),
                signalsmith_stretch: None,
                eq: None,
                output_buffer: self.output_buffer.clone(),
            }
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

        Ok(PitchShift {
            parameters,
            signalsmith_stretch: None,
            eq: None,
            output_buffer: Vec::new(),
        })
    }
}

impl PitchShift {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();

        let init_block_size = 3074 / 128;
        let init_semitones = 0;
        let init_tonality_limit = 4000.0;

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

        parameters.insert(
            "tonality_limit".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(init_tonality_limit),
                min: Some(PedalParameterValue::Float(100.0)),
                max: Some(PedalParameterValue::Float(6000.0)),
                step: None,
            }
        );

        parameters.insert(
            "presence".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(10.0)),
                step: None,
            }
        );

        parameters.insert(
            "active".to_string(),
            PedalParameter {
                value: PedalParameterValue::Bool(true),
                min: None,
                max: None,
                step: None,
            },
        );

        PitchShift { parameters, signalsmith_stretch: None, eq: None, output_buffer: Vec::new() }
    }

    pub fn eq_from_presence(presence: f32, sample_rate: f32) -> Equalizer {
        let biquad = BiquadFilter::peaking(3900.0, sample_rate, 1.0, presence);
        Equalizer::new(vec![biquad])
    }

    pub fn stretch_from_parameters(parameters: &HashMap<String, PedalParameter>, sample_rate: f32) -> Stretch {
        let block_size = parameters.get("block_size").unwrap().value.as_int().unwrap() as usize * 128;
        let semitones = parameters.get("semitones").unwrap().value.as_int().unwrap();
        let tonality_limit_hz = parameters.get("tonality_limit").unwrap().value.as_float().unwrap();
        let tonality_limit = tonality_limit_hz / sample_rate;

        let mut stretch = Stretch::new(1, block_size, block_size/4);
        stretch.set_transpose_factor_semitones(semitones as f32, Some(tonality_limit));

        stretch
    }
}


impl PedalTrait for PitchShift {
    fn set_config(&mut self,_buffer_size:usize, sample_rate:u32) {
        // Set eq
        let eq = Self::eq_from_presence(self.parameters.get("presence").unwrap().value.as_float().unwrap(), sample_rate as f32);
        self.eq = Some((eq, sample_rate));
        // Set stretch
        self.signalsmith_stretch = Some(Self::stretch_from_parameters(&self.parameters, sample_rate as f32));
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        if self.eq.is_none() || self.signalsmith_stretch.is_none() {
            log::warn!("PitchShift: Call set_config before processing.");
            return;
        }

        if self.output_buffer.len() != buffer.len() {
            self.output_buffer.resize(buffer.len(), 0.0);
        }

        self.signalsmith_stretch.as_mut().unwrap().process(buffer.as_ref(), &mut self.output_buffer);

        for sample in self.output_buffer.iter_mut() {
            *sample = self.eq.as_mut().unwrap().0.process(*sample);
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
                    if let Some((_eq, sample_rate)) = &self.eq {
                        self.eq = Some((Self::eq_from_presence(presence, *sample_rate as f32), *sample_rate));
                    }
                } else {
                    parameter.value = value;
                    if let Some((_eq, sample_rate)) = &self.eq {
                        self.signalsmith_stretch = Some(Self::stretch_from_parameters(&self.parameters, *sample_rate as f32));
                    }
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String, PedalParameterValue)> {
        ui.add(egui::Image::new(include_image!("images/pitch_shift.png")));

        let mut to_change = None;
        let semitones_param = self.get_parameters().get("semitones").unwrap();
        if let Some(value) = pedal_knob(ui, "", semitones_param, eframe::egui::Vec2::new(0.05, 0.022), 0.3) {
            to_change = Some(("semitones".to_string(), value));
        }

        let block_size_param = self.get_parameters().get("block_size").unwrap();
        if let Some(value) = pedal_knob(ui, "", block_size_param, eframe::egui::Vec2::new(0.05, 0.171), 0.3) {
            to_change =  Some(("block_size".to_string(), value));
        }

        let tonality_limit_param = self.get_parameters().get("tonality_limit").unwrap();
        if let Some(value) = pedal_knob(ui, "", tonality_limit_param, eframe::egui::Vec2::new(0.05, 0.32), 0.3) {
            to_change =  Some(("tonality_limit".to_string(), value));
        }

        let presence_param = self.get_parameters().get("presence").unwrap();
        if let Some(value) = pedal_knob(ui, "", presence_param, eframe::egui::Vec2::new(0.05, 0.469), 0.3) {
            to_change =  Some(("presence".to_string(), value));
        }

        let active_param = self.get_parameters().get("active").unwrap().value.as_bool().unwrap();
        if let Some(value) = pedal_switch(ui, active_param, egui::Vec2::new(0.33, 0.72), 0.16) {
            to_change = Some(("active".to_string(), PedalParameterValue::Bool(value)));
        }

        to_change
    }
}
