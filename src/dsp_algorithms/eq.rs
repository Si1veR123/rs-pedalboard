use egui_plot::PlotPoint;
use num_complex::Complex64;

use super::biquad::BiquadFilter;


pub struct DynamicEqualizerBuilder {
    pub sample_rate: f32,
    pub bands: Vec<(f32, f32, f32)>,
    pub upper_shelf: bool,
    pub lower_shelf: bool,
}

impl DynamicEqualizerBuilder {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            bands: vec![],
            sample_rate,
            upper_shelf: false,
            lower_shelf: false,
        }
    }

    pub fn with_band(mut self, f: f32, q: f32, gain: f32) -> Self {
        self.bands.push((f, q, gain));
        self
    }

    pub fn with_bands(mut self, bands: Vec<(f32, f32, f32)>) -> Self {
        self.bands = bands;
        self
    }

    /// If multiple bands, upper shelf is applied to the last band
    pub fn with_upper_shelf(mut self) -> Self {
        self.upper_shelf = true;
        self
    }

    /// If single band, lower shelf takes precedence over upper shelf
    /// If multiple bands, lower shelf is applied to the first band
    pub fn with_lower_shelf(mut self) -> Self {
        self.lower_shelf = true;
        self
    }

    pub fn build(self) -> Equalizer {
        let mut biquads = Vec::with_capacity(self.bands.len());
        let last_index = self.bands.len() - 1;
        for (i, (f, bandwidth, gain)) in self.bands.into_iter().enumerate() {
            let bq;

            if self.lower_shelf && i == 0 {
                bq = BiquadFilter::low_shelf(f, self.sample_rate, 1.0/bandwidth, gain);
            } else if self.upper_shelf && i == last_index {
                bq = BiquadFilter::high_shelf(f, self.sample_rate, 1.0/bandwidth, gain);
            } else {
                bq = BiquadFilter::peaking(f, self.sample_rate, 1.0/bandwidth, gain);
            }

            biquads.push(bq);
        }
        Equalizer {
            biquads,
        }
    }
}

pub struct GraphicEqualizerBuilder<const N: usize> {
    pub sample_rate: f32,
    pub bands: [f32; N],
    pub gains: [f32; N],
    pub bandwidth: [f32; N],
    pub upper_shelf: bool,
    pub lower_shelf: bool,
}

impl<const N: usize> GraphicEqualizerBuilder<N> {
    pub fn new(sample_rate: f32) -> Self {
        GraphicEqualizerBuilder {
            bands: Self::default_bands(),
            gains: [0.0; N],
            bandwidth: [1.0; N],
            sample_rate,
            upper_shelf: false,
            lower_shelf: false,
        }
    }

    fn default_bands() -> [f32; N] {
        let mut bands = [0.0; N];
        let step = ((32000 - 400) / N) as f32;
        for i in 0..N {
            bands[i] = 400.0 + (i as f32 * step);
        }
        bands
    }

    pub fn with_bands(mut self, bands: [f32; N]) -> Self {
        self.bands = bands;
        self
    }

    pub fn with_gains(mut self, gains: [f32; N]) -> Self {
        self.gains = gains;
        self
    }

    pub fn with_bandwidths(mut self, steepness: [f32; N]) -> Self {
        self.bandwidth = steepness;
        self
    }

    pub fn with_upper_shelf(mut self) -> Self {
        self.upper_shelf = true;
        self
    }

    /// If single band, lower shelf takes precedence over upper shelf
    pub fn with_lower_shelf(mut self) -> Self {
        self.lower_shelf = true;
        self
    }

    pub fn build(self) -> Equalizer {
        let mut biquads = Vec::with_capacity(N);
        for i in 0..N {
            let f = self.bands[i];
            // Greater q = steeper, lower bandwidth
            let q = 1.0 / self.bandwidth[i];
            let gain = self.gains[i];

            let bq;

            if self.lower_shelf && i == 0 {
                bq = BiquadFilter::low_shelf(f, self.sample_rate, q, gain);
            } else if self.upper_shelf && i == N-1 {
                bq = BiquadFilter::high_shelf(f, self.sample_rate, q, gain);
            } else {
                bq = BiquadFilter::peaking(f, self.sample_rate, q, gain);
            }

            biquads.push(bq);
        }
        Equalizer { biquads }
    }
}

#[derive(Clone)]
pub struct Equalizer {
    biquads: Vec<BiquadFilter>
}

impl Equalizer {
    pub fn new(biquads: Vec<BiquadFilter>) -> Self {
        Equalizer { biquads }
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let mut y = x;
        for bq in &mut self.biquads {
            y = bq.process(y);
        }
        y
    }

    pub fn response_at_freq(&self, f: f64, sample_rate: f64) -> Complex64 {
        let mut response = Complex64::new(1.0, 0.0);
        for bq in &self.biquads {
            response *= bq.response_at_freq(f, sample_rate);
        }
        response
    }

    /// log2 frequency response in dB from start_freq to end_freq with num_points points
    pub fn amplitude_response_plot(&self, sample_rate: f64, mut start_freq: f64, end_freq: f64, num_points: usize) -> Vec<PlotPoint> {
        if start_freq == 0.0 {
            start_freq += 1.0; // Avoid log2(0)
        }

        let mut response = Vec::with_capacity(num_points);
        let log2_start = start_freq.log2();
        let log2_end = end_freq.log2();
        let step = (log2_end - log2_start) / num_points as f64;
    
        for i in 0..num_points {
            let log2_f = log2_start + i as f64 * step;
            let f = 2f64.powf(log2_f);
            let complex_response = self.response_at_freq(f, sample_rate);
            let amplitude = complex_response.norm();
            let amplitude_db = 20.0 * amplitude.log10(); // Convert to dB
            response.push(PlotPoint::new(log2_f, amplitude_db));
        }
    
        response
    }
}
