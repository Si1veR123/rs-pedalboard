use egui_plot::PlotPoint;
use num_complex::Complex32;
use realfft::{RealFftPlanner, RealToComplex};
use std::ops::Range;
use std::sync::Arc;

/// The level, in dBFS, at which the analyser stops reporting quieter bands. The live
/// frequency plot maps this onto the bottom of its graph and the processor clamps every
/// magnitude here, so silent bands stay finite values rather than -inf or NaN (which
/// serde_json would write as `null`) and can always be drawn.
pub const SPECTRUM_FLOOR_DB: f64 = -60.0;

/// Added to a band's magnitude before the dB conversion, so an empty band reads as the
/// silence floor rather than as -inf.
const SILENCE_FLOOR: f32 = 1e-9;

/// Multiplier that undoes the Hann window's coherent gain, keeping the reported magnitudes
/// proportional to the amplitude of the signal.
const WINDOW_COHERENT_GAIN: f32 = 2.0;

/// The equivalent noise bandwidth of the Hann window, in FFT bins. The window spreads a
/// tone's power over three bins, so dividing a band's summed power by this keeps a full
/// scale sine at 0 dBFS without changing the level of broadband content.
const WINDOW_NOISE_BANDWIDTH: f32 = 1.5;

#[derive(Clone)]
pub struct FrequencyAnalyser {
    min_freq: f32,
    max_freq: f32,
    num_bins: usize,

    fft: Arc<dyn RealToComplex<f32>>,
    scratch: Vec<Complex32>,
    output: Vec<Complex32>,
    input: Vec<f32>,
    /// The Hann window applied before the FFT, so a loud band's spectral leakage doesn't
    /// smear across its neighbours.
    window: Vec<f32>,
    /// A windowed copy of `input` for the FFT to overwrite, leaving the rolling buffer
    /// that `push_samples` fills alone.
    windowed: Vec<f32>,
    /// The FFT bins summed into each output bin, worked out once at construction.
    band_bins: Vec<Range<usize>>,
    /// Turns a windowed FFT magnitude into a single sided amplitude, so a band holding a
    /// full scale sine reads 0 dBFS and every other band reads the level of the signal
    /// inside it.
    magnitude_scale: f32,
}

impl FrequencyAnalyser {
    pub fn new(
        sample_rate: f32,
        min_freq: f32,
        mut max_freq: f32,
        num_bins: usize,
        oversample: f32,
    ) -> Self {
        if max_freq > sample_rate * 0.5 {
            tracing::warn!("FrequencyAnalyser: max_freq is greater than Nyquist frequency, clamping to Nyquist");
            max_freq = sample_rate * 0.5;
        }

        let bin_size = ((max_freq - min_freq) / num_bins as f32) / oversample;
        let fft_size = ((sample_rate / bin_size).ceil() as usize).next_power_of_two();

        let mut planner = RealFftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(fft_size);
        let output = fft.make_output_vec();
        // Started empty rather than with `make_input_vec`, which comes back full of silence
        // and would let the first analysis run before a real window of samples has arrived
        let input = Vec::with_capacity(fft_size);
        let windowed = fft.make_input_vec();
        let scratch = fft.make_scratch_vec();
        let window = Self::hann_window(fft_size);
        let band_bins = Self::band_bins(
            min_freq,
            max_freq,
            num_bins,
            sample_rate,
            fft_size,
            output.len(),
        );

        // A sine of amplitude A leaves |X| = A * fft_size / 2 in its bin once the window,
        // whose coherent gain is 1.0, has been applied, so this turns that magnitude back
        // into A
        let magnitude_scale = 2.0 / fft_size as f32;

        Self {
            min_freq,
            max_freq,
            scratch,
            fft,
            output,
            input,
            window,
            windowed,
            band_bins,
            magnitude_scale,
            num_bins,
        }
    }

