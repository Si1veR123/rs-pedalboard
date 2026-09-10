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
            id: unique_time_id(),
        }
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned
    }

    fn param_float(&self, name: &str) -> f32 {
        self.parameters
            .get(name)
            .unwrap()
            .value
            .as_float()
            .unwrap()
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
