use crate::dsp_algorithms::variable_delay::VariableDelay;
use crate::dsp_algorithms::oscillator::{Oscillator, self};
use std::iter::Iterator;

#[derive(Clone)]
pub struct VariableDelayPhaser {
    pub mix: f32,
    delay: VariableDelay,
    min_delay_samples: usize,
    oscillator: Oscillator,
}


impl VariableDelayPhaser {
    fn oscillator_from_selection(selection: u16, sample_rate: f32, frequency: f32) -> Oscillator {
        match selection {
            0 => Oscillator::Sine(oscillator::Sine::new(sample_rate, frequency)),
            1 => Oscillator::Square(oscillator::Square::new(sample_rate, frequency)),
            2 => Oscillator::Sawtooth(oscillator::Sawtooth::new(sample_rate, frequency)),
            3 => Oscillator::Triangle(oscillator::Triangle::new(sample_rate, frequency)),
            _ => panic!("Invalid selection")
        }
    }

    pub fn new(depth_min_ms: f32, depth_max_ms: f32, rate_hz: f32, mix: f32, oscillator_selection: usize) -> Self {
        let depth_samples = ((depth_max_ms / 1000.0) * 48000.0) as usize;
        VariableDelayPhaser {
            mix,
            min_delay_samples: ((depth_min_ms / 1000.0) * 48000.0) as usize,
            delay: VariableDelay::new(depth_samples),
            oscillator: Self::oscillator_from_selection(oscillator_selection as u16, 48000.0, rate_hz)
        }
    }

    pub fn process_audio(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            let max_depth_samples = self.delay.buffer.len();

            let oscillator_val = (self.oscillator.next().unwrap() + 1.0) / 2.0;
            let delay_val = (oscillator_val * (max_depth_samples-self.min_delay_samples) as f32) as usize + self.min_delay_samples;

            let delayed_sample = self.delay.process_sample(*sample, delay_val);
            *sample = self.mix * delayed_sample + (1.0 - self.mix) * *sample;
        }
    }

    pub fn set_rate(&mut self, rate_hz: f32) {
        self.oscillator.set_frequency(rate_hz);
    }

    pub fn set_min_depth(&mut self, depth_ms: f32) {
        self.min_delay_samples = ((depth_ms / 1000.0) * 48000.0) as usize;
    }

    pub fn set_max_depth(&mut self, depth_ms: f32) {
        let depth_samples = ((depth_ms / 1000.0) * 48000.0) as usize;
        self.delay = VariableDelay::new(depth_samples);
    }

    pub fn set_oscillator(&mut self, selection: u16) {
        self.oscillator = Self::oscillator_from_selection(selection, 48000.0, self.oscillator.get_frequency());
    }
}