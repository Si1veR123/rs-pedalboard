/// A polyphase half band resampler for upsampling and downsampling by powers of two.
/// 
/// Each resampler should be used for either upsampling or downsampling.
pub struct Resampler {
    stages: Vec<HalfBandFilter>,
    scratch_a: Vec<f32>,
    scratch_b: Vec<f32>,
}

impl Resampler {
    pub fn new(passes: usize, max_block: usize) -> Self {
        let mut stages = Vec::with_capacity(passes);
        for _ in 0..passes {
            stages.push(HalfBandFilter::new(63));
        }
        // scratch buffer must hold the max expanded size
        let scratch_a = vec![0.0; max_block << passes];
        let scratch_b = scratch_a.clone();
        Self { stages, scratch_a, scratch_b }
    }

    pub fn upsample(&mut self, input: &[f32], output: &mut [f32]) {
        let mut cur_len = input.len();

        if self.scratch_a.len() < (input.len() << self.stages.len()) {
            log::warn!("Resampler: input size exceeds maximum block size, resizing scratch buffer.");
            self.scratch_a.resize(input.len() << self.stages.len(), 0.0);
        }

        self.scratch_a[..cur_len].copy_from_slice(input);
        let mut use_output = true;

        for stage in &mut self.stages {
            let next_len = cur_len * 2;
            if use_output {
                stage.upsample(&self.scratch_a[..cur_len], &mut output[..next_len]);
            } else {
                stage.upsample(&output[..cur_len], &mut self.scratch_a[..next_len]);
            }
            cur_len = next_len;
            use_output = !use_output;
        }

        if use_output {
            output[..cur_len].copy_from_slice(&self.scratch_a[..cur_len]);
        }
    }

    pub fn downsample(&mut self, input: &[f32], output: &mut [f32]) {
        let mut cur_len = input.len();

        if self.scratch_a.len() < (input.len() << self.stages.len()) {
            log::warn!("Resampler: input size exceeds maximum block size, resizing scratch buffer.");
            self.scratch_a.resize(input.len() << self.stages.len(), 0.0);
        }
        if self.scratch_b.len() < (input.len() << self.stages.len()) {
            log::warn!("Resampler: input size exceeds maximum block size, resizing scratch buffer.");
            self.scratch_b.resize(input.len() << self.stages.len(), 0.0);
        }
        self.scratch_a[..cur_len].copy_from_slice(input);
        let mut use_a = true;

        for stage in self.stages.iter_mut().rev() {
            let next_len = cur_len / 2;
            if use_a {
                stage.downsample(&self.scratch_a[..cur_len], &mut self.scratch_b[..next_len]);
            } else {
                stage.downsample(&self.scratch_b[..cur_len], &mut self.scratch_a[..next_len]);
            }
            cur_len = next_len;
            use_a = !use_a;
        }

        if use_a {
            output[..cur_len].copy_from_slice(&self.scratch_a[..cur_len]);
        } else {
            output[..cur_len].copy_from_slice(&self.scratch_b[..cur_len]);
        }
    }

    /// Calculate the output buffer size for upsampling
    pub fn upsample_output_buffer_size(&self, input_size: usize) -> usize {
        input_size << self.stages.len()
    }

    /// Calculate the output buffer size for downsampling
    pub fn downsample_output_buffer_size(&self, input_size: usize) -> usize {
        input_size >> self.stages.len()
    }
}

pub struct HalfBandFilter {
    h_even: Vec<f32>,
    center_tap: f32,
    delay: Vec<f32>,
    pos: usize,
    len: usize,
    mid: usize,
}

