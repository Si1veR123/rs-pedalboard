pub struct PeakVolumeMonitor {
    peak: f32
}

impl PeakVolumeMonitor {
    pub fn new() -> Self {
        Self {
            peak: 0.0
        }
    }

    pub fn add_samples(&mut self, samples: &[f32]) {
        for &sample in samples {
            let abs_sample = sample.abs();
            if abs_sample > self.peak {
                self.peak = abs_sample;
            }
        }
    }

    pub fn take_peak(&mut self) -> f32 {
        let peak = self.peak;
        self.reset();
        peak
    }

    pub fn reset(&mut self) {
        self.peak = 0.0;
    }
}
