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
            current_peak: 1e-3,
            decay,
            target_peak,
        }
    }

    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for &s in buffer.iter() {
            self.current_peak = self.current_peak.max(s.abs());
        }

        self.current_peak *= self.decay;
        log::debug!("Current peak: {}", self.current_peak);

        let gain = self.target_peak / self.current_peak;

        for s in buffer.iter_mut() {
            *s *= gain;
        }
    }

    pub fn reset(&mut self) {
        self.current_peak = 1e-3;
    }
}
