use std::collections::HashMap;
use std::hash::Hash;

use super::{PedalTrait, PedalParameter, PedalParameterValue};
use serde::{ser::SerializeMap, Deserialize, Serialize};
use crate::{dsp_algorithms::moving_bandpass::MovingBandPass, pedals::ui::pedal_switch, unique_time_id};
use super::ui::pedal_knob;

use eframe::egui::{self, include_image};

#[derive(Clone)]
pub struct Wah {
    parameters: HashMap<String, PedalParameter>,
    // Server only
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
            id: helper.id
        })
    }
}

impl Wah {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();

        parameters.insert("position".to_string(), PedalParameter {
            value: PedalParameterValue::Float(0.5),
            min: Some(PedalParameterValue::Float(0.0)),
            max: Some(PedalParameterValue::Float(1.0)),
            step: None
        });

        parameters.insert("base_freq".to_string(), PedalParameter {
            value: PedalParameterValue::Float(100.0),
            min: Some(PedalParameterValue::Float(50.0)),
            max: Some(PedalParameterValue::Float(1000.0)),
            step: None
        });

        parameters.insert("width".to_string(), PedalParameter {
            value: PedalParameterValue::Float(0.5),
            min: Some(PedalParameterValue::Float(0.1)),
            max: Some(PedalParameterValue::Float(2.0)),
            step: None
        });

        parameters.insert("sensitivity".to_string(), PedalParameter {
            value: PedalParameterValue::Float(1000.0),
            min: Some(PedalParameterValue::Float(100.0)),
            max: Some(PedalParameterValue::Float(4000.0)),
            step: None
        });

        parameters.insert("dry_wet".to_string(), PedalParameter {
            value: PedalParameterValue::Float(1.0),
            min: Some(PedalParameterValue::Float(0.0)),
            max: Some(PedalParameterValue::Float(1.0)),
            step: None
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
        self.sample_rate = Some(sample_rate as f32);

        // Initialize the moving bandpass filter
        self.moving_bandpass_filter = Some(MovingBandPass::new(
            self.parameters.get("base_freq").and_then(|p| p.value.as_float()).unwrap(),
            sample_rate as f32,
            self.parameters.get("width").and_then(|p| p.value.as_float()).unwrap(),
            64,
            2.0
        ));
    }

    fn set_parameter_value(&mut self,name: &str,value:PedalParameterValue) {
        let parameters = self.get_parameters_mut();
        if let Some(parameter) = parameters.get_mut(name) {
            if parameter.is_valid(&value) {
                parameter.value = value;

                if name == "width" {
                    if let Some(filter) = &mut self.moving_bandpass_filter {
                        filter.set_width(self.parameters.get("width").unwrap().value.as_float().unwrap());
                    }
                }
            }
        }
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        if self.moving_bandpass_filter.is_none() {
            log::warn!("Wah: Call set_config before processing.");
            return;
        }

        let position = self.parameters.get("position").unwrap().value.as_float().unwrap();
        let base_freq = self.parameters.get("base_freq").unwrap().value.as_float().unwrap();
        let sensitivity = self.parameters.get("sensitivity").unwrap().value.as_float().unwrap();
        let dry_wet = self.parameters.get("dry_wet").unwrap().value.as_float().unwrap();

        let filter = self.moving_bandpass_filter.as_mut().unwrap();
        filter.set_freq(base_freq + position * sensitivity);

        for sample in buffer.iter_mut() {
            *sample = filter.process(*sample) * dry_wet + *sample * (1.0 - dry_wet);
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn ui(&mut self, ui: &mut eframe::egui::Ui,_message_buffer: &[String]) -> Option<(String,PedalParameterValue)> {
        ui.add(egui::Image::new(include_image!("images/wah.png")));

        let mut to_change = None;

        let base_freq_param = self.get_parameters().get("base_freq").unwrap();
        if let Some(value) = pedal_knob(ui, "", base_freq_param, egui::Vec2::new(0.68, 0.04), 0.25) {
            to_change = Some(("base_freq".to_string(), value));
        }

        let sensitivity_param = self.get_parameters().get("sensitivity").unwrap();
        if let Some(value) = pedal_knob(ui, "", sensitivity_param, egui::Vec2::new(0.68, 0.165), 0.25) {
            to_change = Some(("sensitivity".to_string(), value));
        }

        let width_param = self.get_parameters().get("width").unwrap();
        if let Some(value) = pedal_knob(ui, "", width_param, egui::Vec2::new(0.68, 0.29), 0.25) {
            to_change = Some(("width".to_string(), value));
        }

        let position_param = self.get_parameters().get("position").unwrap();
        if let Some(value) = pedal_knob(ui, "", position_param, egui::Vec2::new(0.68, 0.42), 0.25) {
            to_change = Some(("position".to_string(), value));
        }

        let active_param = self.get_parameters().get("active").unwrap().value.as_bool().unwrap();
        if let Some(value) = pedal_switch(ui, active_param, egui::Vec2::new(0.33, 0.72), 0.16) {
            to_change = Some(("active".to_string(), PedalParameterValue::Bool(value)));
        }

        to_change
    }
}