use std::collections::HashMap;
use std::hash::Hash;

use super::ui::pedal_knob;
use crate::dsp::{moving_bandpass::MovingBandPass, ConvertSampleDbLinear};
use crate::pedals::ui::pedal_switch;
use crate::pedals::{PedalParameter, PedalParameterValue, PedalTrait};

use eframe::egui::{self, include_image};
use serde::{ser::SerializeMap, Deserialize, Serialize};

/// Attack time of the envelope follower in seconds.
const ATTACK_SECONDS: f32 = 0.005;

/// Attack and release of the level reference the envelope is compared against. Both are
/// slower than a pick attack, so a picked note stands out above the reference and opens
/// the filter, and faster than a note, so the reference catches up and the filter closes
/// again while the string is still ringing.
const REFERENCE_ATTACK_SECONDS: f32 = 0.06;
const REFERENCE_RELEASE_SECONDS: f32 = 1.5;

/// Window of the note level relative to the reference that spans the whole sweep, in dB.
/// The middle of the sweep is the reference level itself, so a held note rests in the
/// middle of the range instead of parking the filter at one end. Deliberately narrow, so
/// the decay of a plucked note walks the filter down most of the range while the string
/// is still ringing.
const SWEEP_WINDOW_DB: f32 = 15.0;

/// Floor for the level ratio, so silence doesn't get divided by ~nothing.
const AUTO_GAIN_FLOOR: f32 = 1e-4;

#[derive(Clone)]
pub struct AutoWah {
    parameters: HashMap<String, PedalParameter>,
    filter: Option<(MovingBandPass, u32)>,
    envelope: f32,
    /// Slow follower the envelope is compared against, see [`REFERENCE_ATTACK_SECONDS`].
    reference: f32,
    id: u32,
}

impl Serialize for AutoWah {
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

impl<'a> Deserialize<'a> for AutoWah {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        #[derive(Deserialize)]
        struct AutoWahData {
            id: u32,
            parameters: HashMap<String, PedalParameter>,
        }

        let helper = AutoWahData::deserialize(deserializer)?;
        Ok(AutoWah {
            parameters: helper.parameters,
            filter: None,
            envelope: 0.0,
            reference: 0.0,
            id: helper.id,
        })
    }
}

impl Hash for AutoWah {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl AutoWah {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();
        parameters.insert(
            "Width".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.5),
                min: Some(PedalParameterValue::Float(0.1)),
                max: Some(PedalParameterValue::Float(2.0)),
                step: None,
            },
        );
        parameters.insert(
            "Sensitivity".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1000.0), // sweep span in Hz
                min: Some(PedalParameterValue::Float(100.0)),
                max: Some(PedalParameterValue::Float(3000.0)),
                step: None,
            },
        );
        parameters.insert(
            "Base Freq".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(100.0), // in Hz
                min: Some(PedalParameterValue::Float(50.0)),
                max: Some(PedalParameterValue::Float(1000.0)),
                step: None,
            },
        );
        parameters.insert(
            "Envelope Smoothing".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.9999), // smoothing factor
                min: Some(PedalParameterValue::Float(0.999)),
                max: Some(PedalParameterValue::Float(0.999999)),
                step: None,
            },
        );
        parameters.insert(
            "Dry Wet".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
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

        AutoWah {
            parameters,
            filter: None,
            envelope: 0.0,
            reference: 0.0,
            id: crate::unique_time_id(),
        }
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = crate::unique_time_id();
        cloned
    }
}

impl PedalTrait for AutoWah {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        let (filter, sample_rate) = match &mut self.filter {
            Some((f, sr)) => (f, sr),
            None => return,
        };

        let sensitivity = self.parameters["Sensitivity"].value.as_float().unwrap();
        let base_freq = self.parameters["Base Freq"].value.as_float().unwrap();
        let envelope_smoothing = self.parameters["Envelope Smoothing"]
            .value
            .as_float()
            .unwrap();
        let dry_wet = self.parameters["Dry Wet"].value.as_float().unwrap();

        // Fast attack avoids the filter chattering, the smoothing knob controls release.
        // The level reference rises and falls slower than that, see the constants.
        let attack = (-1.0 / (ATTACK_SECONDS * *sample_rate as f32)).exp();
        let reference_attack = (-1.0 / (REFERENCE_ATTACK_SECONDS * *sample_rate as f32)).exp();
        let reference_release = (-1.0 / (REFERENCE_RELEASE_SECONDS * *sample_rate as f32)).exp();

