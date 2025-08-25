/// WIP

pub struct Resampler {
    stages: Vec<HalfBandFilter>,
    scratch: Vec<f32>,
}

impl Resampler {
    pub fn new(passes: usize, max_block: usize) -> Self {
        let mut stages = Vec::with_capacity(passes);
        for _ in 0..passes {
            stages.push(HalfBandFilter::new(63));
        }
        // scratch buffer must hold the max expanded size
        let scratch = vec![0.0; max_block << passes];
        Self { stages, scratch }
    }

    pub fn upsample(&mut self, input: &[f32], output: &mut [f32]) {
        let mut cur_len = input.len();
        self.scratch[..cur_len].copy_from_slice(input);
        let mut use_output = true;

        for stage in &mut self.stages {
            let next_len = cur_len * 2;
            if use_output {
                stage.upsample(&self.scratch[..cur_len], &mut output[..next_len]);
            } else {
                stage.upsample(&output[..cur_len], &mut self.scratch[..next_len]);
            }
            cur_len = next_len;
            use_output = !use_output;
        }

        if use_output {
            output[..cur_len].copy_from_slice(&self.scratch[..cur_len]);
        }
    }

    pub fn downsample(&mut self, input: &[f32], output: &mut [f32]) {
        let mut cur_len = input.len();
        self.scratch[..cur_len].copy_from_slice(input);
        let mut use_output = true;

        for stage in self.stages.iter_mut().rev() {
            let next_len = cur_len / 2;
            if use_output {
                stage.downsample(&self.scratch[..cur_len], &mut output[..next_len]);
            } else {
                stage.downsample(&output[..cur_len], &mut self.scratch[..next_len]);
            }
            cur_len = next_len;
            use_output = !use_output;
        }

        if use_output {
            output[..cur_len].copy_from_slice(&self.scratch[..cur_len]);
        }
    }

    pub fn output_buffer_size(&self, input_size: usize) -> usize {
        input_size << self.stages.len()
    }
}

pub struct HalfBandFilter {
    // even-phase taps: h[0], h[2], h[4], ... (center tap is at an odd index for odd-length filter)
    phase_even: Vec<f32>,
    center_tap: f32,
    delay: Vec<f32>,
    pos: usize,
    len: usize,
    mid: usize,
}

impl HalfBandFilter {
    /// Create a true halfband filter with alternating zero taps
    pub fn new(length: usize) -> Self {
        assert!(length % 2 == 1, "Halfband filter length must be odd");
        let mid = length / 2;

        // Ideal halfband (cutoff 0.25)
        let mut taps = vec![0.0; length];
        for n in 0..length {
            let k = n as isize - mid as isize;
            if k == 0 {
                taps[n] = 0.5; // center tap = 0.5
            } else if k % 2 == 0 {
                let x = std::f32::consts::PI * k as f32 / 2.0;
                taps[n] = 0.5 * x.sin() / x;
            } else {
                taps[n] = 0.0; // odd taps other than center are zero
            }
        }

        // Blackman window
        for n in 0..length {
            let w = 0.42
                - 0.5 * ((2.0 * std::f32::consts::PI * n as f32) / (length as f32 - 1.0)).cos()
                + 0.08 * ((4.0 * std::f32::consts::PI * n as f32) / (length as f32 - 1.0)).cos();
            taps[n] *= w;
        }

        // Normalize DC gain to 1
        let sum: f32 = taps.iter().sum();
        for t in taps.iter_mut() {
            *t /= sum;
        }

        Self::new_from_taps(taps)
    }

    pub fn new_from_taps(taps: Vec<f32>) -> Self {
        assert!(taps.len() % 2 == 1, "Halfband filter must have odd length");
        let len = taps.len();
        let mid = len / 2;
        let center_tap = taps[mid];

        // store only even-index taps (k = 0,2,4,...) – center tap is at an odd index for odd len, so excluded here
        let phase_even: Vec<f32> = taps
            .iter()
            .enumerate()
            .filter(|(i, _)| i % 2 == 0)
            .map(|(_, &c)| c)
            .collect();

        let delay = vec![0.0; len];
        Self {
            phase_even,
            center_tap,
            delay,
            pos: 0,
            len,
            mid,
        }
    }

    #[inline]
    fn delay_idx(&self, back: usize) -> usize {
        (self.pos + self.len - (back % self.len)) % self.len
    }

    pub fn upsample(&mut self, input: &[f32], output: &mut [f32]) {
        assert_eq!(output.len(), input.len() * 2, "upsample: wrong output size");
        // For each x[n] written at pos, y[2n] = sum h[2m] x[n - m]; y[2n+1] = h[mid] x[n - mid/??]
        // Since the center tap index is `mid` (odd), the odd output is simply center * x[n - mid],
        // which we fetch from the delay.
        for (i, &x) in input.iter().enumerate() {
            // write current sample
            self.delay[self.pos] = x;

            // Even output: sum over even taps (k = 2m)
            let mut even_out = 0.0;
            for (m, &c) in self.phase_even.iter().enumerate() {
                let k = 2 * m; // original tap index
                let idx = self.delay_idx(k);
                even_out += c * self.delay[idx];
            }

            // Odd output: only center tap contributes (all other odd taps are zero)
            let odd_idx = self.delay_idx(self.mid);
            let odd_out = self.center_tap * self.delay[odd_idx];

            output[2 * i] = even_out;
            output[2 * i + 1] = odd_out;

            // advance circular pointer
            self.pos = (self.pos + 1) % self.len;
        }
    }

    pub fn downsample(&mut self, input: &[f32], output: &mut [f32]) {
        assert_eq!(input.len(), output.len() * 2, "downsample: wrong input/output sizes");

        // Push samples one by one; emit an output after every 2 pushes.
        let mut out_i = 0usize;
        let mut parity = 0u8;

        for &s in input {
            self.delay[self.pos] = s;
            self.pos = (self.pos + 1) % self.len;

            parity ^= 1;
            if parity == 0 {
                // We have just consumed an even+odd pair -> produce one output.
                let mut acc = 0.0;

                // Even-tap branch
                for (m, &c) in self.phase_even.iter().enumerate() {
                    let k = 2 * m; // original tap index
                    let idx = self.delay_idx(k);
                    acc += c * self.delay[idx];
                }

                // Center tap contribution (only non-zero odd tap)
                let cidx = self.delay_idx(self.mid);
                acc += self.center_tap * self.delay[cidx];

                output[out_i] = acc;
                out_i += 1;
            }
        }
    }
}