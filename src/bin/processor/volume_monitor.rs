use rs_pedalboard::dsp::ConvertSampleDbLinear;

pub struct PeakVolumeMonitor {
    peak: f32,
}

impl PeakVolumeMonitor {
    pub fn new() -> Self {
        Self { peak: 0.0 }
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

    pub fn take_peak_db(&mut self) -> f32 {
        let peak = self.take_peak();
        f32::linear_to_db(peak)
    }

    pub fn reset(&mut self) {
        self.peak = 0.0;
    }
}
