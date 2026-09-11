use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::iter;

use super::ui::pedal_knob;
use super::{PedalParameter, PedalParameterValue, PedalTrait};
use crate::dsp_algorithms::{biquad, eq};
use crate::pedals::ui::pedal_switch;
use crate::unique_time_id;

use eframe::egui::{self, include_image};
use serde::ser::SerializeMap;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct Delay {
    pub parameters: HashMap<String, PedalParameter>,
    // Processor only
    delay_buffer: Option<VecDeque<f32>>,
    tone_eq: Option<eq::Equalizer>,
    sample_rate: Option<f32>,
    id: u32,
}

impl Hash for Delay {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Serialize for Delay {
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

impl<'de> Deserialize<'de> for Delay {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct DelayData {
            id: u32,
            parameters: HashMap<String, PedalParameter>,
        }

        let helper = DelayData::deserialize(deserializer)?;
        Ok(Delay {
            id: helper.id,
            parameters: helper.parameters,
            delay_buffer: None,
            tone_eq: None,
            sample_rate: None,
        })
    }
}

impl Delay {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();

        let init_delay = 430.0;
        let init_warmth = 0.0;

        parameters.insert(
            "Delay".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(init_delay),
                min: Some(PedalParameterValue::Float(10.0)),
                max: Some(PedalParameterValue::Float(1000.0)),
                step: None,
            },
        );
        parameters.insert(
            "Decay".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.5),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None,
            },
        );
        parameters.insert(
            "Dry/Wet".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.5),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None,
            },
        );
        parameters.insert(
            "Warmth".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(init_warmth),
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

        Delay {
            parameters,
            delay_buffer: None,
            tone_eq: None,
            sample_rate: None,
            id: unique_time_id(),
        }
    }

    pub fn eq_from_warmth(tone: f32, sample_rate: f32) -> eq::Equalizer {
        let biquad = biquad::BiquadFilter::high_shelf(4000.0, sample_rate, 0.707, -tone * 10.0);
        let eq = eq::Equalizer::new(vec![biquad]);
        eq
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned
    }
}

impl PedalTrait for Delay {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn set_config(&mut self, _buffer_size: usize, sample_rate: u32) {
        // The processor API calls this before every buffer, so only set up the tone EQ and
        // the delay line when the configuration really changed. Rebuilding them would empty
        // the delay line, which cuts the echoes off at every buffer boundary.
        if self.sample_rate == Some(sample_rate as f32) {
            return;
        }

        self.tone_eq = Some(Self::eq_from_warmth(
            self.parameters
                .get("Warmth")
                .unwrap()
                .value
                .as_float()
                .unwrap(),
            sample_rate as f32,
        ));
        self.sample_rate = Some(sample_rate as f32);
        let delay_ms = self
            .parameters
            .get("Delay")
            .unwrap()
            .value
            .as_float()
            .unwrap();
        let delay_samples = ((delay_ms / 1000.0) * sample_rate as f32) as usize;
        self.delay_buffer = Some(VecDeque::from_iter(iter::repeat(0.0).take(delay_samples)));
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        if self.tone_eq.is_none() || self.delay_buffer.is_none() {
            tracing::warn!("Delay: Call set_config() before processing audio.");
            return;
        }

        let decay = self
            .parameters
            .get("Decay")
            .unwrap()
            .value
            .as_float()
            .unwrap();
        let mix = self
            .parameters
            .get("Dry/Wet")
            .unwrap()
            .value
            .as_float()
            .unwrap();
        for sample in buffer.iter_mut() {
            let delay_sample = self.delay_buffer.as_mut().unwrap().pop_front().unwrap();

            let mut new_sample = *sample + (delay_sample * decay);
            new_sample = self.tone_eq.as_mut().unwrap().process(new_sample);
            self.delay_buffer.as_mut().unwrap().push_back(new_sample);

            *sample = *sample * (1.0 - mix) + delay_sample * mix;
        }
    }

