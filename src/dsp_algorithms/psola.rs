/// Work in progress

use rubato::{Resampler, FftFixedInOut};


fn cross_correlation(prev_frame: &[f32], next_frame: &[f32], window_function: &[f32]) -> usize {
    let mut max_corr = 0.0;
    let mut best_shift = 0;
    
    for k in 0..prev_frame.len() {
        let mut sum = 0.0;
        next_frame.iter()
            .zip(prev_frame.iter().skip(k))
            // Should window function skip k?
            .zip(window_function.iter())
            .for_each(|((a, b), w)| {
                sum += a * b * w;
        });

        if sum > max_corr {
            max_corr = sum;
            best_shift = k;
        }
    }

    best_shift
}

// Pitch Synchronous Overlap and Add
pub struct PsolaPitchShift {
    frame_size: usize,
    window_function: Vec<f32>,
    resampler: FftFixedInOut<f32>,
    overlap: usize,

    // Buffers
    resampled_prev_frame: Vec<f32>,
    resampled_frame_buffer: Vec<f32>,

    // The output buffer that is incomplete as it was at the end of the previous process
    incomplete_output_buffer: Vec<f32>,
}

impl PsolaPitchShift {
    pub fn new(frame_size: usize, overlap: usize, pitch_scale: f32) -> Self {
        let resampler = FftFixedInOut::new(
            frame_size,
            (frame_size as f32 / pitch_scale) as usize,
            frame_size,
            1
        ).expect("Failed to create resampler");

        let window_function = super::hann_window(resampler.output_frames_max());

        PsolaPitchShift {
            frame_size,
            overlap,
            resampled_prev_frame: vec![0.0; resampler.output_frames_max()],
            resampled_frame_buffer: vec![0.0; resampler.output_frames_max()],
            window_function,
            incomplete_output_buffer: vec![0.0; resampler.output_frames_max()],
            resampler
        }
    }

    pub fn process(&mut self, in_buffer: &[f32], out_buffer: &mut [f32]) {
        out_buffer.copy_from_slice(&self.incomplete_output_buffer);

        // Number of input samples processed
        let mut i = 0;

        let mut last_frame_start_in_output = 0;
        while i + self.frame_size < in_buffer.len() {
            let next_frame = &in_buffer[i..i + self.frame_size];

            self.resampler.process_into_buffer(
                &[next_frame],
                &mut [&mut self.resampled_frame_buffer],
                None
            ).expect("Failed to resample frame");

            let shift = cross_correlation(&self.resampled_prev_frame, &self.resampled_frame_buffer, &self.window_function);

            for (i, sample) in self.resampled_frame_buffer.iter().enumerate() {
                let index_to_start_writing = last_frame_start_in_output + shift;
                out_buffer[index_to_start_writing] = out_buffer[index_to_start_writing] + sample * self.window_function[i];
            }

            last_frame_start_in_output += shift;
            i += self.frame_size - self.overlap;
        }
    }
}