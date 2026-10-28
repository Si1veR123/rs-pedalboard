use num_complex::Complex;
use realfft::{ComplexToReal, RealFftPlanner, RealToComplex, num_traits::Euclid};

use std::{collections::VecDeque, sync::Arc};

fn compute_linear_gain(ir: &[f32]) -> f32 {
    if ir.is_empty() {
        return 1.0;
    }

    // Linear gain of IR is the RMS of the impulse response
    let sum_squares: f32 = ir.iter().map(|&x| x * x).sum();
    let rms = (sum_squares / ir.len() as f32).sqrt();
    rms
}

#[derive(Clone)]
pub struct IRConvolver {
    fft_size: usize,
    block_size: usize,
    ir_freq_buffer: Vec<Complex<f32>>,
    fft: Arc<dyn RealToComplex<f32>>,
    ifft: Arc<dyn ComplexToReal<f32>>,
    input_samples_buffer: Vec<f32>,
    overlap_buffer: Vec<f32>,
    scratch_buffer: Vec<Complex<f32>>,
    input_frequency_bins_buffer: Vec<Complex<f32>>,
    ifft_out_buffer: Vec<f32>,
}

impl IRConvolver {
    pub fn new(ir: &[f32], max_block_size: usize) -> Self {
        let fft_size = (max_block_size + ir.len() - 1).next_power_of_two();

        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        let ifft = planner.plan_fft_inverse(fft_size);

        let linear_gain = compute_linear_gain(ir);

        // Find maximum gain in frequency response of IR for normalisation
        let mut ir_padded = vec![0.0f32; fft_size];
        ir_padded[..ir.len()].copy_from_slice(ir);

        // Scale the IR by the linear gain to normalise it
        ir_padded.iter_mut().for_each(|x| *x /= linear_gain);

        let mut ir_freq_buffer = fft.make_output_vec();
        fft.process(&mut ir_padded, &mut ir_freq_buffer).unwrap();

        IRConvolver {
            fft_size,
            block_size: max_block_size,
            ir_freq_buffer,
            ifft_out_buffer: ifft.make_output_vec(),
            input_frequency_bins_buffer: fft.make_output_vec(),
            fft,
            ifft,
            input_samples_buffer: vec![0.0; fft_size],
            overlap_buffer: vec![0.0; fft_size],
            scratch_buffer: vec![Complex::default(); fft_size],
        }
    }

    pub fn process(&mut self, mut buffer: &mut [f32]) {
        if buffer.len() > self.block_size {
            tracing::warn!("IRConvolver: buffer size exceeds maximum block size.");
            buffer = &mut buffer[..self.block_size];
        }

        // Shift in new input
        self.input_samples_buffer.fill(0.0);
        self.input_samples_buffer[..buffer.len()].copy_from_slice(buffer);

        self.fft
            .process_with_scratch(
                &mut self.input_samples_buffer,
                &mut self.input_frequency_bins_buffer,
                &mut self.scratch_buffer,
            )
            .unwrap();

        // Multiply in frequency domain
        for (x, h) in self.input_frequency_bins_buffer.iter_mut().zip(&self.ir_freq_buffer) {
            *x = *x * *h;
        }

        // IFFT
        self.ifft
            .process(&mut self.input_frequency_bins_buffer, &mut self.ifft_out_buffer)
            .unwrap();

        // Normalize
        let scale = 1.0 / self.fft_size as f32;
        for sample in self.ifft_out_buffer.iter_mut() {
            *sample *= scale;
        }

        // Add overlap from previous block
        for i in 0..self.fft_size {
            self.ifft_out_buffer[i] += self.overlap_buffer[i];
        }

        // Write block to output
        buffer.copy_from_slice(&self.ifft_out_buffer[..buffer.len()]);

        // Copy any remaining samples to overlap buffer
        self.overlap_buffer.fill(0.0);
        for i in buffer.len()..self.fft_size {
            self.overlap_buffer[i - buffer.len()] = self.ifft_out_buffer[i];
        }
    }

    pub fn reset(&mut self) {
        self.input_samples_buffer.fill(0.0);
        self.overlap_buffer.fill(0.0);
        self.input_frequency_bins_buffer.fill(Complex::default());
        self.ifft_out_buffer.fill(0.0);
    }
}


#[derive(Clone)]
/// Partitioned IR Convolver using Uniformly Partitioned Overlap-Save (UPOLS)
pub struct PartitionedIRConvolver {
    partition_size: usize,
    ir_partition_freqs: Vec<Vec<Complex<f32>>>, // Chunks of H_n, each size (P + 1)
    fft: Arc<dyn RealToComplex<f32>>,
    ifft: Arc<dyn ComplexToReal<f32>>,

    // if the input buffer is not a multiple of the partition size, there will be unprocessed samples
    // we store them here until we have enough to process a full partition
    // the first call where this happens will return return partial silence
    partition_chunk_buffer: Vec<f32>,
    output_buffer: VecDeque<f32>,

