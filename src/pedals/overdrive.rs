// Roughly modelled after a tube screamer

use std::collections::HashMap;
use std::hash::Hash;

use crate::dsp_algorithms::biquad::BiquadFilter;
use crate::dsp_algorithms::eq;
use crate::unique_time_id;

use super::ui::{pedal_knob, pedal_switch};
use super::PedalParameter;
use super::PedalParameterValue;
use super::PedalTrait;

use eframe::egui::Image;
use eframe::egui::{self, include_image, Vec2};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize};

/// Tone control of the TS-808/TS-9: a passive first order low pass (1k + 0.22uF)
/// that is always in the signal path. With the tone pot all the way to the treble
/// side the corner sits at 720 Hz, rolling the pot back to the bass side drags it
/// down to 360 Hz (AMZ "Expanding the TS-9 Tone Control", ElectroSmash analysis).
const TONE_CORNER_TREBLE_HZ: f32 = 720.0;
const TONE_CORNER_BASS_HZ: f32 = 360.0;
/// Frequency response of the clipping stage. The gain of the real stage is
/// `1 + Rf / (Rs + 1/(sC))`: unity in the bass and `1 + Rf/Rs` above 720 Hz, and the
/// drive pot only moves the high end of that shelf, from about +21 dB to +41 dB. The
/// bass stays clean while the mids are pushed into the diodes, which is the behaviour
/// that makes a tube screamer sound like one - and, because the stage keeps unity gain
/// in the bass instead of cutting it, it is also why the pedal doesn't lose level when
/// the clipping stage is engaged. The shelf is set to the middle of that range here and
/// the drive knob supplies the rest of the gain the pot would add.
const CLIP_STAGE_CORNER_HZ: f32 = 720.0;
const CLIP_STAGE_GAIN_DB: f32 = 12.0;
/// Bump around the corner of the clipping stage, the mid hump of the pedal.
const CLIP_STAGE_MID_DB: f32 = 3.0;
/// The tone control is a passive network, so it loses level. The buffer behind it makes
/// that loss up in the hardware, and the same is done here so that flipping "Enable EQ"
/// doesn't jump the level of the pedal.
const EQ_MAKEUP: f32 = 1.35;
/// Output buffer bandwidth, keeps the clipped harmonics from getting fizzy.
const POST_FILTER_HZ: f32 = 7500.0;

#[derive(Clone)]
pub struct Overdrive {
    parameters: HashMap<String, PedalParameter>,
    // Processor only
    clip_stage: Option<eq::Equalizer>,
    tone_filter: Option<BiquadFilter>,
    tone_corner: f32,
    post_filter: Option<BiquadFilter>,
    sample_rate: Option<f32>,
    id: u32,
}

impl Serialize for Overdrive {
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

impl<'a> Deserialize<'a> for Overdrive {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        #[derive(Deserialize)]
        struct OverdriveData {
            id: u32,
            parameters: HashMap<String, PedalParameter>,
        }
        let helper = OverdriveData::deserialize(deserializer)?;
        Ok(Overdrive {
            parameters: helper.parameters,
            clip_stage: None,
            tone_filter: None,
            tone_corner: 0.0,
            post_filter: None,
            sample_rate: None,
            id: helper.id,
        })
    }
}

impl Hash for Overdrive {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Overdrive {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();
        parameters.insert(
            "Drive".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(5.0),
                min: Some(PedalParameterValue::Float(1.0)),
                max: Some(PedalParameterValue::Float(30.0)),
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
            "Enable EQ".to_string(),
            PedalParameter {
                value: PedalParameterValue::Bool(true),
                min: None,
                max: None,
                step: None,
            },
        );
        Overdrive {
            parameters,
            clip_stage: None,
            tone_filter: None,
            tone_corner: 0.0,
            post_filter: None,
            sample_rate: None,
            id: unique_time_id(),
        }
    }

    pub fn diode_soft_clip(x: f32, knee: f32) -> f32 {
        x / (knee * (1.0 + (x / knee).powi(2)).sqrt())
    }

