use realfft::{ComplexToReal, RealFftPlanner, RealToComplex};
use num_complex::Complex;
use rubato::{Resampler, SincFixedIn, SincInterpolationParameters};
use std::{path::Path, sync::Arc};

pub fn load_wav<P: AsRef<Path>>(wav_path: P, sample_rate: f32, normalise: bool) -> Result<Vec<Vec<f32>>, String> {
    let mut reader = hound::WavReader::open(wav_path.as_ref()).map_err(
        |e| format!("Failed to open WAV file '{}': {}", wav_path.as_ref().display(), e)
    )?;
    
    let spec = reader.spec();
    if spec.bits_per_sample > 32 {
        return Err("WAV file has more than 32 bits per sample. This is not supported.".into());
    }

    let resample_ratio = sample_rate / spec.sample_rate as f32;

    let float_samples = match spec.sample_format {
        hound::SampleFormat::Float => {
            let ir_samples: Result<Vec<f32>, _> = reader.into_samples().collect();
            ir_samples.map_err(|e| e.to_string())
        },
        hound::SampleFormat::Int => {
            let max_amplitude = (1i64 << (spec.bits_per_sample - 1)) as f32;
            let ir_samples: Result<Vec<f32>, _> = reader.samples::<i32>()
                .map(|s| s.and_then(|s| Ok(s as f32 / max_amplitude)))
                .collect();
            ir_samples.map_err(|e| e.to_string())
        }
    };

    float_samples.and_then(|float_samples| {
        let num_channels = spec.channels as usize;

        let mut resampler = SincFixedIn::<f32>::new(
            resample_ratio as f64,
            1.0,
            SincInterpolationParameters {
                sinc_len: 512,
                f_cutoff: 0.9,
                oversampling_factor: 128,
                interpolation: rubato::SincInterpolationType::Linear,
                window: rubato::WindowFunction::BlackmanHarris2,
            },
            float_samples.len() / num_channels,
            num_channels,
        ).map_err(|e| e.to_string())?;

        // Deinterleave samples into a Vec of channels
        let mut channels: Vec<Vec<f32>> = vec![Vec::with_capacity(float_samples.len() / num_channels); num_channels];
        for frame in float_samples.chunks_exact(num_channels) {
            for (i, &sample) in frame.iter().enumerate() {
                channels[i].push(sample);
            }
        }

        let channel_refs: Vec<&[f32]> = channels.iter().map(|ch| ch.as_slice()).collect();

        let mut resampled_channels = resampler.process(&channel_refs, None)
            .map_err(|e| e.to_string())?;

        if normalise {
            // Normalize WAV by RMS
            let rms = resampled_channels
                .iter()
                .flat_map(|ch| ch.iter())
                .map(|&x| x * x)
                .sum::<f32>()
                .sqrt();

            if rms > 1e-12 {
                let scale = 1.0 / rms;
                for ch in &mut resampled_channels {
                    for s in ch.iter_mut() {
                        *s *= scale;
                    }
                }
            }
        }
        Ok(resampled_channels)
    })
}

#[derive(Clone)]
pub struct IRConvolver {
    fft_size: usize,
    block_size: usize,
    ir_freq: Vec<Complex<f32>>,
    fft: Arc<dyn RealToComplex<f32>>,
    ifft: Arc<dyn ComplexToReal<f32>>,
    input_buffer: Vec<f32>,
    overlap: Vec<f32>,
    scratch: Vec<Complex<f32>>,
    input_freq: Vec<Complex<f32>>,
    ifft_out: Vec<f32>,
}

impl IRConvolver {
    pub fn new(ir: &[f32], max_block_size: usize) -> Self {
        let fft_size = (max_block_size + ir.len() - 1).next_power_of_two();

        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        let ifft = planner.plan_fft_inverse(fft_size);

        // Pre-transform IR
        let mut ir_padded = vec![0.0f32; fft_size];
        ir_padded[..ir.len()].copy_from_slice(ir);
        let mut ir_freq = fft.make_output_vec();
        fft.process(&mut ir_padded, &mut ir_freq).unwrap();

        IRConvolver {
            fft_size,
            block_size: max_block_size,
            ir_freq,
            ifft_out: ifft.make_output_vec(),
            input_freq: fft.make_output_vec(),
            fft,
            ifft,
            input_buffer: vec![0.0; fft_size],
            overlap: vec![0.0; fft_size],
            scratch: vec![Complex::default(); fft_size],
        }
    }

