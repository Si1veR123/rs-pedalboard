// Roughly modelled after a DS-1

use std::collections::HashMap;
use std::hash::Hash;

use crate::dsp_algorithms::biquad::BiquadFilter;
use crate::unique_time_id;

use super::ui::{pedal_knob, pedal_switch};
use super::PedalParameter;
use super::PedalParameterValue;
use super::PedalTrait;

use eframe::egui::Image;
use eframe::egui::{self, include_image, Vec2};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize};

/// Boss DS-1 tone stack. It is the classic Big Muff style network: the signal runs
/// through a low pass path (R15 + C11, corner at 234 Hz) and a high pass path
/// (R16 + C12, corner at 1061 Hz) at the same time, and the tone pot blends the two,
/// scooping the mids when the knob sits in the middle.
const TONE_LP_RESISTANCE: f64 = 6_800.0;
const TONE_HP_RESISTANCE: f64 = 1_500.0;
const TONE_POT_RESISTANCE: f64 = 25_000.0;
const TONE_CAPACITANCE: f64 = 100e-9;
/// How far the asymmetry knob can move the operating point of the clipper, as a
/// fraction of the clipping threshold. The diodes of the pedal are mismatched, so one
/// half of the wave saturates before the other; the further the point moves, the more
/// the duty cycle of the clipped wave shifts and the more even harmonics show up.
const ASYMMETRY_BIAS: f32 = 0.9;
/// Clipping threshold and knee of the diode pair.
const CLIP_THRESHOLD: f32 = 1.0;
const CLIP_KNEE: f32 = 5.0;
/// The coupling caps of the pedal, they remove the DC an asymmetric clipper leaves.
const DC_BLOCKER_HZ: f32 = 20.0;
/// Output stage bandwidth, keeps the clipped harmonics from getting fizzy.
const POST_FILTER_HZ: f32 = 9000.0;

#[derive(Clone)]
pub struct Distortion {
    parameters: HashMap<String, PedalParameter>,
    // Processor only
    tone_stack: Option<BiquadFilter>,
    tone_setting: f32,
    tone_makeup: f32,
    post_filter: Option<BiquadFilter>,
    dc_blocker: Option<BiquadFilter>,
    sample_rate: Option<f32>,
    id: u32,
}

impl Serialize for Distortion {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser_map = serializer.serialize_map(Some(2))?;
        ser_map.serialize_entry("id", &self.id)?;
        ser_map.serialize_entry("parameters", &self.parameters)?;
        ser_map.end()
    }
}

impl<'de> Deserialize<'de> for Distortion {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct DistortionData {
            id: u32,
            parameters: HashMap<String, PedalParameter>,
        }

        let helper = DistortionData::deserialize(deserializer)?;
        Ok(Distortion {
            id: helper.id,
            parameters: helper.parameters,
            tone_stack: None,
            tone_setting: 0.0,
            tone_makeup: 1.0,
            post_filter: None,
            dc_blocker: None,
            sample_rate: None,
        })
    }
}