    time_input_history: Vec<f32>, 
    fft_input_buffer: Vec<f32>,
    scratch_buffer: Vec<Complex<f32>>,
    
    // Frequency delay line holding `num_partitions` spectra of size (P + 1)
    input_frequency_domain_delay_line: VecDeque<Vec<Complex<f32>>>,
    
    input_fft_out_buffer: Vec<Complex<f32>>,
    frequency_domain_computation_result_buffer: Vec<Complex<f32>>,
    ifft_out_buffer: Vec<f32>,
}

impl PartitionedIRConvolver {
    fn best_partition_size(max_block_size: usize) -> usize {
        let desired_partition_size = max_block_size.next_power_of_two();
        desired_partition_size.clamp(64, 2048)
    }

    fn fft_size(partition_size: usize) -> usize {
        partition_size * 2
    }

    pub fn new(ir: &[f32], max_block_size: usize, partition_size: Option<usize>) -> Self {
        let partition_size = partition_size.unwrap_or_else(|| Self::best_partition_size(max_block_size));
        let (mut num_input_partitions, num_extra_samples) = ir.len().div_rem_euclid(&partition_size);
        if num_extra_samples > 0 {
            num_input_partitions += 1;
        }
        
        let fft_size = Self::fft_size(partition_size); // N = 2P
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        let ifft = planner.plan_fft_inverse(fft_size);

        let linear_gain = compute_linear_gain(ir);
        let mut partition_sample_buffer = vec![0.0f32; fft_size];
        
        let spectrum_size = fft.complex_len(); 

        // Split the IR into chunks of size P and pad to N
        let mut ir_partition_freqs = vec![vec![Complex::default(); spectrum_size]; num_input_partitions];
        for (i, partition) in ir_partition_freqs.iter_mut().enumerate() {
            let start = i * partition_size;
            let end = ((i + 1) * partition_size).min(ir.len());

            partition_sample_buffer.fill(0.0);
            partition_sample_buffer[..(end - start)].copy_from_slice(&ir[start..end]);
            partition_sample_buffer.iter_mut().for_each(|x| *x /= linear_gain);
            
            fft.process(&mut partition_sample_buffer, partition).unwrap();
        }

        // Initialize empty frequency delay line segments
        let mut input_frequency_domain_delay_line = VecDeque::with_capacity(num_input_partitions);
        for _ in 0..num_input_partitions {
            input_frequency_domain_delay_line.push_back(vec![Complex::default(); spectrum_size]);
        }

        PartitionedIRConvolver {
            partition_size,
            ir_partition_freqs,
            ifft_out_buffer: ifft.make_output_vec(),
            input_fft_out_buffer: fft.make_output_vec(),
            input_frequency_domain_delay_line,
            fft,
            ifft,
            fft_input_buffer: vec![0.0; fft_size],
            time_input_history: vec![0.0; fft_size],
            frequency_domain_computation_result_buffer: vec![Complex::default(); spectrum_size],
            scratch_buffer: vec![Complex::default(); fft_size],
            partition_chunk_buffer: Vec::with_capacity(partition_size),
            output_buffer: VecDeque::with_capacity(partition_size),
        }
    }

    pub fn process(&mut self, buffer: &mut [f32]) {
        let mut input_index = 0;

        while input_index < buffer.len() {
            let remaining_space = self.partition_size - self.partition_chunk_buffer.len();
            let remaining_input = buffer.len() - input_index;
            let to_take = remaining_space.min(remaining_input);

            self.partition_chunk_buffer
                .extend_from_slice(&buffer[input_index..input_index + to_take]);

            input_index += to_take;

            if self.partition_chunk_buffer.len() == self.partition_size {
                self.process_exact_partition_chunk();

                self.output_buffer
                    .extend(self.partition_chunk_buffer.iter().copied());

                self.partition_chunk_buffer.clear();
            }
        }

        for sample in buffer.iter_mut() {
            *sample = self.output_buffer.pop_front().unwrap_or(0.0);
        }
    }

