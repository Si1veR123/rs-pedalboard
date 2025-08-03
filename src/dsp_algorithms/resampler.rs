use crate::dsp_algorithms::biquad::BiquadFilter;

pub struct PolyphaseIIR2xResampler {
    up_filter: BiquadFilter,
    down_filter: BiquadFilter,
    last_input: f32,
}

impl PolyphaseIIR2xResampler {
    pub fn new(sample_rate: f32, q: f32) -> Self {
        let cutoff_freq = 0.45 * sample_rate;

        let up_filter = BiquadFilter::low_pass(cutoff_freq, sample_rate * 2.0, q);
        let down_filter = BiquadFilter::low_pass(cutoff_freq, sample_rate, q);

        Self {
            up_filter,
            down_filter,
            last_input: 0.0,
        }
    }

    /// 2× upsampling: input.len() → output.len() == 2 × input.len()
    pub fn upsample(&mut self, input: &[f32], output: &mut [f32]) {
        assert_eq!(output.len(), input.len() * 2);

        for (i, &x) in input.iter().enumerate() {
            output[2 * i] = self.last_input;

            let midpoint = 0.5 * (self.last_input + x);
            output[2 * i + 1] = self.up_filter.process(midpoint);

            self.last_input = x;
        }
    }

    pub fn downsample(&mut self, input: &[f32], output: &mut [f32]) {
        assert_eq!(input.len(), output.len() * 2);

        for (i, chunk) in input.chunks_exact(2).enumerate() {
            let _ = self.down_filter.process(chunk[0]);
            output[i] = self.down_filter.process(chunk[1]);
        }
    }
}