impl Hash for Distortion {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Distortion {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();
        parameters.insert(
            "Drive".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(10.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(75.0)),
                step: None,
            },
        );
        parameters.insert(
            "Tone".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.5),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None,
            },
        );
        parameters.insert(
            "Level".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(3.0)),
                step: None,
            },
        );
        parameters.insert(
            "Active".to_string(),
            PedalParameter {
                value: PedalParameterValue::Bool(true),
                min: None,
                max: None,
                step: None,
            },
        );
        parameters.insert(
            "Asymmetry".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.5),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None,
            },
        );
        parameters.insert(
            "Enable EQ".to_string(),
            PedalParameter {
                value: PedalParameterValue::Bool(true),
                min: None,
                max: None,
                step: None,
            },
        );
        Distortion {
            parameters,
            tone_stack: None,
            tone_setting: 0.0,
            tone_makeup: 1.0,
            post_filter: None,
            dc_blocker: None,
            sample_rate: None,
            id: unique_time_id(),
        }
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned
    }

    /// Digital biquad coefficients of the tone stack. The network is solved by nodal
    /// analysis in the analog domain and then transformed with the bilinear
    /// transform, so the low pass and the high pass paths keep the corners of the real
    /// pedal instead of sharing a single fixed pivot frequency.
    fn tone_stack_coefficients(tone: f32, sample_rate: f32) -> ([f32; 2], [f32; 3]) {
        let g1 = 1.0 / TONE_LP_RESISTANCE;
        let g2 = 1.0 / TONE_HP_RESISTANCE;
        let gp = 1.0 / TONE_POT_RESISTANCE;
        let c = TONE_CAPACITANCE;
        let t = tone.clamp(0.0, 1.0) as f64;

        // The tone pot blends the low pass node and the high pass node, which gives
        //   H(s) = (n0 + n1 * s + n2 * s^2) / (d0 + d1 * s + c^2 * s^2)
        let n0 = g1 * ((1.0 - t) * g2 + gp);
        let n1 = (g1 + gp) * c;
        let n2 = t * c * c;
        let d0 = (g1 + gp) * (g2 + gp) - gp * gp;
        let d1 = c * (g1 + g2 + 2.0 * gp);
        let d2 = c * c;

        // A passive network loses level; the DS-1 makes that up in the stage behind it.
        // The same is done here by normalizing the peak of the response to unity, so
        // sweeping the tone knob doesn't change how loud the pedal is.
        let mut peak: f64 = 0.0;
        let top = 20_000.0f64.min(sample_rate as f64 * 0.45);
        for step in 0..=60 {
            let f = 20.0 * (top / 20.0).powf(step as f64 / 60.0);
            let w = 2.0 * std::f64::consts::PI * f;
            let num = (n0 - n2 * w * w).powi(2) + (n1 * w).powi(2);
            let den = (d0 - d2 * w * w).powi(2) + (d1 * w).powi(2);
            peak = peak.max(num / den);
        }
        let makeup = 1.0 / peak.sqrt();
        let (n0, n1, n2) = (n0 * makeup, n1 * makeup, n2 * makeup);

        // Bilinear transform: s = 2 * sample_rate * (1 - z^-1) / (1 + z^-1).
        let k = 2.0 * sample_rate as f64;
        let b0 = n0 + n1 * k + n2 * k * k;
        let b1 = 2.0 * (n0 - n2 * k * k);
        let b2 = n0 - n1 * k + n2 * k * k;
        let a0 = d0 + d1 * k + d2 * k * k;
        let a1 = 2.0 * (d0 - d2 * k * k);
        let a2 = d0 - d1 * k + d2 * k * k;

        (
            [(a1 / a0) as f32, (a2 / a0) as f32],
            [(b0 / a0) as f32, (b1 / a0) as f32, (b2 / a0) as f32],
        )
    }

    pub fn tone_stack(tone: f32, sample_rate: f32) -> BiquadFilter {
        let (a, b) = Self::tone_stack_coefficients(tone, sample_rate);
        BiquadFilter::new(a, b)
    }

    /// Gain that makes up the insertion loss of the tone stack, the job the stage behind
    /// the tone stack does in the pedal. The loss depends on the tone setting (the pot
    /// blends a low pass and a high pass path), so the response is measured at the
    /// setting in use: without this the pedal is a lot quieter with the EQ in the path
    /// than with it bypassed, which is not something the hardware does.
    pub fn tone_stack_makeup(tone: f32, sample_rate: f32) -> f32 {
        // A guitar note and its first harmonics, each weighted like the amplitude of a
        // plucked string (1 / harmonic number). The clipper fills in the harmonics above
        // this, but the fundamental still carries most of the level, so weighting the
        // bands this way keeps the makeup in step with the signal going into the pedal.
        const BANDS: [(f64, f64); 9] = [
            (110.0, 2.0),
            (220.0, 1.0),
            (330.0, 0.67),
            (440.0, 0.5),
            (660.0, 0.33),
            (880.0, 0.25),
            (1320.0, 0.17),
            (1760.0, 0.125),
            (2640.0, 0.08),
        ];

        let filter = Self::tone_stack(tone, sample_rate);
        let mut weighted = 0.0;
        let mut weight = 0.0;
        for (frequency, band_weight) in BANDS {
            weighted += filter
                .response_at_freq(frequency, sample_rate as f64)
                .norm()
                * band_weight;
            weight += band_weight;
        }

        let makeup = 1.0 / (weighted / weight);
        makeup as f32
    }

    /// Rebuilds the tone stack when the tone knob has moved, carrying the filter state
    /// over so the knob doesn't click.
    fn update_tone_stack(&mut self, tone: f32, sample_rate: f32) {
        if (tone - self.tone_setting).abs() <= f32::EPSILON {
            return;
        }

        let current = self.tone_stack.as_ref().unwrap();
        let (s1, s2) = (current.s1, current.s2);

        let mut filter = Self::tone_stack(tone, sample_rate);
        filter.s1 = s1;
        filter.s2 = s2;

        self.tone_stack = Some(filter);
        self.tone_setting = tone;
        self.tone_makeup = Self::tone_stack_makeup(tone, sample_rate);
    }

    pub fn hard_diode(x: f32, threshold: f32, knee: f32) -> f32 {
        if x > threshold {
            threshold + (x - threshold) / (1.0 + knee * (x - threshold).abs())
        } else if x < -threshold {
            -threshold + (x + threshold) / (1.0 + knee * (x + threshold).abs())
        } else {
            x
        }
    }

    /// Diode clipper whose operating point sits `bias` away from zero, which is how the
    /// mismatched diode pair of the pedal is modelled. A symmetric clipper (no bias)
    /// only produces odd harmonics, so an "asymmetry" that merely unbalances the two
    /// clipping levels stays inaudible on a heavily clipped signal - the waveform it
    /// makes is a square wave plus a DC offset. Moving the operating point instead makes
    /// one half of the wave reach the threshold before the other, which shifts the duty
    /// cycle of the clipped wave and adds the even harmonics an asymmetric stage is
    /// known for. The quiescent output of the shaper is subtracted again so the stage
    /// doesn't hand a DC offset to the tone stack; whatever is left is removed by the
    /// coupling caps behind it.
    pub fn asymmetric_diode(x: f32, bias: f32, threshold: f32, knee: f32) -> f32 {
        Self::hard_diode(x + bias, threshold, knee) - Self::hard_diode(bias, threshold, knee)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f64 = 48_000.0;

    fn db(filter: &BiquadFilter, frequency: f64) -> f64 {
        20.0 * filter.response_at_freq(frequency, SAMPLE_RATE).norm().log10()
    }

    #[test]
    fn tone_stack_is_dark_at_full_bass_and_bright_at_full_treble() {
        let bass = Distortion::tone_stack(0.0, SAMPLE_RATE as f32);
        let treble = Distortion::tone_stack(1.0, SAMPLE_RATE as f32);

        assert!(db(&bass, 100.0) > db(&bass, 4000.0) + 15.0);
        assert!(db(&treble, 4000.0) > db(&treble, 100.0) + 15.0);
    }

    #[test]
    fn tone_stack_never_boosts_and_scoops_the_mids_at_noon() {
        for tone in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let filter = Distortion::tone_stack(tone, SAMPLE_RATE as f32);
            for frequency in [50.0, 100.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0] {
                // The passive network is normalized to unity peak gain, so it can only
                // ever make the signal quieter.
                assert!(db(&filter, frequency) <= 0.5, "tone {tone} at {frequency} Hz");
            }
        }

        let noon = Distortion::tone_stack(0.5, SAMPLE_RATE as f32);
        let low = db(&noon, 100.0);
        let mid = db(&noon, 700.0);
        let high = db(&noon, 4000.0);

        assert!(mid < low - 3.0, "low {low}, mid {mid}");
        assert!(mid < high - 3.0, "high {high}, mid {mid}");
    }

    fn rms(signal: &[f32]) -> f32 {
        (signal.iter().map(|x| x * x).sum::<f32>() / signal.len() as f32).sqrt()
    }

    /// Rough stand in for a plucked string: a fundamental plus a few harmonics.
    fn guitar_input(samples: usize, fundamental: f64, sample_rate: f64) -> Vec<f32> {
        (0..samples)
            .map(|i| {
                let mut x = 0.0;
                for harmonic in 1..=6 {
                    let f = fundamental * harmonic as f64;
                    x += (2.0 * std::f64::consts::PI * f * i as f64 / sample_rate).sin()
                        / harmonic as f64;
                }
                (x * 0.25) as f32
            })
            .collect()
    }

    /// Amplitude of one frequency in `signal`, measured with a single bin DFT. Feed a
    /// whole number of periods so that the bin lands on the harmonic exactly.
    fn harmonic_amplitude(signal: &[f32], frequency: f64, sample_rate: f64) -> f64 {
        let mut real = 0.0;
        let mut imaginary = 0.0;
        for (i, &sample) in signal.iter().enumerate() {
            let phase = 2.0 * std::f64::consts::PI * frequency * i as f64 / sample_rate;
            real += sample as f64 * phase.cos();
            imaginary += sample as f64 * phase.sin();
        }
        2.0 * (real * real + imaginary * imaginary).sqrt() / signal.len() as f64
    }

    /// Runs a sine through the pedal with the EQ bypassed, so what comes out is the
    /// clipper on its own.
    fn render_clipped_sine(asymmetry: f32, samples: usize, frequency: f64, drive: f32) -> Vec<f32> {
        let mut pedal = Distortion::new();
        pedal.set_config(samples, SAMPLE_RATE as u32);
        pedal.set_parameter_value("Drive", PedalParameterValue::Float(drive));
        pedal.set_parameter_value("Asymmetry", PedalParameterValue::Float(asymmetry));
        pedal.set_parameter_value("Enable EQ", PedalParameterValue::Bool(false));

        let mut buffer: Vec<f32> = (0..samples)
            .map(|i| (2.0 * std::f64::consts::PI * frequency * i as f64 / SAMPLE_RATE).sin() as f32)
            .collect();
        let mut messages = Vec::new();
        pedal.process_audio(&mut buffer, &mut messages);
        buffer
    }

    #[test]
    fn disabling_eq_bypasses_the_tone_stack() {
        let input: Vec<f32> = (0..256).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
        let mut messages = Vec::new();

        let mut render = |tone: f32, eq_enabled: bool| {
            let mut pedal = Distortion::new();
            pedal.set_config(256, SAMPLE_RATE as u32);
            pedal.set_parameter_value("Tone", PedalParameterValue::Float(tone));
            pedal.set_parameter_value("Enable EQ", PedalParameterValue::Bool(eq_enabled));

            let mut buffer = input.clone();
            pedal.process_audio(&mut buffer, &mut messages);
            buffer
        };

        // With the EQ off the tone knob has no effect at all...
        assert_eq!(render(0.0, false), render(1.0, false));
        // ...while with the EQ on it still shapes the sound.
        assert_ne!(render(0.0, true), render(1.0, true));
    }

    #[test]
    fn eq_switch_does_not_jump_the_level() {
        let input = guitar_input(4096, 220.0, SAMPLE_RATE);
        let mut messages = Vec::new();

        let mut render = |tone: f32, eq_enabled: bool, drive: f32| {
            let mut pedal = Distortion::new();
            pedal.set_config(4096, SAMPLE_RATE as u32);
            pedal.set_parameter_value("Tone", PedalParameterValue::Float(tone));
            pedal.set_parameter_value("Drive", PedalParameterValue::Float(drive));
            pedal.set_parameter_value("Enable EQ", PedalParameterValue::Bool(eq_enabled));

            let mut buffer = input.clone();
            pedal.process_audio(&mut buffer, &mut messages);
            buffer
        };

        // The tone stack of the pedal is followed by a stage with a fixed amount of
        // gain, so bypassing the tone stack shouldn't change how loud the pedal is.
        for drive in [0.0, 10.0, 40.0, 75.0] {
            for tone in [0.0, 0.5, 1.0] {
                let with_eq = rms(&render(tone, true, drive));
                let without_eq = rms(&render(tone, false, drive));
                let difference_db = 20.0 * (with_eq / without_eq).log10();

                assert!(
                    difference_db.abs() < 3.0,
                    "drive {drive}, tone {tone}: with EQ is {difference_db:.2} dB off"
                );
            }
        }

        // At the default setting (tone at noon) the stage behind the tone stack makes
        // the insertion loss of the network up, so the two are close.
        let with_eq = rms(&render(0.5, true, 10.0));
        let without_eq = rms(&render(0.5, false, 10.0));
        assert!((with_eq / without_eq - 1.0).abs() < 0.25);
    }

    #[test]
    fn asymmetry_adds_even_harmonics_to_the_clipped_wave() {
        // 4800 samples at 48 kHz is exactly 20 periods of 200 Hz and 40 of 400 Hz, so
        // the bins used below land on the harmonics exactly. The first half of the
        // render is skipped so that the filters have settled.
        const SAMPLES: usize = 4_800;
        const FUNDAMENTAL: f64 = 200.0;
        const DRIVE: f32 = 10.0;

        let second_harmonic_db = |asymmetry: f32| {
            let output = render_clipped_sine(asymmetry, SAMPLES, FUNDAMENTAL, DRIVE);
            let settled = &output[SAMPLES / 2..];
            let fundamental = harmonic_amplitude(settled, FUNDAMENTAL, SAMPLE_RATE);
            let second = harmonic_amplitude(settled, FUNDAMENTAL * 2.0, SAMPLE_RATE);
            20.0 * (second / fundamental).log10()
        };

        // At noon the two halves of the wave clip at the same level, so the clipper only
        // makes odd harmonics.
        let symmetric = second_harmonic_db(0.5);
        assert!(symmetric < -40.0, "symmetric clipper: {symmetric}");

        // Moving the knob either way pushes the operating point of the clipper off
        // centre, which shifts the duty cycle of the clipped wave and is what makes the
        // even harmonics of an asymmetric stage show up. Both ends of the knob should do
        // the same thing, mirror image of each other.
        let biased_down = second_harmonic_db(0.0);
        let biased_up = second_harmonic_db(1.0);
        assert!(biased_down > -20.0, "bias down: {biased_down}");
        assert!(biased_up > -20.0, "bias up: {biased_up}");
        assert!((biased_down - biased_up).abs() < 0.1);
    }

    #[test]
    fn asymmetry_knob_is_not_a_level_knob_and_the_coupling_caps_block_the_dc() {
        const SAMPLES: usize = 4_800;

        let level_db = |asymmetry: f32| {
            let output = render_clipped_sine(asymmetry, SAMPLES, 200.0, 10.0);
            20.0 * rms(&output[SAMPLES / 2..]).log10()
        };
        let dc = |asymmetry: f32| {
            let output = render_clipped_sine(asymmetry, SAMPLES, 200.0, 10.0);
            output[SAMPLES / 2..].iter().sum::<f32>() / (SAMPLES / 2) as f32
        };

        // An asymmetric clipper puts out a DC offset. The stage subtracts its quiescent
        // level and the coupling caps behind it take care of the rest, so neither the
        // level nor the offset of the output should run away when the knob is turned.
        let symmetric_level = level_db(0.5);
        for asymmetry in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let difference_db = level_db(asymmetry) - symmetric_level;
            assert!(
                difference_db.abs() < 1.5,
                "asymmetry {asymmetry} moved the level by {difference_db:.2} dB"
            );
            assert!(
                dc(asymmetry).abs() < 5e-3,
                "asymmetry {asymmetry} leaves DC"
            );
        }
    }
}

