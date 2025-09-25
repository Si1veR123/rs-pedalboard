use super::biquad::BiquadFilter;

#[derive(Debug, Clone)]
pub struct MovingBandPass {
    filter: BiquadFilter,
    sample_rate: f32,
    q: f32,
    update_rate: usize,
    counter: usize,
    target_freq: f32,
    current_freq: f32,
    smoothing_per_update: f32,
}

impl MovingBandPass {
    pub fn new(freq: f32, sample_rate: f32, width: f32, update_rate: usize, smoothing_ms: f32) -> Self {
        let q = 1.0 / width;
        // Make smoothing independent of sample rate
        let smoothing_samples = (smoothing_ms * sample_rate) / 1000.0;
        let smoothing_per_sample = if smoothing_samples > 0.0 {
            (0.01f32).powf(1.0 / smoothing_samples)
        } else {
            0.0
        };
        let smoothing_per_update = smoothing_per_sample.powf(update_rate as f32);

        tracing::debug!("Creating MovingBandPass with freq: {}, sample_rate: {}, q: {}, update_rate: {}, smoothing_per_sample: {}", 
            freq, sample_rate, q, update_rate, smoothing_per_sample);

        Self {
            filter: BiquadFilter::band_pass(freq, sample_rate, q),
            sample_rate,
            q,
            update_rate,
            counter: 0,
            target_freq: freq,
            current_freq: freq,
            smoothing_per_update,
        }
    }

    pub fn set_freq(&mut self, freq: f32) {
        self.target_freq = freq;
    }

    pub fn set_width(&mut self, width: f32) {
        self.q = 1.0 / width;
        self.set_band_filter();
    }

    pub fn set_band_filter(&mut self) {
        let old_x = self.filter.x;
        let old_y = self.filter.y;
        self.filter = BiquadFilter::band_pass(self.current_freq, self.sample_rate, self.q);
        self.filter.x = old_x;
        self.filter.y = old_y;
    }

    pub fn process(&mut self, sample: f32) -> f32 {
        if self.counter % self.update_rate == 0 {
            self.current_freq = self.smoothing_per_update * self.current_freq
                + (1.0 - self.smoothing_per_update) * self.target_freq;
            
            self.set_band_filter();
        }
        self.counter = self.counter.wrapping_add(1);
        self.filter.process(sample)
    }
}
