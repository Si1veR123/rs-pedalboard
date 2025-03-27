use crate::dsp_algorithms::variable_delay::VariableDelay;
use crate::dsp_algorithms::oscillator::{Oscillator, self};
use std::collections::HashMap;
use std::iter::Iterator;
use std::os;
use super::{Pedal, PedalParameter, PedalParameterValue};


pub struct Chorus {
    parameters: HashMap<String, PedalParameter>,
    delay: VariableDelay,
    oscillator: Oscillator,
}

impl Chorus {
    fn oscillator_from_selection(selection: u8, sample_rate: f32, frequency: f32) -> Oscillator{
        match selection {
            0 => Oscillator::Sine(oscillator::Sine::new(sample_rate, frequency)),
            1 => Oscillator::Square(oscillator::Square::new(sample_rate, frequency)),
            2 => Oscillator::Sawtooth(oscillator::Sawtooth::new(sample_rate, frequency)),
            3 => Oscillator::Triangle(oscillator::Triangle::new(sample_rate, frequency)),
            _ => panic!("Invalid selection")
        }
    }

    pub fn new() -> Self {
        // Seconds, probably
        let init_depth_seconds = 0.002;
        let depth_samples = (init_depth_seconds * 48000.0) as usize;
        let init_rate = 1.0;

        let mut parameters = HashMap::new();
        parameters.insert(
            "rate".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(init_rate),
                min: Some(PedalParameterValue::Float(0.01)),
                max: Some(PedalParameterValue::Float(10.0)),
                step: None
            },
        );
        parameters.insert(
            "depth".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(init_depth_seconds),
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
            "oscillator".to_string(),
            PedalParameter {
                value: PedalParameterValue::Selection(0),
                min: Some(PedalParameterValue::Selection(0)),
                max: Some(PedalParameterValue::Selection(3)),
                step: None
            },
        );
        let oscillator = Self::oscillator_from_selection(0, 48000.0, init_rate);

        Chorus { parameters, delay: VariableDelay::new(depth_samples), oscillator }
    }
}

impl Pedal for Chorus {
    fn process_audio(&mut self, buffer: &mut [f32]) {
        let mix = self.parameters.get("mix").unwrap().value.as_float().unwrap();

        for sample in buffer.iter_mut() {
            let oscillator_val = (self.oscillator.next().unwrap() + 1.0) / 2.0;
            let depth_samples = self.delay.buffer.len();
            let delay_val = (oscillator_val * depth_samples as f32) as usize;

            let delayed_sample = self.delay.process_sample(*sample, delay_val);
            *sample = mix * delayed_sample + (1.0 - mix) * *sample;
        }

        //dbg!("{:?}", &buffer);
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }
}
