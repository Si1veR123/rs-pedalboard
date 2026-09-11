use std::collections::HashMap;
use std::hash::Hash;

use super::ui::pedal_knob;
use super::{PedalParameter, PedalParameterValue, PedalTrait};
use crate::{
    dsp_algorithms::moving_bandpass::MovingBandPass, pedals::ui::pedal_switch, unique_time_id,
};
use serde::{ser::SerializeMap, Deserialize, Serialize};

use eframe::egui::{self, include_image};

#[derive(Clone)]
pub struct Wah {
    parameters: HashMap<String, PedalParameter>,
    // Processor only
    sample_rate: Option<f32>,
    moving_bandpass_filter: Option<MovingBandPass>,

    id: u32,
}

impl Hash for Wah {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Serialize for Wah {
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

impl<'a> Deserialize<'a> for Wah {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        #[derive(Deserialize)]
        struct WahData {
            id: u32,
            parameters: HashMap<String, PedalParameter>,
        }
        let helper = WahData::deserialize(deserializer)?;
        Ok(Wah {
            parameters: helper.parameters,
            sample_rate: None,
            moving_bandpass_filter: None,
            id: helper.id,
        })
    }
}

impl Wah {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();

        parameters.insert(
            "Position".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.5),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None,
            },
        );

        parameters.insert(
            "Base Frequency".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(100.0),
                min: Some(PedalParameterValue::Float(50.0)),
                max: Some(PedalParameterValue::Float(1000.0)),
                step: None,
            },
        );

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
                value: PedalParameterValue::Float(1000.0),
                min: Some(PedalParameterValue::Float(100.0)),
                max: Some(PedalParameterValue::Float(4000.0)),
                step: None,
            },
        );

        parameters.insert(
            "Dry/Wet".to_string(),
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

        Self {
            parameters,
            sample_rate: None,
            moving_bandpass_filter: None,
            id: unique_time_id(),
        }
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned
    }
}

impl PedalTrait for Wah {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn set_config(&mut self, _buffer_size: usize, sample_rate: u32) {
        // The processor API calls this before every buffer, so only build the filter when
        // the configuration really changed. Rebuilding it would wipe the biquad states,
        // which makes the filter ring in again from silence on every buffer.
        if self.sample_rate == Some(sample_rate as f32) {
            return;
        }

        self.sample_rate = Some(sample_rate as f32);

        // Initialize the moving bandpass filter
        self.moving_bandpass_filter = Some(MovingBandPass::new(
            self.parameters
                .get("Base Frequency")
                .and_then(|p| p.value.as_float())
                .unwrap(),
            sample_rate as f32,
            self.parameters
                .get("Width")
                .and_then(|p| p.value.as_float())
                .unwrap(),
            64,
            2.0,
        ));
    }

    fn set_parameter_value(&mut self, name: &str, value: PedalParameterValue) {
        let parameters = self.get_parameters_mut();
        if let Some(parameter) = parameters.get_mut(name) {
            if parameter.is_valid(&value) {
                parameter.value = value;

                if name == "Width" {
                    if let Some(filter) = &mut self.moving_bandpass_filter {
                        filter.set_width(
                            self.parameters
                                .get("Width")
                                .unwrap()
                                .value
                                .as_float()
                                .unwrap(),
                        );
                    }
                }
            }
        }
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        if self.moving_bandpass_filter.is_none() {
            tracing::warn!("Wah: Call set_config before processing.");
            return;
        }

        let position = self
            .parameters
            .get("Position")
            .unwrap()
            .value
            .as_float()
            .unwrap();
        let base_freq = self
            .parameters
            .get("Base Frequency")
            .unwrap()
            .value
            .as_float()
            .unwrap();
        let sensitivity = self
            .parameters
            .get("Sensitivity")
            .unwrap()
            .value
            .as_float()
            .unwrap();
        let dry_wet = self
            .parameters
            .get("Dry/Wet")
            .unwrap()
            .value
            .as_float()
            .unwrap();

        let filter = self.moving_bandpass_filter.as_mut().unwrap();
        filter.set_freq(base_freq + position * sensitivity);

        for sample in buffer.iter_mut() {
            *sample = filter.process(*sample) * dry_wet + *sample * (1.0 - dry_wet);
        }
    }

