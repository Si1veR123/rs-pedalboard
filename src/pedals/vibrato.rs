use std::collections::HashMap;
use super::{PedalTrait, PedalParameter, PedalParameterValue};
use crate::dsp_algorithms::{oscillator::{Oscillator, Sine}, variable_delay::VariableDelayLine};

#[derive(Clone)]
pub struct Vibrato {
    sample_rate: f32,
    oscillator: Oscillator,
    delay_line: VariableDelayLine,
    delay_line_pos: usize,
    max_delay_samples: usize,
    parameters: HashMap<String, PedalParameter>,
    oscillator_phase_increment: f32,
}

impl Vibrato {
    pub fn new(sample_rate: usize) -> Self {
        let sample_rate = sample_rate as f32;
        let max_delay_ms = 15.0; // max delay depth in ms, must cover max expected depth + padding
        let max_delay_samples = (sample_rate * max_delay_ms / 1000.0).ceil() as usize;

        let oscillator = Oscillator::Sine(Sine::new(sample_rate, 5.0)); // default freq 5 Hz

        let mut parameters = HashMap::new();
        parameters.insert(
            "depth".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(5.0), // default depth in ms
                min: Some(PedalParameterValue::Float(0.1)),
                max: Some(PedalParameterValue::Float(15.0)),
                step: Some(0.1),
            },
        );
        parameters.insert(
            "rate".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(5.0), // frequency in Hz
                min: Some(PedalParameterValue::Float(0.1)),
                max: Some(PedalParameterValue::Float(10.0)),
                step: Some(0.1),
            },
        );

        Self {
            sample_rate,
            oscillator,
            delay_line: crate::VariableDelayLine::new(max_delay_samples + 2), // add 2 for safety margin
            delay_line_pos: 0,
            max_delay_samples,
            parameters,
            oscillator_phase_increment: 0.0,
        }
    }

    fn ms_to_samples(&self, ms: f32) -> f32 {
        ms * self.sample_rate / 1000.0
    }
}

impl PedalTrait for Vibrato {
    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        // Get parameters
        let depth_ms = self.parameters.get("depth").unwrap().value.as_float().unwrap();
        let rate_hz = self.parameters.get("rate").unwrap().value.as_float().unwrap();

        // Update oscillator frequency if changed
        if (self.oscillator.get_frequency() - rate_hz).abs() > f32::EPSILON {
            self.oscillator.set_frequency(rate_hz);
        }

        // Compute padding: max(1 ms, 20% of depth)
        let padding_ms = depth_ms * 0.2_f32.max(1.0);

        // Clamp padding to at least 1 ms
        let padding_ms = padding_ms.max(1.0);

        // Convert to samples
        let padding_samples = self.ms_to_samples(padding_ms);
        let depth_samples = self.ms_to_samples(depth_ms);

        for sample in buffer.iter_mut() {
            // Write current input to delay line
            if self.delay_line.buffer.len() < self.max_delay_samples + 2 {
                self.delay_line.buffer.push_back(*sample);
            } else {
                self.delay_line.buffer[self.delay_line_pos] = *sample;
            }

            // Get modulator output in [-1,1]
            let mod_val = self.oscillator.next().unwrap_or(0.0);

            // Calculate modulated delay time in samples: delay = padding + depth * (1 + mod_val)
            // mod_val in [-1,1], so delay ∈ [padding, padding + 2*depth]
            let delay_time_samples = padding_samples + depth_samples * (1.0 + mod_val);

            // Compute delay read index (circular buffer)
            let delay_samples_f = delay_time_samples;
            let read_pos_f = (self.delay_line_pos as f32 + self.delay_line.buffer.len() as f32)
                - delay_samples_f;

            // Wrap around buffer length
            let buffer_len = self.delay_line.buffer.len() as f32;
            let read_pos_f = if read_pos_f < 0.0 {
                read_pos_f + buffer_len
            } else if read_pos_f >= buffer_len {
                read_pos_f - buffer_len
            } else {
                read_pos_f
            };

            let read_pos_i = read_pos_f.floor() as usize % self.delay_line.buffer.len();
            let next_pos_i = (read_pos_i + 1) % self.delay_line.buffer.len();
            let frac = read_pos_f - read_pos_f.floor();

            // Linear interpolation between samples to reduce artifacts
            let delayed_sample = (1.0 - frac) * self.delay_line.buffer[read_pos_i]
                + frac * self.delay_line.buffer[next_pos_i];

            // Output the delayed sample (pure vibrato: fully wet)
            *sample = delayed_sample;

            // Advance delay line write position circularly
            self.delay_line_pos = (self.delay_line_pos + 1) % self.delay_line.buffer.len();
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }
}