use std::collections::HashMap;
use std::hash::Hash;
use super::PedalTrait;
use super::PedalParameter;
use super::PedalParameterValue;
use super::ui::pedal_knob;

use serde::{Serialize, Deserialize};
use signalsmith_stretch::Stretch;


pub struct PitchShift {
    parameters: HashMap<String, PedalParameter>,
    signalsmith_stretch: Stretch,
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

        Ok(PitchShift {
            parameters,
            signalsmith_stretch: stretch,
            output_buffer: Vec::new(),
        })
    }
}

impl PitchShift {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();

        let init_block_size = 2048;
        let init_semitones = -1.0;
        let init_speed = 0;
        let init_tonality_limit = 0.5;

        parameters.insert(
            "semitones".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(init_semitones),
                min: Some(PedalParameterValue::Float(-24.0)),
                max: Some(PedalParameterValue::Float(24.0)),
                step: Some(PedalParameterValue::Float(1.0)),
            }
        );

        parameters.insert(
            "block_size".to_string(),
            PedalParameter {
                value: PedalParameterValue::Int(init_block_size),
                min: Some(PedalParameterValue::Int(128)),
                max: Some(PedalParameterValue::Int(4096)),
                step: None,
            }
        );

        // Whether to use 1/8 (slow,0) or 1/4 (faster,1) hop size
        parameters.insert(
            "speed".to_string(),
            PedalParameter {
                value: PedalParameterValue::Int(init_speed),
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

        let stretch = Self::stretch_from_parameters(&parameters);
        PitchShift { parameters, signalsmith_stretch: stretch, output_buffer: Vec::new() }
    }

    pub fn stretch_from_parameters(parameters: &HashMap<String, PedalParameter>) -> Stretch {
        let block_size = parameters.get("block_size").unwrap().value.as_int().unwrap();
        let semitones = parameters.get("semitones").unwrap().value.as_float().unwrap();
        let speed = parameters.get("speed").unwrap().value.as_int().unwrap();
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
                parameter.value = value;
                self.signalsmith_stretch = Self::stretch_from_parameters(parameters);
            }
        }
    }

    fn ui(&mut self, ui: &mut eframe::egui::Ui) -> Option<(String, PedalParameterValue)> {
        let mut to_change = None;
        for (parameter_name, parameter) in self.get_parameters().iter() {
            if let Some(value) = pedal_knob(ui, parameter_name, parameter) {
                to_change = Some((parameter_name.clone(), value));
            }
        }

        to_change
    }
}
