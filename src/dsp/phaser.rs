use crate::dsp::oscillator::Oscillator;

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
    use crate::dsp::oscillator::Sine;

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
    fn feedback_is_clamped_and_validated() {
        let mut phaser = test_phaser();
        phaser.set_feedback(5.0);
        assert_eq!(phaser.feedback, Phaser::MAX_FEEDBACK);
        phaser.set_feedback(f32::NAN);
        assert_eq!(phaser.feedback, 0.0);
    }
}