impl HalfBandFilter {
    pub fn new(length: usize) -> Self {
        assert!(length % 2 == 1, "length must be odd");
        let mid = length / 2;
        let mut taps = vec![0.0f32; length];

        // ideal halfband prototype
        for n in 0..length {
            let k = n as isize - mid as isize;
            if k == 0 {
                taps[n] = 0.5;
            } else if k % 2 == 0 {
                let kf = k as f32;
                taps[n] = (std::f32::consts::PI * 0.5 * kf).sin() /
                          (std::f32::consts::PI * kf);
            } else {
                taps[n] = 0.0; // exact zeros for odd indices
            }
        }

        // Blackman window
        for n in 0..length {
            let w = 0.42
                - 0.5 * ((2.0 * std::f32::consts::PI * n as f32) / (length as f32 - 1.0)).cos()
                + 0.08 * ((4.0 * std::f32::consts::PI * n as f32) / (length as f32 - 1.0)).cos();
            taps[n] *= w;
        }

        // Normalize DC gain
        let sum: f32 = taps.iter().sum();
        for v in taps.iter_mut() { *v /= sum; }

        Self::new_from_taps(taps)
    }

    pub fn new_from_taps(taps: Vec<f32>) -> Self {
        let len = taps.len();
        assert!(len % 2 == 1, "taps length must be odd");
        let mid = len / 2;

        // keep only nonzero even taps
        let mut h_even = Vec::new();
        for (i, &c) in taps.iter().enumerate() {
            if i % 2 == 0 && i != mid {
                h_even.push(c);
            }
        }

        let center_tap = taps[mid];
        let delay = vec![0.0f32; len];

        Self { h_even, center_tap, delay, pos: 0, len, mid }
    }

    fn idx(&self, base: usize, offset: usize) -> usize {
        if base >= offset { base - offset } else { base + self.len - offset }
    }

    pub fn upsample(&mut self, input: &[f32], output: &mut [f32]) {
        assert_eq!(output.len(), input.len() * 2);

        for (i, &x) in input.iter().enumerate() {
            self.delay[self.pos] = x;
            let base = if self.pos == 0 { self.len - 1 } else { self.pos - 1 };

            // y[2n] = convolution with even taps
            let mut even_out = 0.0;
            for (k, &c) in self.h_even.iter().enumerate() {
                let d1 = self.idx(base, 2*k);
                let d2 = self.idx(base, self.len - 1 - 2*k);
                even_out += c * (self.delay[d1] + self.delay[d2]);
            }
            even_out += self.center_tap * self.delay[self.idx(base, self.mid)];

            // y[2n+1] = center_tap * newest sample
            let odd_out = self.center_tap * self.delay[self.pos];

            output[2*i]   = even_out;
            output[2*i+1] = odd_out;

            self.pos = (self.pos + 1) % self.len;
        }
    }