impl PedalTrait for Distortion {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn set_config(&mut self, _buffer_size: usize, sample_rate: u32) {
        let sample_rate = sample_rate as f32;
        let tone = self
            .get_parameters()
            .get("Tone")
            .and_then(|p| p.value.as_float())
            .unwrap_or(0.5);

        self.tone_stack = Some(Self::tone_stack(tone, sample_rate));
        self.tone_setting = tone;
        self.tone_makeup = Self::tone_stack_makeup(tone, sample_rate);
        self.post_filter = Some(BiquadFilter::low_pass(POST_FILTER_HZ, sample_rate, 0.707));
        self.dc_blocker = Some(BiquadFilter::high_pass(DC_BLOCKER_HZ, sample_rate, 0.707));
        self.sample_rate = Some(sample_rate);
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        if self.tone_stack.is_none() || self.post_filter.is_none() || self.dc_blocker.is_none() {
            tracing::warn!("Distortion: Filters not initialized. Call set_config first.");
            return;
        }

        let drive = self
            .get_parameters()
            .get("Drive")
            .unwrap()
            .value
            .as_float()
            .unwrap();
        let volume = self
            .get_parameters()
            .get("Level")
            .unwrap()
            .value
            .as_float()
            .unwrap();
        let asymmetry = self
            .get_parameters()
            .get("Asymmetry")
            .map(|p| p.value.as_float().unwrap())
            .unwrap_or(0.5); // Default to 0.5 if not found, as this is a new parameter
        let asymmetry_amount = (asymmetry - 0.5) * 2.0;

