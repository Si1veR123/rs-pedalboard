use std::fmt::Display;

/// Credit to https://github.com/saresend/yin/ for some functions
use ringbuf::{traits::{Consumer, Observer}, HeapCons};

// How often in milliseconds the server should calculate and send the tuner frequency when active
pub const SERVER_UPDATE_FREQ_MS: u64 = 100;

#[derive(Debug, Clone, Copy)]
pub enum Note {
    A,
    ASharp,
    B,
    C,
    CSharp,
    D,
    DSharp,
    E,
    F,
    FSharp,
    G,
    GSharp,
}

impl Display for Note {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Note::A => write!(f, "A"),
            Note::ASharp => write!(f, "A#"),
            Note::B => write!(f, "B"),
            Note::C => write!(f, "C"),
            Note::CSharp => write!(f, "C#"),
            Note::D => write!(f, "D"),
            Note::DSharp => write!(f, "D#"),
            Note::E => write!(f, "E"),
            Note::F => write!(f, "F"),
            Note::FSharp => write!(f, "F#"),
            Note::G => write!(f, "G"),
            Note::GSharp => write!(f, "G#"),
        }
    }
}

// Get note, octave, and offset in cents
pub fn freq_to_note(freq: f32) -> (Note, isize, f32) {
    // Offset from A4 in cents
    let cents_offset = 1200.0 * (freq / 440.0).log2();
    // Offset from A4 in semitones
    let semitone_offset = (cents_offset / 100.0).round() as isize;
    // MIDI note number for A4 is 69
    let midi_note = 69 + semitone_offset;
    // Note index (0 = A, 1 = A#, ..., 11 = G#)
    let note_index = ((midi_note % 12 + 12) % 12) as u8;
    let note = match note_index {
        0 => Note::C,
        1 => Note::CSharp,
        2 => Note::D,
        3 => Note::DSharp,
        4 => Note::E,
        5 => Note::F,
        6 => Note::FSharp,
        7 => Note::G,
        8 => Note::GSharp,
        9 => Note::A,
        10 => Note::ASharp,
        11 => Note::B,
        _ => unreachable!(),
    };

    // Octave calculation: MIDI note 69 is A4, so octave = (midi_note / 12) - 1
    let octave = (midi_note / 12) - 1;
    // Offset from nearest semitone in cents
    let semitone_cents_offset = cents_offset - (semitone_offset as f32 * 100.0);
    (note, octave, semitone_cents_offset)
}

pub struct Yin {
    read_from: HeapCons<f32>,

    sample_frame_buffer: Vec<f32>,
    diff_buffer: Vec<f32>,
    cmndf_buffer: Vec<f32>,

    prev_estimation: f32,

    tau_min: usize,
    tau_max: usize,
    num_periods: usize,
    threshold: f32,
    sample_rate: usize
}

impl Yin {
    pub fn minimum_buffer_length(sample_rate: usize, freq_min: usize, num_periods: usize) -> usize {
        let tau_max = sample_rate / freq_min;
        tau_max * num_periods
    }

    pub fn new(threshold: f32, freq_min: usize, freq_max: usize, sample_rate: usize, num_periods: usize, read_from: HeapCons<f32>) -> Self {
        let min_buffer = Self::minimum_buffer_length(sample_rate, freq_min, num_periods);
        assert!(read_from.capacity().get() >= min_buffer, "Yin buffer too small: {} < {}", read_from.capacity(), min_buffer);
        
        let tau_max = sample_rate / freq_min;
        let tau_min = sample_rate / freq_max;

        log::debug!("Yin tau_max: {}, tau_min: {}", tau_max, tau_min);

        Self {
            read_from,
            sample_frame_buffer: Vec::with_capacity(tau_max),
            diff_buffer: Vec::with_capacity(tau_max),
            cmndf_buffer: Vec::with_capacity(tau_max),
            prev_estimation: 0.0,
            threshold,
            tau_max,
            tau_min,
            num_periods,
            sample_rate,
        }
    }