    /// Frequency response of the clipping stage (see [`CLIP_STAGE_GAIN_DB`]): unity in
    /// the bass, lifted above 720 Hz, with the mid hump of the pedal sitting on top of
    /// the corner. This belongs to the clipping stage, not to the tone section, so it
    /// stays in the signal path no matter where "Enable EQ" sits.
    pub fn clip_stage(sample_rate: f32) -> eq::Equalizer {
        let shelf =
            BiquadFilter::high_shelf(CLIP_STAGE_CORNER_HZ, sample_rate, 0.6, CLIP_STAGE_GAIN_DB);
        let mid_boost =
            BiquadFilter::peaking(CLIP_STAGE_CORNER_HZ, sample_rate, 0.7, CLIP_STAGE_MID_DB);
        eq::Equalizer::new(vec![shelf, mid_boost])
    }

    /// Corner frequency of the tone control for a given knob position. The sweep is
    /// logarithmic between the two extremes, matching how the hardware pot moves the
    /// corner around.
    fn tone_corner(tone: f32) -> f32 {
        let tone = tone.clamp(0.0, 1.0);
        TONE_CORNER_BASS_HZ * (TONE_CORNER_TREBLE_HZ / TONE_CORNER_BASS_HZ).powf(tone)
    }

    pub fn tone_filter(tone: f32, sample_rate: f32) -> BiquadFilter {
        BiquadFilter::first_order_low_pass(Self::tone_corner(tone), sample_rate)
    }