    /// A Hann window scaled by its coherent gain, so the windowing itself doesn't change
    /// the magnitude a band reports.
    fn hann_window(fft_size: usize) -> Vec<f32> {
        (0..fft_size)
            .map(|sample| {
                let phase = std::f32::consts::TAU * sample as f32 / fft_size as f32;
                (0.5 - 0.5 * phase.cos()) * WINDOW_COHERENT_GAIN
            })
            .collect()
    }

    /// The range of FFT bins to sum for every output bin, worked out once here rather than
    /// on every analysis. Each range holds at least one FFT bin, so bands narrower than the
    /// FFT's resolution share a bin instead of being skipped.
    fn band_bins(
        min_freq: f32,
        max_freq: f32,
        num_bins: usize,
        sample_rate: f32,
        fft_size: usize,
        num_fft_bins: usize,
    ) -> Vec<Range<usize>> {
        let bin_width = sample_rate / fft_size as f32;
        let log2_min = min_freq.log2();
        let log2_step = (max_freq.log2() - log2_min) / num_bins as f32;
        let last_fft_bin = num_fft_bins.saturating_sub(1);

        (0..num_bins)
            .map(|bin| {
                let band_start = 2f32.powf(log2_min + bin as f32 * log2_step);
                let band_end = 2f32.powf(log2_min + (bin + 1) as f32 * log2_step);
                let first = ((band_start / bin_width).floor() as usize).min(last_fft_bin);
                let end = ((band_end / bin_width).ceil() as usize).clamp(first + 1, num_fft_bins);

                first..end
            })
            .collect()
    }

    pub fn push_samples(&mut self, samples: &[f32]) {
        self.input.extend_from_slice(samples);
        if self.input.len() > self.fft.len() {
            // Keep only most recent fft_size samples
            self.input.drain(..self.input.len() - self.fft.len());
        }
    }