    pub fn process_buffer(&mut self) -> f32 {
        let occupied_samples = self.read_from.occupied_len();
        let samples_to_take = self.tau_max * self.num_periods;
        if occupied_samples >= samples_to_take {
            self.sample_frame_buffer.clear();
            self.sample_frame_buffer.extend(self.read_from.pop_iter().take(samples_to_take));

            // Normalise the buffer between -1 and 1
            if let Some(max_amplitude) = self.sample_frame_buffer.iter().map(|v| v.abs()).max_by(|a, b| a.partial_cmp(b).unwrap()) {
                if max_amplitude > 0.0 {
                    for sample in self.sample_frame_buffer.iter_mut() {
                        *sample /= max_amplitude;
                    }
                }
            }

            let freq = self.frequency_from_frame();
            self.prev_estimation = freq;
            return freq;
        } else {
            // Not enough samples
            return self.prev_estimation;
        }
    }

    fn frequency_from_frame(&mut self) -> f32 {
        self.diff_function();
        self.cmndf();
        self.compute_diff_min()
    }

    fn diff_function(&mut self) {
        self.diff_buffer.clear();
        self.diff_buffer.resize(self.tau_max, 0.0);

        debug_assert!(self.sample_frame_buffer.len() >= self.tau_max);
    
        for tau in 1..self.tau_max {
            for j in 0..(self.sample_frame_buffer.len() - self.tau_max) {
                let tmp = self.sample_frame_buffer[j] - self.sample_frame_buffer[j + tau];
                self.diff_buffer[tau] += tmp * tmp;
            }
        }
    }

    fn cmndf(&mut self) {
        let mut running_sum = 0.0;
        self.cmndf_buffer.clear();
        self.cmndf_buffer.push(1.0);
        for index in 1..self.diff_buffer.len() {
            running_sum += self.diff_buffer[index];
            if running_sum == 0.0 {
                self.cmndf_buffer.push(self.diff_buffer[index]);
            } else {
                self.cmndf_buffer.push(self.diff_buffer[index] * index as f32 / running_sum);
            }
        }
    }

    fn compute_diff_min(&mut self) -> f32 {
        if self.cmndf_buffer.len() < self.tau_min {
            return 0.0;
        }
    
        let relevant_cmndf_buffer = &self.cmndf_buffer[self.tau_min..self.tau_max];
        let (min_tau, value) = relevant_cmndf_buffer
            .iter()
            .copied()
            .enumerate()
            .min_by(|a, b| a.1.partial_cmp(&b.1).unwrap())
            .expect("Shouldn't be any NaN in Yin");
    
        if value >= self.threshold {
            0.0
        } else {
            let true_tau = self.tau_min + min_tau;
            let refined = Self::parabolic_interpolation(&self.cmndf_buffer, true_tau);
            self.sample_rate as f32 / refined
        }
    }
    

    fn parabolic_interpolation(cmndf: &[f32], tau_m: usize) -> f32 {
        if tau_m <= 0 || tau_m >= cmndf.len() - 1 {
            return tau_m as f32;
        }

        let (y_0, y_1, y_2) = (cmndf[tau_m-1], cmndf[tau_m], cmndf[tau_m + 1]);
        let denominator = 2.0 * (y_0 - 2.0 * y_1 + y_2);
        if denominator == 0.0 {
            return tau_m as f32;
        }

        let offset = (y_0 - y_2) / denominator;
        return tau_m as f32 + offset;
    }
}


#[cfg(test)]
mod tests {
    use ringbuf::{traits::{Producer, Split}, HeapRb};

    use super::*;

    #[test]
    fn test_yin() {
        let (mut prod, cons) = HeapRb::<f32>::new(Yin::minimum_buffer_length(80, 10, 3)).split();
        let mut estimator = Yin::new(0.1, 10, 30, 80, 3, cons);
        let mut example = vec![];
        let mut prev_value = -1.0;
        // Periodic over every 4 values of i, giving us a frequency of: 80 / 4 == 20
        for i in 0..80 {
            if i % 2 != 0 {
                example.push(0.0);
            } else {
                prev_value *= -1.0;
                example.push(prev_value);
            }
        }
        prod.push_slice(&example);
        let freq = estimator.process_buffer();
        assert!(freq - 20.0 < 0.5, "Yin frequency estimation failed: {} != 20.0", freq);
    }
}