    pub fn downsample(&mut self, input: &[f32], output: &mut [f32]) {
        assert_eq!(input.len(), output.len() * 2);

        for (i, chunk) in input.chunks_exact(2).enumerate() {
            self.delay[self.pos] = chunk[0];
            self.pos = (self.pos + 1) % self.len;
            self.delay[self.pos] = chunk[1];
            self.pos = (self.pos + 1) % self.len;

            let base = if self.pos == 0 { self.len - 1 } else { self.pos - 1 };

            let mut acc = 0.0;
            for (k, &c) in self.h_even.iter().enumerate() {
                let d1 = self.idx(base, 2*k);
                let d2 = self.idx(base, self.len - 1 - 2*k);
                acc += c * (self.delay[d1] + self.delay[d2]);
            }
            acc += self.center_tap * self.delay[self.idx(base, self.mid)];

            output[i] = acc;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use hound::{WavReader, WavWriter};
    use std::io::{self, Write};

    fn samples_from_file(path: &std::path::Path) -> (Vec<f32>, hound::WavSpec) {
        let mut reader = WavReader::open(path).expect("Failed to open test WAV");
        let spec = reader.spec();
        let channels = spec.channels;

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
        }.expect("Failed to read samples");

        (
            float_samples.into_iter()
            .enumerate()
            .filter_map(|(i, s)| if i as u16 % channels == 0 { Some(s) } else { None })
            .collect::<Vec<f32>>(),
            spec
        )
    }

    fn create_resampled_files(path: &std::path::Path, resampler: &mut Resampler) {
        let (input_first_channel, spec) = samples_from_file(path);

        // Upsample
        let out_size = resampler.upsample_output_buffer_size(input_first_channel.len());
        let mut output = vec![0.0; out_size];
        resampler.upsample(&input_first_channel, &mut output);
        let out_spec = hound::WavSpec {
            channels: 1,
            sample_rate: spec.sample_rate * 2_usize.pow(resampler.stages.len() as u32) as u32,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = WavWriter::create(path.with_file_name("upsampled.wav"), out_spec).expect("Failed to create output WAV");
        
        for &s in &output {
            writer.write_sample(s).unwrap();
        }

        writer.finalize().unwrap();

        // Now downsample
        let down_out_size = resampler.downsample_output_buffer_size(output.len());
        let mut down_output = vec![0.0; down_out_size];
        resampler.downsample(&output, &mut down_output);
        let down_spec = hound::WavSpec {
            channels: 1,
            sample_rate: spec.sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut down_writer = WavWriter::create(path.with_file_name("downsampled.wav"), down_spec).expect("Failed to create downsampled WAV");
        for &s in &down_output {
            down_writer.write_sample(s).unwrap();
        }
        down_writer.finalize().unwrap();
    }

    fn create_resampled_files_block(path: &std::path::Path, resampler: &mut Resampler) {
        let (input_first_channel, spec) = samples_from_file(path);

        let block_size = 256;

        // Upsampling in blocks
        let mut upsampled = Vec::new();
        let mut block = vec![0.0; block_size];

        for chunk in input_first_channel.chunks(block_size) {
            block[..chunk.len()].copy_from_slice(chunk);
            if chunk.len() < block_size {
                block[chunk.len()..].fill(0.0);
            }

            let out_size = resampler.upsample_output_buffer_size(chunk.len());
            let mut out_block = vec![0.0; out_size];
            resampler.upsample(&block[..chunk.len()], &mut out_block);

            upsampled.extend_from_slice(&out_block);
        }

        let out_spec = hound::WavSpec {
            channels: 1,
            sample_rate: spec.sample_rate * 2_usize.pow(resampler.stages.len() as u32) as u32,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut writer = WavWriter::create(path.with_file_name("upsampled_block.wav"), out_spec)
            .expect("Failed to create output WAV");

        for &s in &upsampled {
            writer.write_sample(s).unwrap();
        }
        writer.finalize().unwrap();

        // Downsampling in blocks
        let mut downsampled = Vec::new();
        let mut block = vec![0.0; block_size];

        for chunk in upsampled.chunks(block_size) {
            block[..chunk.len()].copy_from_slice(chunk);
            if chunk.len() < block_size {
                block[chunk.len()..].fill(0.0);
            }

            let out_size = resampler.downsample_output_buffer_size(chunk.len());
            let mut out_block = vec![0.0; out_size];
            resampler.downsample(&block[..chunk.len()], &mut out_block);

            downsampled.extend_from_slice(&out_block);
        }

        let down_spec = hound::WavSpec {
            channels: 1,
            sample_rate: spec.sample_rate,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let mut down_writer =
            WavWriter::create(path.with_file_name("downsampled_block.wav"), down_spec)
                .expect("Failed to create downsampled WAV");

        for &s in &downsampled {
            down_writer.write_sample(s).unwrap();
        }
        down_writer.finalize().unwrap();
    }

    #[test]
    fn test_resampler() {
        let mut resampler = Resampler::new(2, 100);

        // Enter wav file to upsample
        print!("Enter a path to upsample: ");
        io::stdout().flush().unwrap();
        let mut input_string = String::new();
        io::stdin().read_line(&mut input_string).expect("Failed to read line");

        let test_path = std::path::Path::new(input_string.trim());
        create_resampled_files(test_path, &mut resampler);
    }

    #[test]
    fn test_resampler_block() {
        let mut resampler = Resampler::new(1, 100);

        // Enter wav file to upsample
        print!("Enter a path to upsample in blocks: ");
        io::stdout().flush().unwrap();
        let mut input_string = String::new();
        io::stdin().read_line(&mut input_string).expect("Failed to read line");

        let test_path = std::path::Path::new(input_string.trim());
        create_resampled_files_block(test_path, &mut resampler);
    }
}
