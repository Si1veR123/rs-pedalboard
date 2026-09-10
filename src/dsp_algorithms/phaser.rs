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

/// Coefficient for a first-order all-pass whose pole sits at `freq_hz`.
///
/// Produces `1.0` as the frequency approaches DC and `-1.0` as it approaches
/// Nyquist, giving a phase shift that sweeps from `0` to `-2*pi` radians.
fn all_pass_coefficient(freq_hz: f32, sample_rate: f32) -> f32 {
    let max_freq = (sample_rate * 0.5) * 0.99;
    let freq = freq_hz.clamp(1.0, max_freq);
    let t = (std::f32::consts::PI * freq / sample_rate).tan();
    (1.0 - t) / (1.0 + t)
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

    fn test_phaser() -> Phaser {
        Phaser::new(
            SWEEP_LOW_HZ,
            SWEEP_HIGH_HZ,
            0.7,
            0.3,
            0.5,
            Oscillator::Sine(Sine::new(48000.0, 0.5, 0.0, 0.0)),
            48000.0,
        )
    }

    #[test]
    fn all_pass_coefficient_spans_one_to_minus_one() {
        assert!(all_pass_coefficient(0.0, 48000.0) > 0.99);
        assert!(all_pass_coefficient(24000.0 * 0.5, 48000.0) < 0.0);
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