        for sample in buffer.iter_mut() {
            let rectified = sample.abs();

            let coeff = if rectified > self.envelope {
                attack
            } else {
                envelope_smoothing
            };
            self.envelope = coeff * self.envelope + (1.0 - coeff) * rectified;

            let reference_coeff = if rectified > self.reference {
                reference_attack
            } else {
                reference_release
            };
            self.reference = reference_coeff * self.reference + (1.0 - reference_coeff) * rectified;

            // Where the note sits relative to how loud the playing has been, spread over
            // the sweep window. Both followers see the same signal, so their steady states
            // are the same level: a note holding still reads as 0 dB and rests in the
            // middle of the sweep, a pick attack reads as positive and opens the filter,
            // and the decay of the note reads as negative and closes it again. Comparing
            // levels rather than scaling by raw amplitude is what makes the wah audible at
            // any input level, and resting in the middle is what stops the filter parking
            // at one end of the range, which just sounds like a fixed band pass.
            let level_ratio =
                self.envelope.max(AUTO_GAIN_FLOOR) / self.reference.max(AUTO_GAIN_FLOOR);

            let db_level = f32::linear_to_db(level_ratio);
            let drive = (0.5 + db_level / SWEEP_WINDOW_DB).clamp(0.0, 1.0);

            filter.set_freq(base_freq + drive * sensitivity);

            *sample = filter.process(*sample) * dry_wet + *sample * (1.0 - dry_wet);
        }
    }

    fn reset_buffer(&mut self) {
        self.envelope = 0.0;
        self.reference = 0.0;
        if let Some((filter, _)) = &mut self.filter {
            filter.reset();
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn set_parameter_value(&mut self, name: &str, value: PedalParameterValue) {
        let parameters = self.get_parameters_mut();
        if let Some(parameter) = parameters.get_mut(name) {
            if parameter.is_valid(&value) {
                parameter.value = value;

                if name == "Width" {
                    let width = parameter.value.as_float().unwrap();
                    if let Some((bandpass, _)) = &mut self.filter {
                        bandpass.set_width(width);
                    }
                }
            } else {
                tracing::warn!(
                    "Attempted to set invalid value for parameter {}: {:?}",
                    name,
                    value
                );
            }
        }
    }

    fn parameter_value_display_string(
        &self,
        name: &str,
        value: &PedalParameterValue,
    ) -> Option<String> {
        match name {
            "Base Freq" => Some(format!("{:.1} Hz", value.as_float().unwrap())),
            "Sensitivity" => Some(format!("{:.0} Hz", value.as_float().unwrap())),
            "Width" => Some(format!("{:.2}", value.as_float().unwrap())),
            "Envelope Smoothing" => Some(format!("{:.5}", value.as_float().unwrap())),
            "Dry Wet" => Some(format!("{:.0}%", value.as_float().unwrap() * 100.0)),
            _ => None,
        }
    }

    fn set_config(&mut self, _buffer_size: usize, sample_rate: u32) {
        // The processor API calls this before every buffer, so only build the filter when
        // the configuration really changed: rebuilding it would snap the sweep back to
        // the base frequency and wipe the filter state on every buffer.
        if self.filter.as_ref().map(|(_, rate)| *rate) == Some(sample_rate) {
            return;
        }

        let width = self.parameters["Width"].value.as_float().unwrap();
        let base_freq = self.parameters["Base Freq"].value.as_float().unwrap();
        let filter = MovingBandPass::new(base_freq, sample_rate as f32, width, 64, 5.0);
        self.filter = Some((filter, sample_rate));
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _message_buffer: &[String],
    ) -> Option<(String, PedalParameterValue)> {
        ui.add(egui::Image::new(include_image!("images/autowah.png")));

        let mut to_change = None;

        let base_freq_param = self.get_parameters().get("Base Freq").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Base Freq",
            base_freq_param,
            egui::Vec2::new(0.68, 0.045),
            0.25,
            self.id,
        ) {
            to_change = Some(("Base Freq".to_string(), value));
        }

        let sensitivity_param = self.get_parameters().get("Sensitivity").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Sensitivity",
            sensitivity_param,
            egui::Vec2::new(0.68, 0.17),
            0.25,
            self.id,
        ) {
            to_change = Some(("Sensitivity".to_string(), value));
        }

        let width_param = self.get_parameters().get("Width").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Width",
            width_param,
            egui::Vec2::new(0.68, 0.295),
            0.25,
            self.id,
        ) {
            to_change = Some(("Width".to_string(), value));
        }

        let envelope_smoothing_param = self.get_parameters().get("Envelope Smoothing").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Envelope Smoothing",
            envelope_smoothing_param,
            egui::Vec2::new(0.68, 0.425),
            0.25,
            self.id,
        ) {
            to_change = Some(("Envelope Smoothing".to_string(), value));
        }

        let active_param = self
            .get_parameters()
            .get("Active")
            .unwrap()
            .value
            .as_bool()
            .unwrap();
        if let Some(value) = pedal_switch(ui, active_param, egui::Vec2::new(0.33, 0.72), 0.16) {
            to_change = Some(("Active".to_string(), PedalParameterValue::Bool(value)));
        }

        to_change
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RATE: u32 = 48_000;
    const BLOCK: usize = 4096;

    fn sine(samples: usize, frequency: f32, amplitude: f32) -> Vec<f32> {
        (0..samples)
            .map(|i| {
                amplitude
                    * (std::f32::consts::TAU * frequency * i as f32 / SAMPLE_RATE as f32).sin()
            })
            .collect()
    }

    fn pedal() -> AutoWah {
        let mut pedal = AutoWah::new();
        pedal.set_config(BLOCK, SAMPLE_RATE);
        pedal
    }

    /// A plucked note: instant attack and a body that rings out over about a second. The
    /// harmonics give the sweep something to move across, the way a guitar does.
    fn pluck(samples: usize, frequency: f32, amplitude: f32) -> Vec<f32> {
        (0..samples)
            .map(|i| {
                let t = i as f32 / SAMPLE_RATE as f32;
                let decay = (-t / 0.3).exp();
                let body: f32 = (1..=6)
                    .map(|harmonic| {
                        (std::f32::consts::TAU * frequency * harmonic as f32 * t).sin()
                            / harmonic as f32
                    })
                    .sum();
                amplitude * decay * body
            })
            .collect()
    }

    fn play(pedal: &mut AutoWah, signal: &[f32]) -> Vec<f32> {
        let mut buffer = signal.to_vec();
        let mut messages = Vec::new();
        pedal.process_audio(&mut buffer, &mut messages);
        buffer
    }

    /// Runs `signal` through the pedal a buffer at a time, reconfiguring before each one
    /// the way the processor API does, and reports where the band pass sat in each buffer.
    fn sweep(pedal: &mut AutoWah, signal: &[f32]) -> Vec<f32> {
        let mut frequencies = Vec::new();
        for block in signal.chunks(BLOCK) {
            pedal.set_config(BLOCK, SAMPLE_RATE);
            let mut buffer = block.to_vec();
            let mut messages = Vec::new();
            pedal.process_audio(&mut buffer, &mut messages);
            frequencies.push(band_hz(pedal));
        }
        frequencies
    }

    /// Frequency the band pass is sitting at.
    fn band_hz(pedal: &AutoWah) -> f32 {
        pedal.filter.as_ref().unwrap().0.current_freq()
    }

    #[test]
    fn a_picked_note_opens_the_filter_and_walks_it_back_down() {
        let sweep = sweep(&mut pedal(), &pluck(SAMPLE_RATE as usize, 150.0, 0.3));

        assert!(
            sweep[0] > 700.0,
            "the pick only opened the filter to {} Hz",
            sweep[0]
        );

        // The filter has to come back down while the string is still ringing: a sweep
        // that only happens once the note has died away is inaudible.
        let lowest_while_ringing = sweep[4..8].iter().cloned().fold(f32::INFINITY, f32::min);
        assert!(
            lowest_while_ringing < 400.0,
            "the filter was still at {lowest_while_ringing} Hz a few hundred ms in: {:?}",
            &sweep[..8]
        );

        // And it ends up back at the base frequency once the note is gone.
        assert!(
            *sweep.last().unwrap() < 150.0,
            "the filter parked at {} Hz after the note stopped",
            sweep.last().unwrap()
        );
    }

    #[test]
    fn a_quiet_pluck_sweeps_the_same_as_a_loud_one() {
        let quiet = sweep(&mut pedal(), &pluck(SAMPLE_RATE as usize, 150.0, 0.02));
        let loud = sweep(&mut pedal(), &pluck(SAMPLE_RATE as usize, 150.0, 0.6));

        for (index, (quiet_hz, loud_hz)) in quiet.iter().zip(&loud).enumerate() {
            assert!(
                (quiet_hz - loud_hz).abs() < loud_hz * 0.1,
                "buffer {index} landed at {quiet_hz} Hz quietly and {loud_hz} Hz loudly"
            );
        }
    }

    #[test]
    fn a_held_note_rests_in_the_middle_of_the_sweep() {
        // A note that just sits there must not park the filter at either end of the
        // range: sitting at the top is what made this pedal sound like a fixed band pass.
        let mut pedal = pedal();
        play(&mut pedal, &sine(SAMPLE_RATE as usize, 300.0, 0.4));

        let settled = band_hz(&pedal);
        assert!(
            (350.0..850.0).contains(&settled),
            "a held note parked the filter at {settled} Hz"
        );
    }

    #[test]
    fn silence_stays_silent() {
        let mut pedal = pedal();
        let output = play(&mut pedal, &vec![0.0; 1024]);
        assert!(output.iter().all(|sample| *sample == 0.0));
    }
}
