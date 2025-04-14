#[derive(Debug, Clone, Copy)]
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
        let y = self.b[0] * x + self.b[1] * self.x[0] + self.b[2] * self.x[1]
            - self.a[0] * self.y[0] - self.a[1] * self.y[1];
        self.x[1] = self.x[0];
        self.x[0] = x;
        self.y[1] = self.y[0];
        self.y[0] = y;
        y
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
}
