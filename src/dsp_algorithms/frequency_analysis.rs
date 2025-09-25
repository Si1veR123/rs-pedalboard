use egui_plot::PlotPoint;
use realfft::{RealFftPlanner, RealToComplex};
use num_complex::Complex32;
use std::sync::Arc;

#[derive(Clone)]
pub struct FrequencyAnalyser {
    sample_rate: f32,
    min_freq: f32,
    max_freq: f32,
    num_bins: usize,

    fft: Arc<dyn RealToComplex<f32>>,
    scratch: Vec<Complex32>,
    output: Vec<Complex32>,
    input: Vec<f32>,
}

impl FrequencyAnalyser {
    pub fn new(sample_rate: f32, min_freq: f32, mut max_freq: f32, num_bins: usize, oversample: f32) -> Self {
        if max_freq > sample_rate * 0.5 {
            tracing::warn!("FrequencyAnalyser: max_freq is greater than Nyquist frequency, clamping to Nyquist");
            max_freq = sample_rate * 0.5;
        }

        let bin_size = ((max_freq - min_freq) / num_bins as f32) / oversample;
        let fft_size = ((sample_rate / bin_size).ceil() as usize).next_power_of_two();

        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        let output = fft.make_output_vec();
        let input = fft.make_input_vec();
        let scratch = fft.make_scratch_vec();

        Self {
            sample_rate,
            min_freq,
            max_freq,
            scratch,
            fft,
            output,
            input,
            num_bins,
        }
    }

    pub fn push_samples(&mut self, samples: &[f32]) {
        self.input.extend_from_slice(samples);
        if self.input.len() > self.fft.len() {
            // Keep only most recent fft_size samples
            self.input.drain(..self.input.len() - self.fft.len());
        }
    }

    pub fn analyse_log2(&mut self, amplitude_output: &mut Vec<PlotPoint>) -> bool {
        if self.input.len() != self.fft.len() {
            return false;
        }

        amplitude_output.clear();

        self.fft
            .process_with_scratch(&mut self.input, &mut self.output, &mut self.scratch)
            .expect("Buffers and input should be correct");

        let bin_width = self.sample_rate / self.fft.len() as f32;
    
        let log2_min = self.min_freq.log2();
        let log2_max = self.max_freq.log2();
        let log2_step = (log2_max - log2_min) / self.num_bins as f32;
    
        for bin in 0..self.num_bins {
            let log2_f = log2_min + bin as f32 * log2_step;
            let freq_center = 2f32.powf(log2_f);
            let fft_bin_index = (freq_center / bin_width).round() as usize;
    
            if fft_bin_index < self.output.len() {
                amplitude_output.push(PlotPoint::new(log2_f, self.output[fft_bin_index].norm()))
            } else {
                tracing::warn!("FrequencyAnalyser: FFT bin index out of range");
                amplitude_output.push(PlotPoint::new(log2_f, 0.0));
            }
        }

        return true
    }
}