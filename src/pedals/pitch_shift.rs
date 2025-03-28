use std::collections::HashMap;
use super::PedalTrait;
use super::PedalParameter;
use super::PedalParameterValue;
use crate::dsp_algorithms::phase_vocoder::PhaseVocoder;

use serde::{Serialize, Deserialize};

pub struct PitchShift {
    parameters: HashMap<String, PedalParameter>,
    phase_vocoder: PhaseVocoder,
    output_buffer: Option<Vec<f32>>,
}

impl Serialize for PitchShift {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let pitch = self.parameters.get("pitch").unwrap().value.as_float().unwrap();
        serializer.serialize_f32(pitch)
    }
}

impl<'a> Deserialize<'a> for PitchShift {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        let pitch = f32::deserialize(deserializer)?;
        let mut pedal = PitchShift::new();
        pedal.set_parameter_value("pitch", PedalParameterValue::Float(pitch));
        Ok(pedal)
    }
}

impl PitchShift {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();
        parameters.insert(
            "pitch".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.5),
                min: Some(PedalParameterValue::Float(0.5)),
                max: Some(PedalParameterValue::Float(2.0)),
                step: None
            },
        );

        let phase_vocoder = PhaseVocoder::new(128, 0.5);

        PitchShift { parameters, phase_vocoder, output_buffer: None }
    }
}

impl PedalTrait for PitchShift {
    fn process_audio(&mut self, buffer: &mut [f32]) {
        if self.output_buffer.is_none() {
            self.output_buffer = Some(vec![0.0; buffer.len()]);
        }
        
        self.phase_vocoder.process_buffer(buffer, self.output_buffer.as_mut().unwrap());
        buffer.copy_from_slice(&self.output_buffer.as_ref().unwrap()[..buffer.len()]);
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
                if name == "pitch" {
                    let pitch = value.as_float().unwrap();
                    self.phase_vocoder = PhaseVocoder::new(128, pitch);
                    // Borrow checker stuff
                    self.get_parameters_mut().get_mut(name).unwrap().value = PedalParameterValue::Float(pitch);
                } else {
                    parameter.value = value;
                }

                
            }
        }
    }
}
