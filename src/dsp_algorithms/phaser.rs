use crate::dsp_algorithms::oscillator::Oscillator;

/// Number of all-pass stages in the filter cascade.
///
/// Four stages is the classic analogue phaser voicing (two moving notches).
const STAGES: usize = 4;

/// Lowest frequency the all-pass stages sweep to, in Hz.
pub const SWEEP_LOW_HZ: f32 = 100.0;
/// Highest frequency the all-pass stages sweep to, in Hz.
pub const SWEEP_HIGH_HZ: f32 = 4000.0;

/// Single first-order all-pass filter stage.
///
/// Transfer function: `H(z) = (a + z^-1) / (1 + a * z^-1)`
#[derive(Clone, Default)]
struct AllPassStage {
    x1: f32,
    y1: f32,
}

impl AllPassStage {
    /// Process a single sample with all-pass coefficient `a`.
    fn process(&mut self, input: f32, a: f32) -> f32 {
        let output = a * input + self.x1 - a * self.y1;
        self.x1 = input;
        self.y1 = output;
        output
    }

    fn reset(&mut self) {
        self.x1 = 0.0;
        self.y1 = 0.0;
    }
}

/// Coefficient for a first-order all-pass that shifts phase by `-90` degrees at
/// `freq_hz`.
///
/// The analogue prototype `H(s) = (1 - s / wc) / (1 + s / wc)` becomes
/// `H(z) = (a + z^-1) / (1 + a * z^-1)` with `a = (t - 1) / (t + 1)` under the
/// prewarped bilinear transform (`t = tan(pi * freq / sample_rate)`), so the
/// coefficient runs from `-1` at DC to `+1` at Nyquist and the phase passes through
/// `-90` degrees at exactly the requested frequency.
///
/// The opposite sign (which is what this used to do) flips the pole and the zero,
/// mirroring the filter around Nyquist: asking for a 100 Hz corner then puts the real
/// corner near 6 kHz, which parks every notch of the cascade up in the top octave and
/// makes the sweep inaudible.
fn all_pass_coefficient(freq_hz: f32, sample_rate: f32) -> f32 {
    let max_freq = (sample_rate * 0.5) * 0.99;
    let freq = freq_hz.clamp(1.0, max_freq);
    let t = (std::f32::consts::PI * freq / sample_rate).tan();
    (t - 1.0) / (t + 1.0)
}

/// An all-pass based phaser with an LFO swept filter cascade, feedback and
/// dry/wet blend, modelled after classic analogue phaser pedals.
#[derive(Clone)]
pub struct Phaser {
    stages: [AllPassStage; STAGES],
    pub oscillator: Oscillator,
    /// Lowest frequency of the LFO sweep in Hz.
    sweep_low_hz: f32,
    /// Highest frequency of the LFO sweep in Hz.
    sweep_high_hz: f32,
    /// Sweep depth in `[0, 1]`, scaling how far the LFO moves the filter.
    width: f32,
    feedback: f32,
    blend: f32,
    last_stage_output: f32,
    sample_rate: f32,
}

impl Phaser {
    const MAX_FEEDBACK: f32 = 0.95;

    fn validated_feedback(feedback: f32) -> f32 {
        if feedback.is_finite() {
            feedback.clamp(0.0, Self::MAX_FEEDBACK)
        } else {
            0.0
        }
    }

    pub fn new(
        sweep_low_hz: f32,
        sweep_high_hz: f32,
        width: f32,
        feedback: f32,
        blend: f32,
        oscillator: Oscillator,
        sample_rate: f32,
    ) -> Self {
        Phaser {
            stages: std::array::from_fn(|_| AllPassStage::default()),
            oscillator,
            sweep_low_hz,
            sweep_high_hz: sweep_high_hz.max(sweep_low_hz),
            width: width.clamp(0.0, 1.0),
            feedback: Self::validated_feedback(feedback),
            blend: blend.clamp(0.0, 1.0),
            last_stage_output: 0.0,
            sample_rate,
        }
    }

    pub fn process_audio(&mut self, buffer: &mut [f32]) {
        let feedback = Self::validated_feedback(self.feedback);
        let blend = self.blend.clamp(0.0, 1.0);
        let ratio = (self.sweep_high_hz / self.sweep_low_hz).max(1.0);

        for sample in buffer.iter_mut() {
            let dry = *sample;

            // Bipolar LFO in [-1, 1] mapped to [0, 1]
            let lfo = (self.oscillator.next().unwrap_or(0.0) + 1.0) * 0.5;

            // Exponential sweep from the low frequency up to a width-scaled high frequency
            let freq = self.sweep_low_hz * ratio.powf(self.width * lfo);
            let a = all_pass_coefficient(freq, self.sample_rate);

            // Feed the previous output of the cascade back into the input for resonance
            let mut stage_input = dry + feedback * self.last_stage_output;
            for stage in self.stages.iter_mut() {
                stage_input = stage.process(stage_input, a);
            }

            let wet = stage_input;
            self.last_stage_output = wet;
            *sample = dry * (1.0 - blend) + wet * blend;
        }
    }

