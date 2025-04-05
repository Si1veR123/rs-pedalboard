use std::collections::HashMap;
use super::PedalTrait;
use super::PedalParameter;
use super::PedalParameterValue;

use serde::{Serialize, Deserialize};
use signalsmith_stretch::Stretch;


pub struct PitchShift {
    parameters: HashMap<String, PedalParameter>,
    signalsmith_stretch: Stretch,
    output_buffer: Vec<f32>,
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

        let init_block_size = 128;
        let init_semitones = -1.0;
        let init_speed = 0;
        let init_tonality_limit = 10000.0;

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
                value: PedalParameterValue::Selection(init_block_size),
                min: Some(PedalParameterValue::Selection(10)),
                max: Some(PedalParameterValue::Selection(180)),
                step: None,
            }
        );

        // Whether to use 1/4 (slow,0) or 1/2 (faster,1) hop size
        parameters.insert(
            "speed".to_string(),
            PedalParameter {
                value: PedalParameterValue::Selection(init_speed),
                min: Some(PedalParameterValue::Selection(0)),
                max: Some(PedalParameterValue::Selection(1)),
                step: None
            }
        );

        parameters.insert(
            "tonality_limit".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(init_tonality_limit),
                min: Some(PedalParameterValue::Float(1000.0)),
                max: Some(PedalParameterValue::Float(20000.0)),
                step: Some(PedalParameterValue::Float(100.0)),
            }
        );

        let stretch = Self::stretch_from_parameters(&parameters);
        PitchShift { parameters, signalsmith_stretch: stretch, output_buffer: Vec::new() }
    }

    pub fn stretch_from_parameters(parameters: &HashMap<String, PedalParameter>) -> Stretch {
        let block_size = parameters.get("block_size").unwrap().value.as_selection().unwrap();
        let semitones = parameters.get("semitones").unwrap().value.as_float().unwrap();
        let speed = parameters.get("speed").unwrap().value.as_selection().unwrap();
        let tonality_limit = parameters.get("tonality_limit").unwrap().value.as_float().unwrap();

        let interval = block_size / if speed == 0 { 4 } else { 2 };
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

    /// TODO: Update the stretch object when parameters change
}
