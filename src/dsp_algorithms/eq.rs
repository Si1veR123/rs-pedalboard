use super::biquad::BiquadFilter;


pub struct DynamicEqualizerBuilder {
    pub bands: Vec<(f32, f32, f32)>,
    pub sample_rate: f32
}

impl DynamicEqualizerBuilder {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            bands: vec![],
            sample_rate
        }
    }

    pub fn with_band(mut self, f: f32, q: f32, gain: f32) -> Self {
        self.bands.push((f, q, gain));
        self
    }

    pub fn build(self) -> Vec<BiquadFilter> {
        let mut biquads = Vec::with_capacity(self.bands.len());
        for (f, q, gain) in self.bands {
            let bq = BiquadFilter::peaking_eq(f, self.sample_rate, q, gain);
            biquads.push(bq);
        }
        biquads
    }
}

pub struct GraphicEqualizerBuilder<const N: usize> {
    pub bands: [f32; N],
    pub gains: [f32; N],
    pub steepness: [f32; N],
    pub sample_rate: f32,
}

impl<const N: usize> GraphicEqualizerBuilder<N> {
    pub fn new(sample_rate: f32) -> Self {
        GraphicEqualizerBuilder {
            bands: Self::default_bands(),
            gains: [0.0; N],
            steepness: [1.0; N],
            sample_rate,
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

    pub fn with_steepness(mut self, steepness: [f32; N]) -> Self {
        self.steepness = steepness;
        self
    }

    pub fn build(self) -> Equalizer {
        let mut biquads = Vec::with_capacity(N);
        for i in 0..N {
            let f = self.bands[i];
            let q = self.steepness[i];
            let gain = self.gains[i];
            let bq = BiquadFilter::peaking_eq(f, self.sample_rate, q, gain);
            biquads.push(bq);
        }
        Equalizer { biquads }
    }
}

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
}