        let tone = self
            .get_parameters()
            .get("Tone")
            .unwrap()
            .value
            .as_float()
            .unwrap();
        let eq_enabled = self
            .get_parameters()
            .get("Enable EQ")
            .and_then(|p| p.value.as_bool())
            .unwrap_or(true);

        if eq_enabled {
            let sample_rate = self.sample_rate.unwrap();
            self.update_tone_stack(tone, sample_rate);
        }

        let tone_stack = self.tone_stack.as_mut().unwrap();
        let post_filter = self.post_filter.as_mut().unwrap();
        let dc_blocker = self.dc_blocker.as_mut().unwrap();

        let clip_gain = 1.0 + drive * 0.5;
        let bias = ASYMMETRY_BIAS * asymmetry_amount;
        let makeup = if eq_enabled { self.tone_makeup } else { 1.0 };

        for sample in buffer.iter_mut() {
            let mut x = *sample;

            x *= clip_gain;
            x = Self::asymmetric_diode(x, bias, CLIP_THRESHOLD, CLIP_KNEE);
            x /= clip_gain.sqrt();
            // The coupling caps of the pedal sit right behind the clipper.
            x = dc_blocker.process(x);

            // The clipper feeds the tone stack, and the tone stack feeds the output
            // buffer, exactly as in the pedal. The filters keep running while the EQ is
            // off so that flipping the switch back on doesn't jump their state, only
            // their output is bypassed.
            let filtered = post_filter.process(tone_stack.process(x));
            if eq_enabled {
                x = filtered * makeup;
            }

            x *= volume;
            *sample = x;
        }
    }