    fn reset_buffer(&mut self) {
        if let Some(delay_buffer) = &mut self.delay_buffer {
            delay_buffer.iter_mut().for_each(|s| *s = 0.0);
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
                if name == "Delay" {
                    let delay_ms = value.as_float().unwrap();
                    parameter.value = value;

                    if let Some(delay_buffer) = &mut self.delay_buffer {
                        if let Some(sample_rate) = self.sample_rate {
                            let delay_samples = ((delay_ms / 1000.0) * sample_rate) as usize;
                            if delay_samples > delay_buffer.len() {
                                delay_buffer.extend(
                                    iter::repeat(0.0).take(delay_samples - delay_buffer.len()),
                                );
                            } else {
                                // shift samples back so the most recent audio is kept when shortening
                                let shift = delay_buffer.len() - delay_samples;
                                for i in 0..delay_samples {
                                    delay_buffer[i] = delay_buffer[i + shift];
                                }
                                delay_buffer.truncate(delay_samples);
                            }
                        }
                    }
                } else if name == "Warmth" {
                    let warmth = value.as_float().unwrap();
                    parameter.value = value;
                    if let Some(sample_rate) = self.sample_rate {
                        self.tone_eq = Some(Self::eq_from_warmth(warmth, sample_rate));
                    }
                } else {
                    parameter.value = value;
                }
            }
        }
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _message_buffer: &[String],
    ) -> Option<(String, PedalParameterValue)> {
        ui.add(egui::Image::new(include_image!("images/delay.png")));

        let mut to_change = None;
        let delay_param = self.get_parameters().get("Delay").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Delay",
            delay_param,
            egui::Vec2::new(0.125, 0.038),
            0.3,
            self.id,
        ) {
            to_change = Some(("Delay".to_string(), value));
        }

        let decay_param = self.get_parameters().get("Decay").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Decay",
            decay_param,
            egui::Vec2::new(0.58, 0.145),
            0.3,
            self.id,
        ) {
            to_change = Some(("Decay".to_string(), value));
        }

        let warmth_param = self.get_parameters().get("Warmth").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Warmth",
            warmth_param,
            egui::Vec2::new(0.125, 0.27),
            0.3,
            self.id,
        ) {
            to_change = Some(("Warmth".to_string(), value));
        }

        let dry_wet_param = self.get_parameters().get("Dry/Wet").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Dry/Wet",
            dry_wet_param,
            egui::Vec2::new(0.58, 0.365),
            0.3,
            self.id,
        ) {
            to_change = Some(("Dry/Wet".to_string(), value));
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
    const BLOCK: usize = 1024;

    /// A buffer of a steady tone, phase continuous with the buffers around it.
    fn tone_buffer(freq: f32, buffer_index: usize) -> Vec<f32> {
        (0..BLOCK)
            .map(|i| {
                let n = buffer_index * BLOCK + i;
                (std::f32::consts::TAU * freq * n as f32 / SAMPLE_RATE as f32).sin()
            })
            .collect()
    }

    /// A pedal with a delay short enough that the echoes come back within one buffer.
    fn short_delay_pedal() -> Delay {
        let mut pedal = Delay::new();
        pedal.set_parameter_value("Delay", PedalParameterValue::Float(10.0));
        pedal
    }

    /// Runs `buffers` buffers of a tone through the pedal, reconfiguring before every
    /// buffer the way the processor API does when `reconfiguring` is set.
    fn run(pedal: &mut Delay, buffers: usize, reconfiguring: bool) -> Vec<f32> {
        let mut messages = Vec::new();
        let mut output = Vec::with_capacity(buffers * BLOCK);

        if !reconfiguring {
            pedal.set_config(BLOCK, SAMPLE_RATE);
        }

        for buffer_index in 0..buffers {
            if reconfiguring {
                pedal.set_config(BLOCK, SAMPLE_RATE);
            }

            let mut buffer = tone_buffer(220.0, buffer_index);
            pedal.process_audio(&mut buffer, &mut messages);
            output.extend_from_slice(&buffer);
        }

        output
    }

    #[test]
    fn reconfiguring_between_buffers_does_not_change_the_output() {
        let mut reference = short_delay_pedal();
        let expected = run(&mut reference, 4, false);

        let mut reconfigured = short_delay_pedal();
        let actual = run(&mut reconfigured, 4, true);

        let worst = expected
            .iter()
            .zip(&actual)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0f32, f32::max);
        assert!(
            worst < 1e-9,
            "the redundant reconfigure changed the output by {worst}"
        );
    }

    #[test]
    fn reconfiguring_between_buffers_keeps_the_delay_line() {
        let mut messages = Vec::new();
        let mut pedal = short_delay_pedal();
        pedal.set_config(BLOCK, SAMPLE_RATE);

        let mut burst = vec![0.5; BLOCK];
        pedal.process_audio(&mut burst, &mut messages);

        // A configuration that changes nothing has to leave the delay line alone, otherwise
        // the echoes are cut off at every buffer boundary.
        pedal.set_config(BLOCK, SAMPLE_RATE);
        let mut silence = vec![0.0; BLOCK];
        pedal.process_audio(&mut silence, &mut messages);
        assert!(
            silence.iter().any(|sample| *sample != 0.0),
            "the delay line was emptied"
        );
    }

    #[test]
    fn a_changed_sample_rate_rebuilds_the_delay_line() {
        let mut pedal = Delay::new();
        pedal.set_config(BLOCK, SAMPLE_RATE);
        // the default 430 ms of delay
        assert_eq!(pedal.delay_buffer.as_ref().map(VecDeque::len), Some(20_640));

        pedal.set_config(BLOCK, SAMPLE_RATE * 2);
        assert_eq!(pedal.delay_buffer.as_ref().map(VecDeque::len), Some(41_280));
    }
}