    /// Analyse the newest samples and fill `amplitude_output` with one point per output
    /// bin: its centre frequency as `log2(Hz)` and its level in dBFS, where 0 dBFS is the
    /// amplitude of the loudest sine that fits in the samples. Every level is clamped to
    /// [`SPECTRUM_FLOOR_DB`], so the result is always finite and can be serialized. Returns
    /// false until a full window of samples has been pushed.
    pub fn analyse_log2(&mut self, amplitude_output: &mut Vec<PlotPoint>) -> bool {
        if self.input.len() != self.fft.len() {
            return false;
        }

        amplitude_output.clear();

        for (windowed, (sample, window)) in self
            .windowed
            .iter_mut()
            .zip(self.input.iter().zip(self.window.iter()))
        {
            *windowed = sample * window;
        }

        self.fft
            .process_with_scratch(&mut self.windowed, &mut self.output, &mut self.scratch)
            .expect("Buffers and input should be correct");

        let log2_min = self.min_freq.log2();
        let log2_step = (self.max_freq.log2() - log2_min) / self.num_bins as f32;
        let output = &self.output;

        for (bin, band_bins) in self.band_bins.iter().enumerate() {
            let log2_f = log2_min + bin as f32 * log2_step;

            // Sum the power of every FFT bin in the band, rather than reading a single one,
            // so the band reports the level of the signal inside it: a tone still reads its
            // own amplitude, because all of its power lands in one bin, while broadband
            // content is measured over the whole band and so keeps the same level however
            // much the band widens with frequency
            let total_power = band_bins
                .clone()
                .map(|fft_bin| output[fft_bin].norm_sqr())
                .sum::<f32>();

            let magnitude = (total_power / WINDOW_NOISE_BANDWIDTH).sqrt() * self.magnitude_scale;

            // The processor sends dB rather than linear magnitudes because the payload is
            // rounded to two decimals, which would flatten away everything below -46 dB
            let db = (20.0 * (magnitude + SILENCE_FLOOR).log10()) as f64;

            amplitude_output.push(PlotPoint::new(log2_f, db.max(SPECTRUM_FLOOR_DB)));
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The settings the graphic EQ builds its analyser with, so these tests cover the sizes
    /// that are actually running.
    const SAMPLE_RATE: f32 = 48_000.0;
    const NUM_BINS: usize = 80;
    const OVERSAMPLE: f32 = 10.0;
    const MIN_FREQ_HZ: f32 = 30.0;
    const MAX_FREQ_HZ: f32 = 16_000.0;

    /// The FFT size those settings ask for, so a test can fill exactly one window.
    const FFT_SIZE: usize = 4096;

    /// An FFT bin well inside the analysed range, used to place test tones on a bin centre
    /// (996 Hz, where windowing doesn't spread their power over the neighbouring bins).
    const SINE_FFT_BIN: usize = 85;

    fn analyser() -> FrequencyAnalyser {
        FrequencyAnalyser::new(SAMPLE_RATE, MIN_FREQ_HZ, MAX_FREQ_HZ, NUM_BINS, OVERSAMPLE)
    }

    /// One full window of a bin centred sine of the given amplitude.
    fn sine(amplitude: f32, fft_bin: usize) -> Vec<f32> {
        (0..FFT_SIZE)
            .map(|sample| {
                let phase =
                    std::f32::consts::TAU * fft_bin as f32 * sample as f32 / FFT_SIZE as f32;

                amplitude * phase.sin()
            })
            .collect()
    }

    fn analyse(samples: &[f32]) -> Vec<PlotPoint> {
        let mut analyser = analyser();
        analyser.push_samples(samples);

        let mut amplitude_output = Vec::new();

        assert!(analyser.analyse_log2(&mut amplitude_output));

        amplitude_output
    }

    fn loudest(amplitude_output: &[PlotPoint]) -> &PlotPoint {
        amplitude_output
            .iter()
            .max_by(|a, b| a.y.total_cmp(&b.y))
            .expect("Every analyser reports its bands")
    }

    #[test]
    fn a_full_scale_sine_reads_full_scale_db() {
        let amplitude_output = analyse(&sine(1.0, SINE_FFT_BIN));
        let loudest = loudest(&amplitude_output);

        // Full scale is a sine at the top of the range, not the raw FFT magnitude it left
        // behind, so the curve lands inside the graph instead of being clipped onto its top
        assert!(loudest.y.abs() < 0.01, "{} dBFS", loudest.y);

        // ... and it is reported by the band that holds it: 85 * 48000 / 4096 Hz
        let sine_log2 = (SINE_FFT_BIN as f64 * SAMPLE_RATE as f64 / FFT_SIZE as f64).log2();
        let band_width =
            ((MAX_FREQ_HZ as f64).log2() - (MIN_FREQ_HZ as f64).log2()) / NUM_BINS as f64;

        assert!(loudest.x <= sine_log2);
        assert!(sine_log2 - loudest.x < band_width);
    }

    #[test]
    fn halving_the_amplitude_drops_the_level_by_six_db() {
        let full_scale = loudest(&analyse(&sine(1.0, SINE_FFT_BIN))).y;
        let half_scale = loudest(&analyse(&sine(0.5, SINE_FFT_BIN))).y;

        // The levels follow the signal itself, so the usual 6 dB per halving holds
        assert!(
            (full_scale - half_scale - 6.02).abs() < 0.05,
            "{} dB",
            full_scale - half_scale
        );
    }

    #[test]
    fn silence_reads_the_floor() {
        let amplitude_output = analyse(&vec![0.0; FFT_SIZE]);

        // Silent bands stay finite and drawable rather than falling to -inf or NaN
        assert!(amplitude_output
            .iter()
            .all(|point| point.y == SPECTRUM_FLOOR_DB));
    }

    #[test]
    fn analysing_before_a_full_window_does_nothing() {
        let mut analyser = analyser();
        analyser.push_samples(&vec![0.0; FFT_SIZE / 2]);

        let mut amplitude_output = vec![PlotPoint::new(0.0, 0.0)];

        assert!(!analyser.analyse_log2(&mut amplitude_output));
        assert_eq!(amplitude_output, vec![PlotPoint::new(0.0, 0.0)]);
    }
}
