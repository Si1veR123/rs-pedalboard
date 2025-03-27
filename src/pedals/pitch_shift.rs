use std::collections::HashMap;
use super::Pedal;
use super::PedalParameter;
use super::PedalParameterValue;
use crate::dsp_algorithms::phase_vocoder::PhaseVocoder;

pub struct PitchShift {
    parameters: HashMap<String, PedalParameter>,
    phase_vocoder: PhaseVocoder,
    output_buffer: Option<Vec<f32>>,
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

        let phase_vocoder = PhaseVocoder::new(60, 1.0);

        PitchShift { parameters, phase_vocoder, output_buffer: None }
    }
}

impl Pedal for PitchShift {
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
}
