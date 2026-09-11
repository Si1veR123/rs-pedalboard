use std::collections::HashMap;
use std::hash::Hash;

use super::ui::pedal_knob;
use crate::dsp_algorithms::moving_bandpass::MovingBandPass;
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
            let drive = (0.5 + 20.0 * level_ratio.log10() / SWEEP_WINDOW_DB).clamp(0.0, 1.0);

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

    fn rms(signal: &[f32]) -> f32 {
        (signal.iter().map(|x| x * x).sum::<f32>() / signal.len() as f32).sqrt()
    }

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
    fn the_filter_opens_and_closes_with_the_playing_level() {
        // A swell that arrives quickly enough for the reference to lag behind it: the
        // filter has to open on the way up and close again on the way back down.
        let swell: Vec<f32> = sine(SAMPLE_RATE as usize, 300.0, 0.5)
            .iter()
            .enumerate()
            .map(|(i, sample)| {
                let t = i as f32 / SAMPLE_RATE as f32;
                let level = if t < 0.6 {
                    (t / 0.2).min(1.0)
                } else {
                    (1.0 - (t - 0.6) / 0.2).max(0.0)
                };
                sample * level
            })
            .collect();

        let sweep = sweep(&mut pedal(), &swell);

        // The swell arrives faster than the reference can follow, so the filter opens on
        // the way up and closes again on the way down.
        assert!(
            sweep[0] > 900.0,
            "the swell only opened the filter to {} Hz: {sweep:?}",
            sweep[0]
        );
        assert!(
            sweep[11] < 300.0,
            "the filter stayed at {} Hz after the level dropped: {sweep:?}",
            sweep[11]
        );
    }

    #[test]
    fn the_sensitivity_knob_sets_how_far_the_filter_travels() {
        let pluck = pluck(SAMPLE_RATE as usize, 150.0, 0.3);

        let mut narrow = pedal();
        narrow.set_parameter_value("Sensitivity", PedalParameterValue::Float(500.0));
        let narrow_top = sweep(&mut narrow, &pluck)[0];

        let mut wide = pedal();
        wide.set_parameter_value("Sensitivity", PedalParameterValue::Float(2500.0));
        let wide_top = sweep(&mut wide, &pluck)[0];

        // The sweep starts at the 100 Hz base frequency, so the travel above it should
        // scale with the knob.
        let (narrow_travel, wide_travel) = (narrow_top - 100.0, wide_top - 100.0);
        assert!(
            wide_travel > narrow_travel * 2.0,
            "500 Hz of sensitivity travelled {narrow_travel} Hz and 2500 Hz travelled {wide_travel} Hz"
        );
    }

    #[test]
    fn the_wah_closes_again_once_the_note_stops() {
        let mut pedal = pedal();
        play(&mut pedal, &sine(SAMPLE_RATE as usize, 300.0, 0.8));
        assert!(band_hz(&pedal) > 400.0);

        play(&mut pedal, &vec![0.0; SAMPLE_RATE as usize]);
        let band = band_hz(&pedal);
        assert!(band < 150.0, "the filter stayed open at {band} Hz");
    }

    #[test]
    fn width_still_trades_resonance_for_bandwidth() {
        let input = sine(24_000, 1000.0, 0.5);

        // Sensitivity is wound right down so both pedals hold the band pass on the base
        // frequency and the only difference between them is the width.
        let mut narrow = pedal();
        narrow.set_parameter_value("Base Freq", PedalParameterValue::Float(1000.0));
        narrow.set_parameter_value("Sensitivity", PedalParameterValue::Float(100.0));
        narrow.set_parameter_value("Width", PedalParameterValue::Float(0.2));
        let narrow_output = play(&mut narrow, &input);

        let mut wide = pedal();
        wide.set_parameter_value("Base Freq", PedalParameterValue::Float(1000.0));
        wide.set_parameter_value("Sensitivity", PedalParameterValue::Float(100.0));
        wide.set_parameter_value("Width", PedalParameterValue::Float(2.0));
        let wide_output = play(&mut wide, &input);

        let (narrow_hz, wide_hz) = (band_hz(&narrow), band_hz(&wide));
        assert!(
            (narrow_hz - wide_hz).abs() < 100.0,
            "the two band passes landed at {narrow_hz} Hz and {wide_hz} Hz"
        );

        let (narrow_level, wide_level) = (
            rms(&narrow_output[narrow_output.len() - 480..]),
            rms(&wide_output[wide_output.len() - 480..]),
        );
        assert!(
            narrow_level > wide_level * 3.0,
            "a narrow band pass gave {narrow_level} and a wide one {wide_level}"
        );
    }

    /// Amplitude of `signal` at `freq`, by direct DFT correlation.
    fn level_at(signal: &[f32], sample_rate: f32, freq: f32) -> f64 {
        let omega = std::f64::consts::TAU * (freq as f64) / (sample_rate as f64);
        let (mut re, mut im) = (0.0f64, 0.0f64);
        for (n, sample) in signal.iter().enumerate() {
            let theta = omega * n as f64;
            re += *sample as f64 * theta.cos();
            im += *sample as f64 * theta.sin();
        }
        (re * re + im * im).sqrt() * 2.0 / signal.len() as f64
    }

    /// Energy in 80..250, 250..900 and 900..5000 Hz, sampled at 15% spacing.
    fn bands(signal: &[f32], sample_rate: f32) -> [f64; 3] {
        let ranges = [(80.0f32, 250.0f32), (250.0, 900.0), (900.0, 5000.0)];
        let mut out = [0.0f64; 3];
        for (index, (lo, hi)) in ranges.iter().enumerate() {
            let mut freq = *lo;
            while freq <= *hi {
                out[index] += level_at(signal, sample_rate, freq);
                freq *= 1.15;
            }
        }
        out
    }

    /// Manual probe: renders a recording through the pedal and prints what the sweep is
    /// doing. `AUTOWAH_PROBE_WAV=<path> cargo test --lib probe_real -- --ignored --nocapture`
    #[test]
    #[ignore = "manual probe against a local recording"]
    fn probe_real_recording() {
        let path = match std::env::var("AUTOWAH_PROBE_WAV") {
            Ok(path) => path,
            Err(_) => {
                println!("set AUTOWAH_PROBE_WAV to a recording to probe it");
                return;
            }
        };
        let mut reader = hound::WavReader::open(&path).unwrap();
        let spec = reader.spec();
        let interleaved: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => reader.samples::<f32>().map(Result::unwrap).collect(),
            hound::SampleFormat::Int => {
                let scale = (1i64 << (spec.bits_per_sample - 1)) as f32;
                reader
                    .samples::<i32>()
                    .map(|s| s.unwrap() as f32 / scale)
                    .collect()
            }
        };
        let channels = spec.channels as usize;
        let mono: Vec<f32> = interleaved
            .chunks_exact(channels)
            .map(|frame| frame.iter().sum::<f32>() / channels as f32)
            .collect();
        let sample_rate = spec.sample_rate as f32;
        println!(
            "{} Hz, {} ch, {} bits, {} frames ({:.2} s), input peak {:.4}",
            sample_rate,
            channels,
            spec.bits_per_sample,
            mono.len(),
            mono.len() as f32 / sample_rate,
            mono.iter().fold(0.0f32, |a, b| a.max(b.abs()))
        );

        let mut pedal = AutoWah::new();
        let mut messages = Vec::new();
        let mut output = vec![0.0f32; mono.len()];
        let mut trace: Vec<(usize, f32, f32)> = Vec::new();

        let mut start = 0;
        while start < mono.len() {
            let end = (start + BLOCK).min(mono.len());
            // The processor API reconfigures before every block.
            pedal.set_config(BLOCK, sample_rate as u32);
            let mut frame = mono[start..end].to_vec();
            pedal.process_audio(&mut frame, &mut messages);
            output[start..end].copy_from_slice(&frame);
            let filter = &pedal.filter.as_ref().unwrap().0;
            let level_ratio =
                pedal.envelope.max(AUTO_GAIN_FLOOR) / pedal.reference.max(AUTO_GAIN_FLOOR);
            trace.push((
                start,
                (0.5 + 20.0 * level_ratio.log10() / SWEEP_WINDOW_DB).clamp(0.0, 1.0),
                filter.current_freq(),
            ));
            start = end;
        }

        let hop = 4096;
        println!(
            "{:>6} {:>7} {:>7} {:>7} | {:>7} {:>7} {:>7} | {:>7} {:>6}",
            "t", "inlo", "inmid", "inhi", "outlo", "outmid", "outhi", "drive", "freq"
        );
        let mut window = 0;
        while (window + 1) * hop <= mono.len() {
            let from = window * hop;
            let to = (window + 1) * hop;
            let input_bands = bands(&mono[from..to], sample_rate);
            let output_bands = bands(&output[from..to], sample_rate);
            let (_, drive, freq) = trace[from / BLOCK];
            println!(
                "{:>6.2} {:>7.4} {:>7.4} {:>7.4} | {:>7.4} {:>7.4} {:>7.4} | {:>7.3} {:>6.0}",
                from as f32 / sample_rate as f32,
                input_bands[0],
                input_bands[1],
                input_bands[2],
                output_bands[0],
                output_bands[1],
                output_bands[2],
                drive,
                freq
            );
            window += 1;
        }

        let frequencies: Vec<f32> = trace.iter().map(|(_, _, hz)| *hz).collect();
        let drives: Vec<f32> = trace.iter().map(|(_, drive, _)| *drive).collect();
        let min = frequencies.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = frequencies.iter().cloned().fold(0.0f32, f32::max);
        println!(
            "filter swept {min:.0}..{max:.0} Hz, drive {}",
            drives
                .iter()
                .map(|d| format!("{d:.2}"))
                .collect::<Vec<_>>()
                .join(" ")
        );

        let out_spec = hound::WavSpec {
            channels: 1,
            sample_rate: sample_rate as u32,
            bits_per_sample: 32,
            sample_format: hound::SampleFormat::Float,
        };
        let out_path = std::path::Path::new(&path).with_file_name("autowah_probe_out.wav");
        let mut writer = hound::WavWriter::create(&out_path, out_spec).unwrap();
        for sample in &output {
            writer.write_sample(*sample).unwrap();
        }
        writer.finalize().unwrap();
        println!("wrote {}", out_path.display());
    }

    #[test]
    fn silence_stays_silent() {
        let mut pedal = pedal();
        let output = play(&mut pedal, &vec![0.0; 1024]);
        assert!(output.iter().all(|sample| *sample == 0.0));
    }

    #[test]
    fn reconfiguring_between_buffers_keeps_the_sweep_where_it_is() {
        let mut pedal = pedal();
        play(&mut pedal, &pluck(SAMPLE_RATE as usize, 150.0, 0.3));
        let settled = band_hz(&pedal);

        // The processor API calls set_config before every buffer. It must not drop the
        // filter back down to the base frequency each time.
        pedal.set_config(BLOCK, SAMPLE_RATE);
        assert!(
            (band_hz(&pedal) - settled).abs() < 1.0,
            "the sweep fell from {settled} Hz to {} Hz",
            band_hz(&pedal)
        );
    }

    #[test]
    fn a_sample_rate_change_rebuilds_the_filter() {
        let mut pedal = pedal();
        play(&mut pedal, &pluck(SAMPLE_RATE as usize, 150.0, 0.3));

        // Every coefficient depends on the sample rate, so a new rate has to build a new
        // filter. Reconfiguring with the same rate must leave the sweep alone, which is
        // what stops the processor API resetting it on every buffer.
        pedal.set_config(BLOCK, 96_000);
        assert_eq!(
            pedal.filter.as_ref().unwrap().1,
            96_000,
            "set_config kept the old sample rate"
        );

        assert!(
            band_hz(&pedal) < 150.0,
            "a fresh filter for the new sample rate started at {} Hz instead of the base frequency",
            band_hz(&pedal)
        );
    }
}