    fn reset_buffer(&mut self) {
        if let Some(filter) = &mut self.moving_bandpass_filter {
            filter.reset();
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
        ui: &mut eframe::egui::Ui,
        _message_buffer: &[String],
    ) -> Option<(String, PedalParameterValue)> {
        ui.add(egui::Image::new(include_image!("images/wah.png")));

        let mut to_change = None;

        let base_freq_param = self.get_parameters().get("Base Frequency").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Base Frequency",
            base_freq_param,
            egui::Vec2::new(0.68, 0.04),
            0.25,
            self.id,
        ) {
            to_change = Some(("Base Frequency".to_string(), value));
        }

        let sensitivity_param = self.get_parameters().get("Sensitivity").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Sensitivity",
            sensitivity_param,
            egui::Vec2::new(0.68, 0.165),
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
            egui::Vec2::new(0.68, 0.29),
            0.25,
            self.id,
        ) {
            to_change = Some(("Width".to_string(), value));
        }

        let position_param = self.get_parameters().get("Position").unwrap();
        if let Some(value) = pedal_knob(
            ui,
            "",
            "Position",
            position_param,
            egui::Vec2::new(0.68, 0.42),
            0.25,
            self.id,
        ) {
            to_change = Some(("Position".to_string(), value));
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

    /// Runs `buffers` buffers of a tone through the pedal, reconfiguring before every
    /// buffer the way the processor API does when `reconfiguring` is set.
    fn run(pedal: &mut Wah, tone_hz: f32, buffers: usize, reconfiguring: bool) -> Vec<f32> {
        let mut messages = Vec::new();
        let mut output = Vec::with_capacity(buffers * BLOCK);

        if !reconfiguring {
            pedal.set_config(BLOCK, SAMPLE_RATE);
        }

        for buffer_index in 0..buffers {
            if reconfiguring {
                pedal.set_config(BLOCK, SAMPLE_RATE);
            }

            let mut buffer = tone_buffer(tone_hz, buffer_index);
            pedal.process_audio(&mut buffer, &mut messages);
            output.extend_from_slice(&buffer);
        }

        output
    }

    #[test]
    fn reconfiguring_between_buffers_does_not_change_the_output() {
        let mut reference = Wah::new();
        let expected = run(&mut reference, 220.0, 4, false);

        let mut reconfigured = Wah::new();
        let actual = run(&mut reconfigured, 220.0, 4, true);

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
    fn reconfiguring_between_buffers_keeps_the_filter_state() {
        let mut messages = Vec::new();
        let mut pedal = Wah::new();
        pedal.set_config(BLOCK, SAMPLE_RATE);

        let mut tone = tone_buffer(220.0, 0);
        pedal.process_audio(&mut tone, &mut messages);

        // A configuration that changes nothing has to leave the running filter alone,
        // otherwise the filter rings in again from silence on every buffer.
        pedal.set_config(BLOCK, SAMPLE_RATE);
        let mut silence = vec![0.0; BLOCK];
        pedal.process_audio(&mut silence, &mut messages);
        assert!(
            silence.iter().any(|sample| *sample != 0.0),
            "the biquad states were wiped"
        );
    }

    #[test]
    fn a_changed_sample_rate_rebuilds_the_filter() {
        let mut pedal = Wah::new();
        pedal.set_config(BLOCK, SAMPLE_RATE);
        assert_eq!(pedal.sample_rate, Some(SAMPLE_RATE as f32));

        pedal.set_config(BLOCK, SAMPLE_RATE * 2);
        assert_eq!(pedal.sample_rate, Some(SAMPLE_RATE as f32 * 2.0));
        assert_eq!(
            pedal
                .moving_bandpass_filter
                .as_ref()
                .map(MovingBandPass::current_freq),
            Some(100.0)
        );
    }
}
