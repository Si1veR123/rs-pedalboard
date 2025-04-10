use std::collections::{HashMap, VecDeque};
use std::iter;
use std::hash::Hash;
use super::{PedalParameter, PedalParameterValue, PedalTrait, ui::pedal_knob};
use serde::ser::SerializeMap;
use serde::{Serialize, Deserialize};

#[derive(Clone)]
pub struct Delay {
    pub parameters: HashMap<String, PedalParameter>,
    delay_buffer: VecDeque<f32>
}

impl Hash for Delay {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Probably not technically correct since values may change order but good enough for now
        self.parameters.values().map(|p| &p.value).for_each(|v| v.hash(state));
    }
}

impl Serialize for Delay {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser_map = serializer.serialize_map(Some(self.parameters.len()))?;
        for (key, value) in &self.parameters {
            ser_map.serialize_entry(key, value)?;
        }
        Ok(ser_map.end()?)
    }
}

impl<'a> Deserialize<'a> for Delay {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        let parameters: HashMap<String, PedalParameter> = HashMap::deserialize(deserializer)?;
        // Just unwrap since if the parameter is missing, the pedal is going to be unusable anyway
        let delay = parameters.get("delay").unwrap().value.as_float().unwrap();
        let delay_samples = ((delay / 1000.0) * 48000.0) as usize;
        Ok(Delay { parameters, delay_buffer: VecDeque::from_iter(iter::repeat(0.0).take(delay_samples)) })
    }
}

impl Delay {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();

        let init_delay_ms = 50.0;
        let init_delay_samples = ((init_delay_ms / 1000.0) * 48000.0) as usize;

        parameters.insert(
            "delay".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(init_delay_ms),
                min: Some(PedalParameterValue::Float(10.0)),
                max: Some(PedalParameterValue::Float(1000.0)),
                step: None
            },
        );
        parameters.insert(
            "decay".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.5),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None
            },
        );
        parameters.insert(
            "mix".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(0.5),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None
            },
        );
        Delay { parameters, delay_buffer: VecDeque::from_iter(iter::repeat(0.0).take(init_delay_samples)) }
    }
}

impl PedalTrait for Delay {
    fn process_audio(&mut self, buffer: &mut [f32]) {
        let decay = self.parameters.get("decay").unwrap().value.as_float().unwrap();
        let mix = self.parameters.get("mix").unwrap().value.as_float().unwrap();

        for sample in buffer.iter_mut() {
            let delayed_sample = self.delay_buffer.pop_front().unwrap();
            self.delay_buffer.push_back(*sample + delayed_sample * decay);
            *sample = *sample * (1.0 - mix) + delayed_sample * mix;
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
                if name == "delay" {
                    let delay = value.as_float().unwrap();
                    let delay_samples = ((delay / 1000.0) * 48000.0) as usize;
                    let old_delay = parameter.value.as_float().unwrap();
                    let old_delay_samples = ((old_delay / 1000.0) * 48000.0) as usize;

                    parameter.value = value;

                    if delay_samples < old_delay_samples {
                        self.delay_buffer.truncate(old_delay_samples - delay_samples);
                    } else {
                        self.delay_buffer = VecDeque::from_iter(iter::repeat(0.0).take(delay_samples as usize));
                    }
                } else {
                    parameter.value = value;
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut eframe::egui::Ui) -> Option<(String, PedalParameterValue)> {
        let mut to_change = None;
        for (parameter_name, parameter) in self.get_parameters().iter() {
            if let Some(value) = pedal_knob(ui, parameter_name, parameter) {
                to_change = Some((parameter_name.clone(), value));
            }
        }
        to_change
    }
}
