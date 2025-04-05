
pub struct BiquadFilter {
    y: [f32; 2],
    x: [f32; 2],
    b: [f32; 3],
    a: [f32; 2]
}

impl BiquadFilter {
    pub fn new(a: [f32; 2], b: [f32; 3]) -> Self {
        BiquadFilter {
            y: [0.0, 0.0],
            x: [0.0, 0.0],
            b,
            a
        }
    }

    fn compute(f: f32, sample_rate: f32, q: f32) -> (f32, f32) {
        let w0 = 2.0 * std::f32::consts::PI * f / sample_rate;
        let alpha = w0.sin() / (2.0 * q);

        (w0, alpha)
    }

    pub fn low_pass(f: f32, sample_rate: f32, q: f32) -> Self {
        let (w0, alpha) = Self::compute(f, sample_rate, q);
        let b0 = (1.0 - (w0.cos())) / 2.0;
        let b1 = 1.0 - w0.cos();
        let b2 = (1.0 - (w0.cos())) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * w0.cos();
        let a2 = 1.0 - alpha;

        BiquadFilter::new([a1 / a0, a2 / a0], [b0 / a0, b1 / a0, b2 / a0])
    }

    pub fn high_pass(f: f32, sample_rate: f32, q: f32) -> Self {
        let (w0, alpha) = Self::compute(f, sample_rate, q);
        let b0 = (1.0 + (w0.cos())) / 2.0;
        let b1 = -(1.0 + (w0.cos()));
        let b2 = (1.0 + (w0.cos())) / 2.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * w0.cos();
        let a2 = 1.0 - alpha;

        BiquadFilter::new([a1 / a0, a2 / a0], [b0 / a0, b1 / a0, b2 / a0])
    }

    pub fn band_pass(f: f32, sample_rate: f32, q: f32) -> Self {
        let (w0, alpha) = Self::compute(f, sample_rate, q);
        let b0 = w0.sin() / 2.0;
        let b1 = 0.0;
        let b2 = -b0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * w0.cos();
        let a2 = 1.0 - alpha;

        BiquadFilter::new([a1 / a0, a2 / a0], [b0 / a0, b1 / a0, b2 / a0])
    }

    pub fn notch(f: f32, sample_rate: f32, q: f32) -> Self {
        let (w0, alpha) = Self::compute(f, sample_rate, q);
        let b0 = 1.0;
        let b1 = -2.0 * w0.cos();
        let b2 = 1.0;
        let a0 = 1.0 + alpha;
        let a1 = -2.0 * w0.cos();
        let a2 = 1.0 - alpha;

        BiquadFilter::new([a1 / a0, a2 / a0], [b0 / a0, b1 / a0, b2 / a0])
    }

    pub fn peaking_eq(f: f32, sample_rate: f32, q: f32, gain: f32) -> Self {
        let (w0, alpha) = Self::compute(f, sample_rate, q);
        let a = 10f32.powf(gain / 40.0);
        let b0 = 1.0 + (alpha * a);
        let b1 = -2.0 * w0.cos();
        let b2 = 1.0 - (alpha * a);
        let a0 = 1.0 + (alpha / a);
        let a1 = -2.0 * w0.cos();
        let a2 = 1.0 - (alpha / a);

        BiquadFilter::new([a1 / a0, a2 / a0], [b0 / a0, b1 / a0, b2 / a0])
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0] - self.a[1] * self.y[1];
        self.x[1] = self.x[0];
        self.x[0] = x;
        self.y[1] = self.y[0];
        self.y[0] = y;
        y
    }
}