    /// Rebuilds the tone filter when the tone knob has moved, carrying the filter
    /// state over so the knob doesn't click.
    fn update_tone_filter(&mut self, tone: f32, sample_rate: f32) {
        let corner = Self::tone_corner(tone);
        if (corner - self.tone_corner).abs() <= f32::EPSILON {
            return;
        }

        let current = self.tone_filter.as_ref().unwrap();
        let (s1, s2) = (current.s1, current.s2);

        let mut filter = Self::tone_filter(tone, sample_rate);
        filter.s1 = s1;
        filter.s2 = s2;

        self.tone_filter = Some(filter);
        self.tone_corner = corner;
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: f64 = 48_000.0;

    fn gain(filter: &BiquadFilter, frequency: f64) -> f64 {
        filter.response_at_freq(frequency, SAMPLE_RATE).norm()
    }

    #[test]
    fn tone_control_sweeps_between_the_corners_of_the_pedal() {
        let treble = Overdrive::tone_filter(1.0, SAMPLE_RATE as f32);
        let bass = Overdrive::tone_filter(0.0, SAMPLE_RATE as f32);

        // The tone pot swings the corner between 720 Hz and 360 Hz.
        assert!((gain(&treble, TONE_CORNER_TREBLE_HZ as f64) - 0.7071).abs() < 0.01);
        assert!((gain(&bass, TONE_CORNER_BASS_HZ as f64) - 0.7071).abs() < 0.01);

        // Rolling the tone knob back filters the highs instead of boosting anything.
        assert!(gain(&bass, 4000.0) < gain(&treble, 4000.0));
        for tone in [0.0, 0.25, 0.5, 0.75, 1.0] {
            let filter = Overdrive::tone_filter(tone, SAMPLE_RATE as f32);
            for frequency in [100.0, 500.0, 1000.0, 4000.0, 10_000.0] {
                assert!(gain(&filter, frequency) <= 1.0 + 1e-6);
            }
        }

        // The knob position moves the corner logarithmically between the two extremes.
        let middle = Overdrive::tone_corner(0.5);
        assert!((middle - TONE_CORNER_BASS_HZ * 2.0f32.sqrt()).abs() < 1.0);
    }

    #[test]
    fn disabling_eq_bypasses_the_filters() {
        let input: Vec<f32> = (0..256).map(|i| (i as f32 * 0.1).sin() * 0.5).collect();
        let mut messages = Vec::new();

        let mut render = |tone: f32, eq_enabled: bool| {
            let mut pedal = Overdrive::new();
            pedal.set_config(256, SAMPLE_RATE as u32);
            pedal.set_parameter_value("Tone", PedalParameterValue::Float(tone));
            pedal.set_parameter_value("Enable EQ", PedalParameterValue::Bool(eq_enabled));

            let mut buffer = input.clone();
            pedal.process_audio(&mut buffer, &mut messages);
            buffer
        };

        let without_eq_low_tone = render(0.0, false);
        let without_eq_high_tone = render(1.0, false);

        // With the EQ off the tone knob has no effect at all...
        assert_eq!(without_eq_low_tone, without_eq_high_tone);
        // ...while with the EQ on it still shapes the sound.
        assert_ne!(render(0.0, true), render(1.0, true));
    }

    #[test]
    fn clip_stage_keeps_the_bass_and_pushes_the_mids_into_the_diodes() {
        let stage = Overdrive::clip_stage(SAMPLE_RATE as f32);
        let db = |frequency: f64| {
            20.0 * stage
                .response_at_freq(frequency, SAMPLE_RATE)
                .norm()
                .log10()
        };

        // The clipping stage of a tube screamer has unity gain in the bass: the bass
        // stays clean while everything above the corner is driven into the diodes.
        assert!(db(60.0).abs() < 1.0, "bass {}", db(60.0));
        assert!(db(200.0) < 1.0, "low mids {}", db(200.0));
        assert!(db(CLIP_STAGE_CORNER_HZ as f64) > db(60.0) + 5.0);
        assert!(db(4000.0) > db(60.0) + 9.0);
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

    #[test]
    fn eq_switch_does_not_jump_the_level() {
        let input = guitar_input(4096, 220.0, SAMPLE_RATE);
        let mut messages = Vec::new();

        let mut render = |tone: f32, eq_enabled: bool, drive: f32| {
            let mut pedal = Overdrive::new();
            pedal.set_config(4096, SAMPLE_RATE as u32);
            pedal.set_parameter_value("Tone", PedalParameterValue::Float(tone));
            pedal.set_parameter_value("Drive", PedalParameterValue::Float(drive));
            pedal.set_parameter_value("Enable EQ", PedalParameterValue::Bool(eq_enabled));

            let mut buffer = input.clone();
            pedal.process_audio(&mut buffer, &mut messages);
            buffer
        };

        // The tube screamer is no louder with its tone control bypassed than with it in
        // the path, so the with / without EQ levels have to stay in the same ballpark
        // for the whole range of the tone and drive knobs.
        for drive in [1.0, 5.0, 15.0, 30.0] {
            for tone in [0.0, 0.5, 1.0] {
                let with_eq = rms(&render(tone, true, drive));
                let without_eq = rms(&render(tone, false, drive));
                let difference_db = 20.0 * (with_eq / without_eq).log10();

                assert!(
                    difference_db.abs() < 2.0,
                    "drive {drive}, tone {tone}: with EQ is {difference_db:.2} dB hotter than without"
                );
            }
        }

        // ...and at the default setting the two are within a fraction of a dB.
        let with_eq = rms(&render(0.5, true, 5.0));
        let without_eq = rms(&render(0.5, false, 5.0));
        assert!((with_eq / without_eq - 1.0).abs() < 0.15);
    }
}

impl PedalTrait for Overdrive {
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

        self.clip_stage = Some(Self::clip_stage(sample_rate));
        self.tone_filter = Some(Self::tone_filter(tone, sample_rate));
        self.tone_corner = Self::tone_corner(tone);
        self.post_filter = Some(BiquadFilter::low_pass(POST_FILTER_HZ, sample_rate, 0.707));
        self.sample_rate = Some(sample_rate);
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        if self.clip_stage.is_none() || self.tone_filter.is_none() || self.post_filter.is_none() {
            tracing::warn!("Overdrive: Filters not initialized. Call set_config first.");
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
            self.update_tone_filter(tone, sample_rate);
        }

        let clip_stage = self.clip_stage.as_mut().unwrap();
        let tone_filter = self.tone_filter.as_mut().unwrap();
        let post_filter = self.post_filter.as_mut().unwrap();

        let makeup = if eq_enabled { EQ_MAKEUP } else { 1.0 };

        for sample in buffer.iter_mut() {
            let mut x = *sample;

            // Clipping stage: the shelf of the feedback network sits in front of the
            // diodes and the drive knob adds the rest of the stage gain on top of it.
            x = clip_stage.process(x);

            x *= drive;

            x = Self::diode_soft_clip(x, 0.5);
            x /= drive.sqrt().max(1.0);

            // The tone control and the output buffer sit behind the clipper. The filters
            // keep running while the EQ is off so that flipping the switch back on
            // doesn't jump their state, only their output is bypassed.
            let tone_out = tone_filter.process(x);
            let post_out = post_filter.process(tone_out);
            if eq_enabled {
                x = post_out * makeup;
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
        ui.add(Image::new(include_image!("images/overdrive.png")));

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
