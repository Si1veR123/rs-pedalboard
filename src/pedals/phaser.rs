use std::collections::HashMap;
use std::hash::Hash;

use super::ui::pedal_knob;
use super::{PedalParameter, PedalParameterValue, PedalTrait};
use crate::dsp_algorithms::oscillator::{Oscillator, Sine};
use crate::dsp_algorithms::phaser::{Phaser as PhaserAlgorithm, SWEEP_HIGH_HZ, SWEEP_LOW_HZ};
use crate::pedals::ui::pedal_switch;
use crate::unique_time_id;

use eframe::egui::{self, include_image};
use serde::{ser::SerializeMap, Deserialize, Serialize};

#[derive(Clone)]
pub struct Phaser {
    parameters: HashMap<String, PedalParameter>,
    // Processor only
    phaser: Option<PhaserAlgorithm>,
    /// Sample rate the sweep was built for, so that reconfiguring with the same
    /// sample rate does not restart it.
    configured_sample_rate: Option<u32>,
    id: u32,
}

impl Hash for Phaser {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Serialize for Phaser {
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

impl<'a> Deserialize<'a> for Phaser {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        #[derive(Deserialize)]
        struct PhaserData {
            id: u32,
            parameters: HashMap<String, PedalParameter>,
        }
        let helper = PhaserData::deserialize(deserializer)?;
        Ok(Phaser {
            parameters: helper.parameters,
            phaser: None,
            configured_sample_rate: None,
            id: helper.id,
        })
    }
}

impl Phaser {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();

        // Sample rate on oscillators is not used on clients so the hardcoded sample rate is ok
        let init_oscillator = Oscillator::Sine(Sine::new(48000.0, 0.5, 0.0, 0.0));

        parameters.insert(
            "Oscillator".to_string(),
            PedalParameter {
                value: PedalParameterValue::Oscillator(init_oscillator),
                min: Some(PedalParameterValue::Float(0.05)),
                max: Some(PedalParameterValue::Float(10.0)),
                step: None,
            },
        );

        parameters.insert(
            "Width".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.7),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None,
            },
        );

        parameters.insert(
            "Feedback".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.3),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(0.95)),
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
            "Active".to_string(),
            PedalParameter {
                value: PedalParameterValue::Bool(true),
                min: None,
                max: None,
                step: None,
            },
        );

        Phaser {
            parameters,
            phaser: None,
            configured_sample_rate: None,
            id: unique_time_id(),
        }
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned
    }

    fn param_float(&self, name: &str) -> f32 {
        self.parameters.get(name).unwrap().value.as_float().unwrap()
    }
}