    fn process_exact_partition_chunk(&mut self) {
        let b = self.partition_chunk_buffer.len();
        debug_assert!(b == self.partition_size, "Buffer size exceeds partition size.");

        let fft_size = Self::fft_size(self.partition_size);

        // Shift the time-domain history left by B samples
        self.time_input_history.copy_within(b.., 0);
        
        // Append the new incoming chunk of B samples to the right side
        let history_insert_idx = fft_size - b;
        self.time_input_history[history_insert_idx..].copy_from_slice(&self.partition_chunk_buffer);

        // Compute the FFT of the historical lookback window
        self.fft_input_buffer.copy_from_slice(&self.time_input_history);
        self.fft.process_with_scratch(
            &mut self.fft_input_buffer,
            &mut self.input_fft_out_buffer,
            &mut self.scratch_buffer
        ).unwrap();

        // Update the frequency-domain delay line
        let mut old_block = self.input_frequency_domain_delay_line.pop_back().expect("Frequency domain delay line should not be empty");
        // Reuse the old block's memory
        old_block.copy_from_slice(&self.input_fft_out_buffer);
        self.input_frequency_domain_delay_line.push_front(old_block);

        // Multiply each IR chunk H_n with spectrum b_n and sum all convolutions
        self.frequency_domain_computation_result_buffer.fill(Complex::default());
        for (i, ir_partition) in self.ir_partition_freqs.iter().enumerate() {
            let block_spectrum = &self.input_frequency_domain_delay_line[i];
            
            for bin in 0..self.frequency_domain_computation_result_buffer.len() {
                self.frequency_domain_computation_result_buffer[bin] += block_spectrum[bin] * ir_partition[bin];
            }
        }

        // Take the IFFT of the sum to get the time-domain result
        self.ifft.process_with_scratch(
            &mut self.frequency_domain_computation_result_buffer,
            &mut self.ifft_out_buffer,
            &mut self.scratch_buffer
        ).unwrap();

        // Normalize
        let scale = 1.0 / (fft_size as f32);
        for sample in self.ifft_out_buffer.iter_mut() {
            *sample *= scale;
        }

        // Discard the first (N - B) samples containing time aliased data
        // Copy the last B samples back to the partition_chunk_buffer for output
        let valid_output_start = fft_size - b;
        self.partition_chunk_buffer.copy_from_slice(&self.ifft_out_buffer[valid_output_start..fft_size]);
    }

    pub fn reset(&mut self) {
        self.time_input_history.fill(0.0);
        self.fft_input_buffer.fill(0.0);
        self.input_frequency_domain_delay_line.iter_mut().for_each(|v| v.fill(Complex::default()));
        self.frequency_domain_computation_result_buffer.fill(Complex::default());
        self.ifft_out_buffer.fill(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! assert_nearly_eq_array {
        ($a:expr, $b:expr, $epsilon:expr) => {
            for (i, (x, y)) in $a.iter().zip($b.iter()).enumerate() {
                assert!(
                    (x - y).abs() < $epsilon,
                    "Arrays differ at index {}: {} vs {}",
                    i,
                    x,
                    y
                );
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

        // The IR is normalized by its RMS gain, which is 0.5 for this input.
        for (i, e) in input.iter().zip(ir.iter()) {
            assert!((i - 2.0 * e).abs() < 1e-6, "Expected {}, got {}", 2.0 * e, i);
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

        // The IR is normalized by its RMS gain before convolution.
        let expected = vec![1.4142135, 0.0, 0.0, 1.4142135];
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
            vec![1.4048787, 1.9668302, 2.8097575, 1.4048787],
            1e-6
        );

        convolver.process(&mut input_block2);

        assert_nearly_eq_array!(
            input_block2,
            vec![0.8429272, 0.0, 4.2146363, 3.090733],
            1e-6
        );

        convolver.process(&mut input_block3);

        assert_nearly_eq_array!(
            input_block3,
            vec![3.090733, 0.8429272, 0.0, 1.4048787],
            1e-6
        );
    }

    #[test]
    fn partitioned_ir_convolver_passes_an_identity_ir() {
        let mut convolver = PartitionedIRConvolver::new(&[1.0], 2, Some(2));
        let mut block = vec![0.25, -0.5];

        convolver.process(&mut block);

        assert_nearly_eq_array!(block, vec![0.25, -0.5], 1e-6);
    }

    #[test]
    fn partitioned_ir_convolver_spans_multiple_partitions() {
        let mut convolver =
            PartitionedIRConvolver::new(&[1.0, 1.0, 1.0, 1.0], 2, Some(2));
        let mut first_block = vec![1.0, 0.0];
        let mut second_block = vec![0.0, 0.0];
        let mut third_block = vec![0.0, 0.0];

        convolver.process(&mut first_block);
        convolver.process(&mut second_block);
        convolver.process(&mut third_block);

        assert_nearly_eq_array!(first_block, vec![1.0, 1.0], 1e-6);
        assert_nearly_eq_array!(second_block, vec![1.0, 1.0], 1e-6);
        assert_nearly_eq_array!(third_block, vec![0.0, 0.0], 1e-6);
    }

    #[test]
    fn partitioned_ir_convolver_matches_convolution_across_blocks() {
        let ir = [0.5, 0.5, 0.5, 0.5];
        let mut convolver = PartitionedIRConvolver::new(&ir, 2, Some(2));
        let mut first_block = vec![1.0, 2.0];
        let mut second_block = vec![3.0, 4.0];
        let mut third_block = vec![5.0, 6.0];

        convolver.process(&mut first_block);
        convolver.process(&mut second_block);
        convolver.process(&mut third_block);

        assert_nearly_eq_array!(first_block, vec![1.0, 3.0], 1e-6);
        assert_nearly_eq_array!(second_block, vec![6.0, 10.0], 1e-6);
        assert_nearly_eq_array!(third_block, vec![14.0, 18.0], 1e-6);
    }
}