    pub fn process(&mut self, mut buffer: &mut [f32]) {
        if buffer.len() > self.block_size {
            tracing::warn!("IRConvolver: buffer size exceeds maximum block size.");
            buffer = &mut buffer[..self.block_size];
        }

        // Shift in new input
        self.input_buffer.fill(0.0);
        self.input_buffer[..buffer.len()].copy_from_slice(buffer);

        self.fft.process_with_scratch(&mut self.input_buffer, &mut self.input_freq, &mut self.scratch).unwrap();

        // Multiply in frequency domain
        for (x, h) in self.input_freq.iter_mut().zip(&self.ir_freq) {
            *x = *x * *h;
        }

        // IFFT
        self.ifft.process(&mut self.input_freq, &mut self.ifft_out).unwrap();

        // Normalize
        let scale = self.fft_size as f32;
        for sample in self.ifft_out.iter_mut() {
            *sample /= scale;
        }

        // Add overlap from previous block
        for i in 0..self.fft_size {
            self.ifft_out[i] += self.overlap[i];
        }

        // Write block to output
        buffer.copy_from_slice(&self.ifft_out[..buffer.len()]);

        // Copy any remaining samples to overlap buffer
        self.overlap.fill(0.0);
        for i in buffer.len()..self.fft_size {
            self.overlap[i-buffer.len()] = self.ifft_out[i];
        }
    }

    pub fn reset(&mut self) {
        self.input_buffer.fill(0.0);
        self.overlap.fill(0.0);
        self.input_freq.fill(Complex::default());
        self.ifft_out.fill(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_nearly_eq_array {
        ($a:expr, $b:expr, $epsilon:expr) => {
            for (i, (x, y)) in $a.iter().zip($b.iter()).enumerate() {
                assert!((x - y).abs() < $epsilon, "Arrays differ at index {}: {} vs {}", i, x, y);
            }
        };
    }

    #[test]
    fn impulse_response_identity_test() {
        // Impulse response: identity (i.e. passes input through unchanged)
        let ir = vec![1.0, 0.0, 0.0, 0.0];
        let block_size = 4;

        let mut convolver = IRConvolver::new(&ir, block_size);

        // Input: a single impulse followed by zeros
        let mut input = vec![1.0, 0.0, 0.0, 0.0];

        convolver.process(&mut input);

        // Output should match the IR
        for (i, e) in input.iter().zip(ir.iter()) {
            assert!((i - e).abs() < 1e-6, "Expected {}, got {}", e, i);
        }
    }

    #[test]
    fn impulse_response_echo_test() {
        // IR simulates an echo
        let ir = vec![0.5, 0.0, 0.0, 0.5];
        let block_size = 4;

        let mut convolver = IRConvolver::new(&ir, block_size);

        let mut input = vec![1.0, 0.0, 0.0, 0.0];

        convolver.process(&mut input);

        // Expected output: impulse convolved with IR
        let expected = vec![0.5, 0.0, 0.0, 0.5];
        for (i, e) in input.iter().zip(expected.iter()) {
            assert!((i - e).abs() < 1e-6, "Expected {}, got {}", e, i);
        }
    }

    #[test]
    fn test_ir_convolver_multiple_blocks() {
        let ir = vec![0.5, 0.2, 0.3];

        // Use a block size of 4
        let mut convolver = IRConvolver::new(&ir, 4);

        // Input: 3 blocks of audio, each of size 4
        let mut input_block1 = vec![1.0, 1.0, 1.0, 0.0];
        let mut input_block2 = vec![0.0, 0.0, 3.0, 1.0];
        let mut input_block3 = vec![0.0, 0.0, 0.0, 1.0];

        convolver.process(&mut input_block1);

        assert_nearly_eq_array!(
            input_block1,
            vec![0.5, 0.7, 1.0, 0.5],
            1e-6
        );

        convolver.process(&mut input_block2);

        assert_nearly_eq_array!(
            input_block2,
            vec![0.3, 0.0, 1.5, 1.1],
            1e-6
        );

        convolver.process(&mut input_block3);

        assert_nearly_eq_array!(
            input_block3,
            vec![1.1, 0.3, 0.0, 0.5],
            1e-6
        );
    }
}