    pub fn set_rate(&mut self, rate_hz: f32) {
        self.oscillator.set_frequency(rate_hz);
    }

    pub fn set_width(&mut self, width: f32) {
        self.width = width.clamp(0.0, 1.0);
    }

    pub fn set_feedback(&mut self, feedback: f32) {
        self.feedback = Self::validated_feedback(feedback);
    }

    pub fn set_blend(&mut self, blend: f32) {
        self.blend = blend.clamp(0.0, 1.0);
    }

    pub fn reset(&mut self) {
        for stage in self.stages.iter_mut() {
            stage.reset();
        }
        self.last_stage_output = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dsp_algorithms::oscillator::Sine;

    const SAMPLE_RATE: f32 = 48_000.0;

    fn lfo() -> Oscillator {
        Oscillator::Sine(Sine::new(SAMPLE_RATE, 0.5, 0.0, 0.0))
    }

    fn test_phaser() -> Phaser {
        Phaser::new(
            SWEEP_LOW_HZ,
            SWEEP_HIGH_HZ,
            0.7,
            0.3,
            0.5,
            lfo(),
            SAMPLE_RATE,
        )
    }

    fn rms(signal: &[f32]) -> f64 {
        (signal.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / signal.len() as f64).sqrt()
    }

    /// Level of the phaser at `frequency` in dB, measured with a steady sine of unit
    /// amplitude. `cycles` whole periods are rendered twice, the first half is warm up
    /// and only the second half is measured, so the bin lands exactly on the probe.
    fn response_db(phaser: &mut Phaser, frequency: f32, cycles: usize) -> f64 {
        let samples = (cycles as f32 * SAMPLE_RATE / frequency).round() as usize;
        let mut buffer: Vec<f32> = (0..samples * 2)
            .map(|i| (2.0 * std::f32::consts::PI * frequency * i as f32 / SAMPLE_RATE).sin())
            .collect();

        phaser.reset();
        phaser.process_audio(&mut buffer);

        // The level of a sine of unit amplitude.
        const UNIT_SINE_RMS: f64 = std::f64::consts::FRAC_1_SQRT_2;
        20.0 * (rms(&buffer[samples..]) / UNIT_SINE_RMS).log10()
    }

    /// Frequency and level of the deepest point of the response between `low` and
    /// `high`. The logarithmic grid is refined around the best point of each pass, so
    /// the search lands inside a narrow notch instead of stepping over it.
    fn deepest_notch(phaser: &mut Phaser, low: f32, high: f32) -> (f32, f64) {
        let (mut low, mut high) = (low, high);
        let mut deepest = (low, f64::INFINITY);
        for _ in 0..4 {
            let ratio = high / low;
            let mut best = (low, f64::INFINITY);
            for step in 0..=12 {
                let frequency = low * ratio.powf(step as f32 / 12.0);
                let level = response_db(phaser, frequency, 12);
                if level < best.1 {
                    best = (frequency, level);
                }
            }
            deepest = best;
            let quarter = ratio.powf(0.25);
            low = best.0 / quarter;
            high = best.0 * quarter;
        }
        deepest
    }

    /// Phase of one all-pass stage at `frequency`, measured by correlating the steady
    /// state output with the input sine and cosine.
    fn measure_phase(stage: &mut AllPassStage, frequency: f32) -> f32 {
        let omega = 2.0 * std::f32::consts::PI * frequency / SAMPLE_RATE;
        let samples = (32.0 * SAMPLE_RATE / frequency).round() as usize;
        let coefficient = all_pass_coefficient(frequency, SAMPLE_RATE);

        // Warm the stage up first so the measurement only sees the steady state.
        for n in 0..samples {
            stage.process((omega * n as f32).sin(), coefficient);
        }

        let (mut correlation, mut quadrature) = (0.0f64, 0.0f64);
        for n in samples..samples * 2 {
            let output = stage.process((omega * n as f32).sin(), coefficient) as f64;
            correlation += output * (omega * n as f32).cos() as f64;
            quadrature += output * (omega * n as f32).sin() as f64;
        }

        correlation.atan2(quadrature) as f32
    }

    #[test]
    fn all_pass_coefficient_runs_from_minus_one_to_one() {
        // DC puts the pole of the all-pass on the unit circle at z = +1, Nyquist puts
        // it at z = -1, and a quarter of the sample rate sits exactly in between.
        assert!(all_pass_coefficient(1.0, SAMPLE_RATE) < -0.99);
        assert!(all_pass_coefficient(SAMPLE_RATE / 4.0, SAMPLE_RATE).abs() < 1e-6);
        // Nyquist itself is clamped just short of the unit circle, where the
        // coefficient tops out around 0.97.
        assert!(all_pass_coefficient(SAMPLE_RATE / 2.0, SAMPLE_RATE) > 0.9);

        let mut previous = f32::NEG_INFINITY;
        for frequency in [10.0, 100.0, 500.0, 1000.0, 4000.0, 12000.0, 20000.0] {
            let coefficient = all_pass_coefficient(frequency, SAMPLE_RATE);
            assert!(
                coefficient > previous,
                "{frequency} Hz gave {coefficient}, which is not above {previous}"
            );
            previous = coefficient;
        }
    }

    #[test]
    fn a_stage_shifts_its_corner_frequency_by_ninety_degrees() {
        for frequency in [200.0, 1000.0, 3000.0] {
            let mut stage = AllPassStage::default();
            let phase = measure_phase(&mut stage, frequency);
            assert!(
                (phase + std::f32::consts::FRAC_PI_2).abs() < 0.05,
                "{frequency} Hz came out {phase} rad instead of -pi/2"
            );
        }
    }

    #[test]
    fn the_cascade_notches_the_band_the_sweep_covers() {
        // Width 0 pins the sweep at its lowest frequency, and without feedback the
        // notches of a four stage cascade sit where the total all-pass phase reaches
        // -180 and -540 degrees: fc * tan(22.5 deg) and fc * tan(67.5 deg).
        let mut phaser = Phaser::new(
            SWEEP_LOW_HZ,
            SWEEP_HIGH_HZ,
            0.0,
            0.0,
            0.5,
            lfo(),
            SAMPLE_RATE,
        );

        let (low_notch, low_db) = deepest_notch(&mut phaser, 20.0, 80.0);
        let (high_notch, high_db) = deepest_notch(&mut phaser, 150.0, 400.0);

        assert!(
            low_db < -12.0,
            "lowest notch only reached {low_db:.1} dB at {low_notch:.1} Hz"
        );
        assert!(
            high_db < -12.0,
            "highest notch only reached {high_db:.1} dB at {high_notch:.1} Hz"
        );
        assert!(
            (low_notch - SWEEP_LOW_HZ * (std::f32::consts::FRAC_PI_8).tan()).abs() < 6.0,
            "lowest notch landed at {low_notch:.1} Hz"
        );
        assert!(
            (high_notch - SWEEP_LOW_HZ * (3.0 * std::f32::consts::FRAC_PI_8).tan()).abs() < 30.0,
            "highest notch landed at {high_notch:.1} Hz"
        );

        // Right at the corner the four stages add up to a whole turn, so dry and wet
        // arrive in phase and the response is flat rather than notched.
        let corner_db = response_db(&mut phaser, SWEEP_LOW_HZ, 64);
        assert!(corner_db > -1.0, "corner came out {corner_db:.1} dB");
    }

    #[test]
    fn the_lfo_sweeps_the_notches_across_a_steady_tone() {
        // The whole point of the pedal: a steady tone has to breathe. 500 Hz is
        // crossed by the notches of the sweep (100 Hz to 1.3 kHz at a width of 0.7) as
        // the LFO runs, so the level of the output cannot sit still.
        let mut phaser = Phaser::new(
            SWEEP_LOW_HZ,
            SWEEP_HIGH_HZ,
            0.7,
            0.3,
            0.5,
            lfo(),
            SAMPLE_RATE,
        );

        const PROBE_HZ: f32 = 500.0;
        const WINDOW: usize = 2_400; // 50 ms, a fortieth of the LFO cycle
        let samples = SAMPLE_RATE as usize * 2; // Two full LFO cycles at 0.5 Hz.

        let mut buffer: Vec<f32> = (0..samples)
            .map(|i| (2.0 * std::f32::consts::PI * PROBE_HZ * i as f32 / SAMPLE_RATE).sin())
            .collect();
        phaser.process_audio(&mut buffer);

        // Skip the first window so the filters are past their warm up.
        let levels: Vec<f64> = buffer.chunks(WINDOW).skip(1).map(rms).collect();
        let quietest = levels.iter().cloned().fold(f64::INFINITY, f64::min);
        let loudest = levels.iter().cloned().fold(0.0, f64::max);

        assert!(
            loudest / quietest > 2.0,
            "the sweep only moved the level between {quietest} and {loudest}"
        );
    }

    #[test]
    fn feedback_is_clamped_and_validated() {
        let mut phaser = test_phaser();
        phaser.set_feedback(5.0);
        assert_eq!(phaser.feedback, Phaser::MAX_FEEDBACK);
        phaser.set_feedback(f32::NAN);
        assert_eq!(phaser.feedback, 0.0);
    }

    #[test]
    fn silent_input_stays_silent() {
        let mut phaser = test_phaser();
        let mut buffer = [0.0; 256];
        phaser.process_audio(&mut buffer);
        assert!(buffer.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn reset_clears_internal_state() {
        let mut phaser = test_phaser();
        let mut buffer = [0.5; 64];
        phaser.process_audio(&mut buffer);
        phaser.reset();
        let mut silence = [0.0; 64];
        phaser.process_audio(&mut silence);
        assert!(silence.iter().all(|sample| *sample == 0.0));
    }
}
