use crate::dsp_algorithms::variable_delay::VariableDelayLine;
use crate::dsp_algorithms::oscillator::Oscillator;
use std::iter::Iterator;

#[derive(Clone)]
pub struct VariableDelayPhaser {
    pub mix: f32,
    delay: VariableDelayLine,
    min_delay_samples: usize,
    pub feedback: f32,
    pub oscillator: Oscillator,
    sample_rate: f32
}


impl VariableDelayPhaser {
    pub fn new(depth_min_ms: f32, depth_max_ms: f32, mix: f32, oscillator: Oscillator, feedback: f32, sample_rate: f32) -> Self {
        let depth_samples = ((depth_max_ms / 1000.0) * sample_rate) as usize;

        VariableDelayPhaser {
            mix,
            min_delay_samples: ((depth_min_ms / 1000.0) * sample_rate) as usize,
            delay: VariableDelayLine::new(depth_samples),
            feedback,
            oscillator,
            sample_rate
        }
    }

    pub fn process_audio(&mut self, buffer: &mut [f32]) {
        for sample in buffer.iter_mut() {
            let max_depth_samples = self.delay.max_delay().ceil() as usize;

            let oscillator_val = (self.oscillator.next().unwrap() + 1.0) / 2.0;
            let delay_val = (oscillator_val * (max_depth_samples-self.min_delay_samples) as f32) + self.min_delay_samples as f32;

            let delayed_sample = self.delay.get_sample(delay_val);

            // Apply feedback
            self.delay.buffer.pop_front();
            let feedback_sample = delayed_sample * self.feedback + *sample;
            self.delay.buffer.push_back(feedback_sample);

            *sample = self.mix * delayed_sample + (1.0 - self.mix) * *sample;
        }
    }

    pub fn set_rate(&mut self, rate_hz: f32) {
        self.oscillator.set_frequency(rate_hz);
    }

    pub fn set_min_depth(&mut self, depth_ms: f32) {
        self.min_delay_samples = ((depth_ms / 1000.0) * self.sample_rate) as usize;
    }

    pub fn set_max_depth(&mut self, depth_ms: f32) {
        let depth_samples = ((depth_ms / 1000.0) * self.sample_rate) as usize;
        self.delay = VariableDelayLine::new(depth_samples);
    }

    pub fn reset(&mut self) {
        self.delay.buffer.iter_mut().for_each(|s| *s = 0.0);
    }
}