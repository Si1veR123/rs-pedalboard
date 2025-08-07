use std::collections::HashMap;
use std::hash::Hash;
use crate::{pedals::ui::pedal_switch, unique_time_id};

use super::{PedalTrait, PedalParameter, PedalParameterValue, ui::pedal_knob};
use eframe::egui::{self, include_image};
use serde::{Serialize, Deserialize};
use freeverb::Freeverb;

pub struct Reverb {
    // Freeverb instance, Sample rate
    // None if sample rate not yet set
    reverb: Option<(Freeverb, u32)>,
    parameters: HashMap<String, PedalParameter>,
    id: u32
}

impl Hash for Reverb {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.parameters.values().map(|p| &p.value).for_each(|v| v.hash(state));
    }
}

impl Serialize for Reverb {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_map(self.parameters.iter())
    }
}

impl<'a> Deserialize<'a> for Reverb {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        let parameters: HashMap<String, PedalParameter> = HashMap::deserialize(deserializer)?;
        Ok(Reverb { reverb: None, parameters, id: unique_time_id() })
    }
}

impl Clone for Reverb {
    fn clone(&self) -> Self {
        let cloned_reverb = self.reverb.as_ref().and_then(|(_reverb, sample_rate)| {
            Some((Freeverb::new(*sample_rate as usize), *sample_rate))
        });
        let cloned_parameters = self.parameters.clone();
        let mut cloned_pedal = Self {
            reverb: cloned_reverb,
            parameters: cloned_parameters,
            id: unique_time_id()
        };
        cloned_pedal.sync_parameters();
        cloned_pedal
    }
}

impl Reverb {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();

        parameters.insert("room_size".into(), PedalParameter {
            value: PedalParameterValue::Float(0.5),
            min: Some(PedalParameterValue::Float(0.0)),
            max: Some(PedalParameterValue::Float(1.0)),
            step: None,
        });

        parameters.insert("dampening".into(), PedalParameter {
            value: PedalParameterValue::Float(0.5),
            min: Some(PedalParameterValue::Float(0.0)),
            max: Some(PedalParameterValue::Float(1.0)),
            step: None,
        });

        parameters.insert("width".into(), PedalParameter {
            value: PedalParameterValue::Float(1.0),
            min: Some(PedalParameterValue::Float(0.0)),
            max: Some(PedalParameterValue::Float(1.0)),
            step: None,
        });

        parameters.insert("freeze".into(), PedalParameter {
            value: PedalParameterValue::Bool(false),
            min: None,
            max: None,
            step: None,
        });

        parameters.insert("dry_wet".into(), PedalParameter {
            value: PedalParameterValue::Float(0.5),
            min: Some(PedalParameterValue::Float(0.0)),
            max: Some(PedalParameterValue::Float(1.0)),
            step: None,
        });

        parameters.insert(
            "active".to_string(),
            PedalParameter {
                value: PedalParameterValue::Bool(true),
                min: None,
                max: None,
                step: None,
            },
        );

        let pedal = Self {
            reverb: None,
            parameters,
            id: unique_time_id()
        };

        pedal
    }

    fn sync_parameters(&mut self) {
        let p = &self.parameters;

        let dry_wet = p["dry_wet"].value.as_float().unwrap().clamp(0.0, 1.0);
        let wet = dry_wet;
        let dry = 1.0 - dry_wet;

        if let Some((ref mut reverb, _sample_rate)) = &mut self.reverb {
            reverb.set_room_size(p["room_size"].value.as_float().unwrap() as f64);
            reverb.set_dampening(p["dampening"].value.as_float().unwrap() as f64);
            reverb.set_wet(wet as f64);
            reverb.set_dry(dry as f64);
            reverb.set_width(p["width"].value.as_float().unwrap() as f64);
            reverb.set_freeze(p["freeze"].value.as_bool().unwrap_or(false));
        }
    }
}

impl PedalTrait for Reverb {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn set_config(&mut self,_buffer_size:usize, sample_rate:u32) {
        if self.reverb.is_none() {
            let reverb = Freeverb::new(sample_rate as usize);
            self.reverb = Some((reverb, sample_rate));
            self.sync_parameters();
        } else if let Some((ref mut reverb, old_sample_rate)) = &mut self.reverb {
            if sample_rate != *old_sample_rate {
                *reverb = Freeverb::new(sample_rate as usize);
                self.sync_parameters();
            }
        }
    }

    fn process_audio(&mut self, buffer: &mut [f32], _messages: &mut Vec<String>) {
        if let Some((ref mut reverb, _)) = self.reverb {
            for sample in buffer.iter_mut() {
                let (wet_sample, _) = reverb.tick((*sample as f64, 0.0));
                *sample = wet_sample as f32;
            }
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn set_parameter_value(&mut self, name: &str, value:PedalParameterValue) {
        let parameters = self.get_parameters_mut();
        if let Some(parameter) = parameters.get_mut(name) {
            if parameter.is_valid(&value) {
                parameter.value = value;
                if name == "room_size" || name == "dampening" || name == "width" || name == "dry_wet" || name == "freeze" {
                    self.sync_parameters();
                }
            } else {
                log::warn!("Attempted to set invalid value for parameter {}: {:?}", name, value);
            }
        }
    }

    fn ui(&mut self, ui: &mut eframe::egui::Ui, _message_buffer: &[String]) -> Option<(String, PedalParameterValue)> {
        ui.add(egui::Image::new(include_image!("images/reverb.png")));

        let mut to_change = None;
        let room_size_param = self.get_parameters().get("room_size").unwrap();
        if let Some(value) = pedal_knob(ui, "", room_size_param, egui::Vec2::new(0.05, 0.022), 0.3) {
            to_change = Some(("room_size".to_string(), value));
        }

        let width_param = self.get_parameters().get("width").unwrap();
        if let Some(value) = pedal_knob(ui, "", width_param, egui::Vec2::new(0.05, 0.171), 0.3) {
            to_change = Some(("width".to_string(), value));
        }

        let dampening_param = self.get_parameters().get("dampening").unwrap();
        if let Some(value) = pedal_knob(ui, "", dampening_param, egui::Vec2::new(0.05, 0.32), 0.3) {
            to_change = Some(("dampening".to_string(), value));
        }

        let dry_wet_param = self.get_parameters().get("dry_wet").unwrap();
        if let Some(value) = pedal_knob(ui, "", dry_wet_param, egui::Vec2::new(0.05, 0.469), 0.3) {
            to_change = Some(("dry_wet".to_string(), value));
        }

        let active_param = self.get_parameters().get("active").unwrap().value.as_bool().unwrap();
        if let Some(value) = pedal_switch(ui, active_param, egui::Vec2::new(0.33, 0.72), 0.16) {
            to_change = Some(("active".to_string(), PedalParameterValue::Bool(value)));
        }

        to_change
    }
}