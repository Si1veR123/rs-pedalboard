/// Credit to https://github.com/saresend/yin/ for some functions
use ringbuf::{traits::{Consumer, Observer, Producer, Split}, HeapCons, HeapProd, HeapRb};

pub struct Yin {
    sample_buffer_prod: HeapProd<f32>,
    sample_buffer_cons: HeapCons<f32>,

    sample_frame_buffer: Vec<f32>,
    diff_buffer: Vec<f32>,
    cmndf_buffer: Vec<f32>,

    prev_estimation: f32,

    tau_min: usize,
    tau_max: usize,
    threshold: f32,
    sample_rate: usize
}

impl Yin {
    pub fn new(sample_rate: usize, freq_min: usize, freq_max: usize, threshold: f32) -> Self {
        let tau_max = sample_rate / freq_min;
        let tau_min = sample_rate / freq_max;

        let sample_buffer = HeapRb::new(tau_max*3);
        let (sample_buffer_prod, sample_buffer_cons) = sample_buffer.split();

        Self {
            sample_buffer_prod,
            sample_buffer_cons,
            sample_frame_buffer: Vec::with_capacity(tau_max),
            diff_buffer: Vec::with_capacity(tau_max),
            cmndf_buffer: Vec::with_capacity(tau_max),
            prev_estimation: 0.0,
            threshold,
            tau_max,
            tau_min,
            sample_rate,
        }
    }

    pub fn process_buffer(&mut self, buffer: &[f32]) -> f32 {
        let n = self.sample_buffer_prod.push_slice(buffer);
        if n != buffer.len() {
            log::warn("YIN can't process full buffer. Reduce size.")
        }

        let occupied_samples = self.sample_buffer_cons.occupied_len();
        if occupied_samples >= self.tau_max {
            // Get last tau_max samples
            self.sample_buffer_cons.skip(occupied_samples - self.tau_max);
            self.sample_frame_buffer.extend(self.sample_buffer_cons);
            let freq = self.frequency_from_frame(&self.sample_frame_buffer);
            self.prev_estimation = freq;
            return freq;
        } else {
            // Not enough samples
            return self.prev_estimation;
        }
    }

    fn frequency_from_frame(&mut self, buffer: &[f32]) -> f32 {
        let diff = self.diff_function(buffer);
        let cmndf = self.cmndf(diff);
        self.compute_diff_min(cmndf)
    }

    fn diff_function(&mut self, buffer: &[f32]) -> &[f32] {
        self.diff_buffer.clear();
    
        debug_assert!(audio_sample.len() >= self.tau_max);
    
        for tau in 1..self.tau_max {
            for j in 0..(audio_sample.len() - self.tau_max) {
                let tmp = audio_sample[j] - audio_sample[j + tau];
                self.diff_buffer[tau] += tmp * tmp;
            }
        }
        &self.diff_buffer
    }

    fn cmndf(&mut self, raw_diff: &[f32]) ->&[f32] {
        let mut running_sum = 0.0;
        self.cmndf_buffer.clear();
        self.cmndf_buffer.push(1.0);

        for index in 1..raw_diff.len() {
            running_sum += raw_diff[index];
            if running_sum == 0.0 {
                self.cmndf_buffer.push(raw_diff[index]);
            } else {
                self.cmndf_buffer.push(raw_diff[index] * index as f64 / running_sum);
            }
        }
    
        cmndf_diff
    }

    fn compute_diff_min(&mut self, cmndf: &[f32]) -> f32 {
        let mut tau = self.min_tau;
        while tau < self.max_tau {
            if cmndf[tau] < self.threshold {
                let refined = Self::parabolic_interpolation(cmndf, tau);
                let freq = self.sample_rate as f32 / refined;
                return freq;
            }
            tau += 1;
        }
        0.0
    }

    fn parabolic_interpolation(cmndf: &[f32], tau_m: usize) -> f32 {
        if tau_m <= 0 || tau_m >= cmndf.len() - 1 {
            return tau_m as f32;
        }

        let (y_0, y_1, y_2) = (cmndf[tau_m-1], cmndf[tau_m], cmndf[tau_m + 1]);
        let denominator = 2.0 * (y_0 - 2.0 * y_1 + y_2);
        if denominator == 0 {
            return tau_m as f32;
        }

        let offset = (y_0 - y_2) / denominator;
        return tau_m as f32 + offset;
    }
}


