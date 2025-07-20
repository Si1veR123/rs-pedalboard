pub struct PeakNormalizer {
    current_peak: f32,
    decay: f32,
    target_peak: f32,
}

impl PeakNormalizer {
    pub fn new(target_peak: f32, decay_per_second: f32, buffer_size: usize, sample_rate: usize) -> Self {
        let time_per_buffer = buffer_size as f32 / sample_rate as f32;
        let decay = decay_per_second.powf(time_per_buffer);

        Self {
            current_peak: Self::start_peak(decay),
            decay,
            target_peak,
        }
    }

    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for &s in buffer.iter() {
            self.current_peak = self.current_peak.max(s.abs());
        }

        self.current_peak *= self.decay;

        let gain = self.target_peak / self.current_peak;

        for s in buffer.iter_mut() {
            *s *= gain;
        }
    }

    fn start_peak(decay: f32) -> f32 {
        if decay == 1.0 {
            // 'Manual' mode. Peak can't decay so start low.
            1e-3
        } else {
            // Since the peak is able to decay we dont need to start so low
            1e-2
        }
    }

    pub fn reset(&mut self) {
        self.current_peak = Self::start_peak(self.decay);
    }
}