impl PedalTrait for Phaser {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        if self.phaser.is_none() {
            tracing::error!("Phaser is not initialized. Call set_config() first.");
            return;
        }
        self.phaser.as_mut().unwrap().process_audio(buffer);
    }

    fn reset_buffer(&mut self) {
        if let Some(phaser) = &mut self.phaser {
            phaser.reset();
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn set_parameter_value(&mut self, name: &str, value: PedalParameterValue) {
        if !self.parameters.contains_key(name)
            || !self.parameters.get(name).unwrap().is_valid(&value)
        {
            return;
        }

        match name {
            "Oscillator" => {
                if let PedalParameterValue::Oscillator(oscillator) = value {
                    if let Some(phaser) = &mut self.phaser {
                        phaser.oscillator = oscillator.clone();
                    }
                    self.parameters.get_mut(name).unwrap().value =
                        PedalParameterValue::Oscillator(oscillator);
                }
            }
            "Width" => {
                if let PedalParameterValue::Float(width) = value {
                    if let Some(phaser) = &mut self.phaser {
                        phaser.set_width(width);
                    }
                    self.parameters.get_mut(name).unwrap().value =
                        PedalParameterValue::Float(width);
                }
            }
            "Feedback" => {
                if let PedalParameterValue::Float(feedback) = value {
                    if let Some(phaser) = &mut self.phaser {
                        phaser.set_feedback(feedback);
                    }
                    self.parameters.get_mut(name).unwrap().value =
                        PedalParameterValue::Float(feedback);
                }
            }
            "Dry/Wet" => {
                if let PedalParameterValue::Float(dry_wet) = value {
                    if let Some(phaser) = &mut self.phaser {
                        phaser.set_blend(dry_wet);
                    }
                    self.parameters.get_mut(name).unwrap().value =
                        PedalParameterValue::Float(dry_wet);
                }
            }
            _ => {
                if let Some(parameter) = self.parameters.get_mut(name) {
                    parameter.value = value;
                } else {
                    tracing::warn!("Attempted to set unknown parameter: {}", name);
                }
            }
        }
    }

    fn set_config(&mut self, _buffer_size: usize, sample_rate: u32) {
        let parameter_oscillator = self
            .parameters
            .get_mut("Oscillator")
            .unwrap()
            .value
            .as_oscillator_mut()
            .unwrap();
        parameter_oscillator.set_sample_rate(sample_rate as f32);
        let oscillator = parameter_oscillator.clone();

        // The processor API calls this before every buffer, so only build the sweep when
        // the configuration really changed. Rebuilding it would restart the LFO from
        // phase zero and wipe the all-pass states, which parks the sweep at the start of
        // its cycle instead of letting it run.
        if self.configured_sample_rate == Some(sample_rate) {
            return;
        }

        let width = self.param_float("Width");
        let feedback = self.param_float("Feedback");
        let dry_wet = self.param_float("Dry/Wet");

        self.phaser = Some(PhaserAlgorithm::new(
            SWEEP_LOW_HZ,
            SWEEP_HIGH_HZ,
            width,
            feedback,
            dry_wet,
            oscillator,
            sample_rate as f32,
        ));
        self.configured_sample_rate = Some(sample_rate);
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _message_buffer: &[String],
    ) -> Option<(String, PedalParameterValue)> {
        ui.add(egui::Image::new(include_image!("images/phaser.png")));

        let mut to_change = None;

        let width_param = self.get_parameters().get("Width").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Width",
            width_param,
            egui::Vec2::new(0.086, 0.036),
            0.3,
            self.id,
        ) {
            to_change = Some(("Width".to_string(), value));
        }

        let feedback_param = self.get_parameters().get("Feedback").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Feedback",
            feedback_param,
            egui::Vec2::new(0.61, 0.036),
            0.3,
            self.id,
        ) {
            to_change = Some(("Feedback".to_string(), value));
        }

        let dry_wet_param = self.get_parameters().get("Dry/Wet").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Dry/Wet",
            dry_wet_param,
            egui::Vec2::new(0.35, 0.3),
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

    fn rms(signal: &[f32]) -> f64 {
        (signal.iter().map(|x| (*x as f64).powi(2)).sum::<f64>() / signal.len() as f64).sqrt()
    }

    /// Level of a steady probe in consecutive buffers, with the pedal reconfigured
    /// before each buffer the way the processor API does it.
    fn levels_per_buffer(buffers: usize, probe_hz: f32) -> Vec<f64> {
        let mut pedal = Phaser::new();
        let mut messages = Vec::new();
        let mut levels = Vec::with_capacity(buffers);

        for buffer_index in 0..buffers {
            pedal.set_config(BLOCK, SAMPLE_RATE);

            let mut buffer: Vec<f32> = (0..BLOCK)
                .map(|i| {
                    let n = buffer_index * BLOCK + i;
                    (std::f32::consts::TAU * probe_hz * n as f32 / SAMPLE_RATE as f32).sin()
                })
                .collect();
            pedal.process_audio(&mut buffer, &mut messages);
            levels.push(rms(&buffer));
        }

        levels
    }

    #[test]
    fn reconfiguring_between_buffers_does_not_restart_the_sweep() {
        // 100 buffers of 1024 samples is two LFO cycles at the default 0.5 Hz, and the
        // notches of the sweep cross 500 Hz four times over that.
        let levels = levels_per_buffer(100, 500.0);

        let quietest = levels.iter().cloned().fold(f64::INFINITY, f64::min);
        let loudest = levels.iter().cloned().fold(0.0, f64::max);
        assert!(
            loudest / quietest > 2.0,
            "the sweep only moved the level between {quietest} and {loudest}"
        );
    }

    #[test]
    fn reconfiguring_between_buffers_keeps_the_filter_state() {
        let mut pedal = Phaser::new();
        let mut messages = Vec::new();
        pedal.set_config(BLOCK, SAMPLE_RATE);

        let mut buffer = vec![0.5; BLOCK];
        pedal.process_audio(&mut buffer, &mut messages);

        // A configuration that changes nothing has to leave the running filter alone,
        // otherwise every buffer starts again from silence.
        pedal.set_config(BLOCK, SAMPLE_RATE);
        let mut silence = vec![0.0; BLOCK];
        pedal.process_audio(&mut silence, &mut messages);
        assert!(
            silence.iter().any(|sample| *sample != 0.0),
            "the all-pass states were wiped"
        );
    }

    #[test]
    fn a_changed_sample_rate_rebuilds_the_sweep() {
        let mut pedal = Phaser::new();
        pedal.set_config(BLOCK, SAMPLE_RATE);
        assert_eq!(pedal.configured_sample_rate, Some(SAMPLE_RATE));

        pedal.set_config(BLOCK, SAMPLE_RATE * 2);
        assert_eq!(pedal.configured_sample_rate, Some(SAMPLE_RATE * 2));
    }
}