    fn set_parameter_value(&mut self, name: &str, value: PedalParameterValue) {
        let parameters = self.get_parameters_mut();
        if let Some(parameter) = parameters.get_mut(name) {
            if parameter.is_valid(&value) {
                parameter.value = value;
            } else {
                tracing::warn!(
                    "Attempted to set invalid value for parameter {}: {:?}",
                    name,
                    value
                );
            }
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _message_buffer: &[String],
    ) -> Option<(String, PedalParameterValue)> {
        ui.add(Image::new(include_image!("images/distortion.png")));

        let mut to_change = None;
        let drive_param = self.get_parameters().get("Drive").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Drive",
            drive_param,
            egui::Vec2::new(0.127, 0.085),
            0.35,
            self.id,
        ) {
            to_change = Some(("Drive".to_string(), value));
        }

        let tone_param = self.get_parameters().get("Tone").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Tone",
            tone_param,
            egui::Vec2::new(0.535, 0.085),
            0.35,
            self.id,
        ) {
            to_change = Some(("Tone".to_string(), value));
        }

        let level_param = self.get_parameters().get("Level").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Level",
            level_param,
            egui::Vec2::new(0.325, 0.335),
            0.35,
            self.id,
        ) {
            to_change = Some(("Level".to_string(), value));
        }

        let active_param = self
            .get_parameters()
            .get("Active")
            .unwrap()
            .value
            .as_bool()
            .unwrap();
        if let Some(value) = pedal_switch(ui, active_param, Vec2::new(0.33, 0.72), 0.16) {
            to_change = Some(("Active".to_string(), PedalParameterValue::Bool(value)));
        }

        to_change
    }
}
