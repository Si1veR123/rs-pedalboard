use num_complex::Complex64;

#[derive(Debug, Clone, Copy)]
pub struct BiquadFilter {
    pub s1: f32,
    pub s2: f32,
    b: [f32; 3],
    a: [f32; 2],
}

impl BiquadFilter {
    pub fn new(a: [f32; 2], b: [f32; 3]) -> Self {
        BiquadFilter {
            s1: 0.0,
            s2: 0.0,
            b,
            a,
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

    /// First order (6 dB/oct) low pass. Used by tone controls that are built from a
    /// single RC section, where a resonant second order filter would be too sharp.
    pub fn first_order_low_pass(f: f32, sample_rate: f32) -> Self {
        // Prewarped bilinear transform: H(s) = 1 / (1 + s / wc).
        let g = (std::f32::consts::PI * f / sample_rate).tan();
        let a0 = 1.0 + g;

        BiquadFilter::new([(g - 1.0) / a0, 0.0], [g / a0, g / a0, 0.0])
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

    pub fn peaking(f: f32, sample_rate: f32, q: f32, gain: f32) -> Self {
        let (w0, alpha) = Self::compute(f, sample_rate, q);
        let a = (10f32.powf(gain / 20.0)).sqrt();
        let b0 = 1.0 + (alpha * a);
        let b1 = -2.0 * w0.cos();
        let b2 = 1.0 - (alpha * a);
        let a0 = 1.0 + (alpha / a);
        let a1 = -2.0 * w0.cos();
        let a2 = 1.0 - (alpha / a);

        BiquadFilter::new([a1 / a0, a2 / a0], [b0 / a0, b1 / a0, b2 / a0])
    }

    pub fn low_shelf(f: f32, sample_rate: f32, q: f32, gain: f32) -> Self {
        let a = 10f32.powf(gain / 40.0);

        let (w0, alpha) = Self::compute(f, sample_rate, q);

        let b0 = a * ((a + 1.0) - (a - 1.0) * w0.cos() + (2.0 * a.sqrt() * alpha));
        let b1 = 2.0 * a * ((a - 1.0) - (a + 1.0) * w0.cos());
        let b2 = a * ((a + 1.0) - (a - 1.0) * w0.cos() - (2.0 * a.sqrt() * alpha));
        let a0 = (a + 1.0) + (a - 1.0) * w0.cos() + (2.0 * a.sqrt() * alpha);
        let a1 = -2.0 * ((a - 1.0) + (a + 1.0) * w0.cos());
        let a2 = (a + 1.0) + (a - 1.0) * w0.cos() - (2.0 * a.sqrt() * alpha);

        BiquadFilter::new([a1 / a0, a2 / a0], [b0 / a0, b1 / a0, b2 / a0])
    }

    pub fn high_shelf(f: f32, sample_rate: f32, q: f32, gain: f32) -> Self {
        let a = 10f32.powf(gain / 40.0);
        let (w0, alpha) = Self::compute(f, sample_rate, q);

        let b0 = a * ((a + 1.0) + (a - 1.0) * w0.cos() + (2.0 * a.sqrt() * alpha));
        let b1 = -2.0 * a * ((a - 1.0) + (a + 1.0) * w0.cos());
        let b2 = a * ((a + 1.0) + (a - 1.0) * w0.cos() - (2.0 * a.sqrt() * alpha));
        let a0 = (a + 1.0) - (a - 1.0) * w0.cos() + (2.0 * a.sqrt() * alpha);
        let a1 = 2.0 * ((a - 1.0) - (a + 1.0) * w0.cos());
        let a2 = (a + 1.0) - (a - 1.0) * w0.cos() - (2.0 * a.sqrt() * alpha);

        BiquadFilter::new([a1 / a0, a2 / a0], [b0 / a0, b1 / a0, b2 / a0])
    }

    pub fn process(&mut self, x: f32) -> f32 {
        let y = self.b[0] * x + self.s1;
        self.s1 = self.b[1] * x - self.a[0] * y + self.s2;
        self.s2 = self.b[2] * x - self.a[1] * y;
        y
    }

    pub fn response_at_freq(&self, f: f64, sample_rate: f64) -> Complex64 {
        let omega = 2.0 * std::f64::consts::PI * f / sample_rate;
        let z1 = Complex64::from_polar(1.0, -omega);
        let z2 = Complex64::from_polar(1.0, -2.0 * omega);

        let num = self.b[0] as f64 + self.b[1] as f64 * z1 + self.b[2] as f64 * z2;
        let den = 1.0 + self.a[0] as f64 * z1 + self.a[1] as f64 * z2;
        num / den
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sine_wave(frequency: f32, sample_rate: f32, duration: f32) -> Vec<f32> {
        let num_samples = (sample_rate * duration) as usize;
        (0..num_samples)
            .map(|i| {
                let t = i as f32 / sample_rate;
                (2.0 * std::f32::consts::PI * frequency * t).sin()
            })
            .collect()
    }

    fn rms_energy(signal: &[f32]) -> f32 {
        let sum_of_squares: f32 = signal.iter().map(|&x| x * x).sum();
        (sum_of_squares / signal.len() as f32).sqrt()
    }

    #[test]
    fn test_high_shelf() {
        let high_freq = 6000.0;
        let low_freq = 300.0;

        let sample_rate = 48000.0;
        let q = 0.707;

        let mut filter = BiquadFilter::high_shelf(4000.0, sample_rate, q, -10.0);
        let input = sine_wave(low_freq, sample_rate, 1.0);
        let mut output = vec![0.0; input.len()];
        for i in 0..input.len() {
            output[i] = filter.process(input[i]);
        }

        let mut filter2 = BiquadFilter::high_shelf(4000.0, sample_rate, q, -10.0);
        let input2 = sine_wave(high_freq, sample_rate, 1.0);
        let mut output2 = vec![0.0; input2.len()];
        for i in 0..input2.len() {
            output2[i] = filter2.process(input2[i]);
        }

        assert!(rms_energy(&output) > rms_energy(&output2));

        dbg!(rms_energy(&input), rms_energy(&output));
        dbg!(rms_energy(&input2), rms_energy(&output2));
    }

    #[test]
    fn test_first_order_low_pass() {
        let sample_rate = 48000.0;
        let corner = 720.0;

        let filter = BiquadFilter::first_order_low_pass(corner, sample_rate);

        let at_corner = filter
            .response_at_freq(corner as f64, sample_rate as f64)
            .norm();
        let well_below = filter
            .response_at_freq(corner as f64 / 8.0, sample_rate as f64)
            .norm();
        let octave_above = filter
            .response_at_freq(corner as f64 * 2.0, sample_rate as f64)
            .norm();

        // -3 dB at the corner, flat below it and 6 dB/oct above it.
        assert!((at_corner - std::f64::consts::FRAC_1_SQRT_2).abs() < 1e-3);
        assert!(well_below > 0.99);
        assert!((octave_above - 1.0 / 5.0f64.sqrt()).abs() < 1e-2);

        // A DC input settles at unity gain.
        let mut settling = BiquadFilter::first_order_low_pass(corner, sample_rate);
        let mut output = 0.0;
        for _ in 0..1000 {
            output = settling.process(1.0);
        }
        assert!((output - 1.0).abs() < 1e-3);
    }
}
