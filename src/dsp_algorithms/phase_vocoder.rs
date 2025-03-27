use realfft::{RealFftPlanner, RealToComplex, ComplexToReal};
use realfft::num_complex::Complex;
use std::sync::Arc;

pub struct PhaseVocoder {
    fft_size: usize,
    hop_size: usize,
    fft: Arc<dyn RealToComplex<f32>>,
    ifft: Arc<dyn ComplexToReal<f32>>,
    
    pitch_shift: f32,

    prev_phase: Vec<f32>,
    phase_acc: Vec<f32>,

    hann_window: Vec<f32>,

    // Resusable buffers
    fft_input_frame: Vec<f32>,
    fft_output_vec: Vec<Complex<f32>>,
    scratch_buffer: Vec<Complex<f32>>,

    // The part of the buffers that was not completed last process (size of hop size)
    last_buffer_output_incomplete: Vec<f32>,
    last_buffer_input_incomplete: Vec<f32>,
}

impl PhaseVocoder {
    pub fn new(fft_size: usize, pitch_shift: f32) -> Self {
        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        let ifft = planner.plan_fft_inverse(fft_size);

        let hop_size = fft_size / 2;
        Self {
            fft_size,
            hop_size,
            fft,
            ifft,
            prev_phase: vec![0.0; fft_size / 2 + 1],
            phase_acc: vec![0.0; fft_size / 2 + 1],
            pitch_shift,
            hann_window: super::hann_window(fft_size),
            fft_input_frame: vec![0.0; fft_size],
            fft_output_vec: vec![Complex::ZERO; fft_size / 2 + 1],
            scratch_buffer: vec![Complex::ZERO; fft_size],
            last_buffer_input_incomplete: vec![0.0; hop_size],
            last_buffer_output_incomplete: vec![0.0; hop_size],
        }
    }

    pub fn process_buffer(&mut self, in_buffer: &[f32], out_buffer: &mut [f32]) {
        out_buffer.fill(0.0);

        // Copy incomplete buffer from last process
        out_buffer[..self.hop_size].copy_from_slice(&self.last_buffer_output_incomplete);

        // Process first frame that uses half of the previous frame and half of the current frame
        self.fft_input_frame[..self.hop_size].copy_from_slice(&self.last_buffer_input_incomplete);
        self.fft_input_frame[self.hop_size..].copy_from_slice(&in_buffer[..self.hop_size]);
        self.process_frame();

        for i in 0..self.fft_size {
            out_buffer[i] += self.fft_input_frame[i] * self.hann_window[i];
        }

        // Process the rest of the in_buffer
        let mut buffer_index = 0;
        while buffer_index + self.fft_size <= in_buffer.len() {
            let frame = &in_buffer[buffer_index..buffer_index + self.fft_size];

            self.fft_input_frame.copy_from_slice(frame);
            self.process_frame();

            // Overlap and add
            for i in 0..self.fft_size.min(out_buffer.len() - buffer_index - self.hop_size) {
                out_buffer[buffer_index + i + self.hop_size] += self.fft_input_frame[i] * self.hann_window[i];
            }

            buffer_index += self.hop_size;
        }

        // Save the incomplete buffer for the next process
        self.last_buffer_input_incomplete.copy_from_slice(&in_buffer[in_buffer.len() - self.hop_size..]);
        self.last_buffer_output_incomplete.copy_from_slice(&self.fft_input_frame[self.hop_size..]);
        self.last_buffer_output_incomplete.iter_mut().enumerate().for_each(|(i, s)| *s *= self.hann_window[i + self.hop_size]);
    }

    fn process_frame(&mut self) {
        // Apply Hann window
        for i in 0..self.fft_size {
            self.fft_input_frame[i] *= self.hann_window[i];
        }

        // Apply FFT
        self.fft.process_with_scratch(&mut self.fft_input_frame, &mut self.fft_output_vec, &mut self.scratch_buffer).unwrap();

        // Phase vocoder processing
        let mut magnitudes = vec![0.0; self.fft_size / 2 + 1];
        let mut phases = vec![0.0; self.fft_size / 2 + 1];

        for k in 0..=self.fft_size / 2 {
            let re = self.fft_output_vec[k].re;
            let im = self.fft_output_vec[k].im;
            magnitudes[k] = (re * re + im * im).sqrt();
            let phase = im.atan2(re);

            // Phase difference
            let delta_phase = phase - self.prev_phase[k];
            self.prev_phase[k] = phase;

            // Unwrap phase difference
            let expected_phase = (k as f32 * self.hop_size as f32 * std::f32::consts::TAU) / self.fft_size as f32;
            let phase_dev = delta_phase - expected_phase;
            let adjusted_phase_dev = phase_dev - (std::f32::consts::TAU * (phase_dev / std::f32::consts::TAU).round());

            // Accumulate phase
            self.phase_acc[k] += expected_phase + adjusted_phase_dev * self.pitch_shift;
            phases[k] = self.phase_acc[k];
        }

        // Synthesize new spectrum
        for k in 0..=self.fft_size / 2 {
            self.fft_output_vec[k] = Complex::new(
                magnitudes[k] * phases[k].cos(),
                magnitudes[k] * phases[k].sin(),
            );
        }

        let last = self.fft_output_vec.len() - 1;
        self.fft_output_vec[0].im = 0.0;
        self.fft_output_vec[last].im = 0.0;

        // Apply inverse FFT
        self.ifft.process(&mut self.fft_output_vec, &mut self.fft_input_frame).unwrap();

        // Normalize output
        let hann_correction_factor = 1.0 / (self.fft_size as f32 * 0.5); // Adjust for 50% overlap
        self.fft_input_frame.iter_mut().for_each(|s| *s *= hann_correction_factor);
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_phase_vocoder() {
        let mut phase_vocoder = PhaseVocoder::new(32, 1.0);
        let in_buffer = vec![10.0; 128];

        let mut out_buffer = vec![0.0; 128];
        phase_vocoder.process_buffer(&in_buffer, &mut out_buffer);

        println!("OUT BUFFER 1 {:?}", out_buffer);

        phase_vocoder.process_buffer(&in_buffer, &mut out_buffer);
        
        println!("OUT BUFFER 2 {:?}", out_buffer);
    }
}
